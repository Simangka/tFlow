use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use futures::FutureExt;
use crate::async_tasks::{Task, TaskResult, TaskId, RecoveryFile};
use crate::workspace::SearchResult;

#[derive(Debug)]
pub struct Envelope {
    pub id: TaskId,
    pub task: Task,
}

#[derive(Debug)]
enum WorkerEvent {
    Started { id: TaskId, token: Arc<AtomicBool> },
    Finished { id: TaskId },
}

#[derive(Debug)]
pub struct TaskQueue {
    pub(crate) pending: VecDeque<Envelope>,
    pub(crate) in_progress: HashSet<TaskId>,
    pub(crate) in_progress_tokens: HashMap<TaskId, Arc<AtomicBool>>,
    pub(crate) max_concurrent: usize,
    pub(crate) task_tx: mpsc::Sender<Envelope>,
    pub(crate) task_rx: Option<Arc<tokio::sync::Mutex<mpsc::Receiver<Envelope>>>>,
    pub(crate) result_tx: mpsc::UnboundedSender<TaskResult>,
    pub(crate) result_rx: Option<mpsc::UnboundedReceiver<TaskResult>>,
    pub(crate) next_id: TaskId,
}

impl TaskQueue {
    pub fn new(max_concurrent: usize) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<Envelope>(1024);
        let task_rx = Arc::new(tokio::sync::Mutex::new(task_rx));
        let (result_tx, result_rx) = mpsc::unbounded_channel::<TaskResult>();
        TaskQueue {
            pending: VecDeque::new(),
            in_progress: HashSet::new(),
            in_progress_tokens: HashMap::new(),
            max_concurrent: max_concurrent.max(1),
            task_tx,
            task_rx: Some(task_rx),
            result_tx,
            result_rx: Some(result_rx),
            next_id: 1,
        }
    }

    pub fn enqueue(&mut self, task: Task) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        let envelope = Envelope { id, task };
        match self.task_tx.try_send(envelope) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(env))
            | Err(mpsc::error::TrySendError::Closed(env)) => {
                self.pending.push_back(env);
            }
        }
        id
    }

    pub fn enqueue_front(&mut self, task: Task) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        let envelope = Envelope { id, task };
        match self.task_tx.try_send(envelope) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(env))
            | Err(mpsc::error::TrySendError::Closed(env)) => {
                self.pending.push_front(env);
            }
        }
        id
    }

    pub fn cancel(&mut self, id: TaskId) {
        self.pending.retain(|e| e.id != id);
        if self.in_progress.contains(&id) {
            if let Some(token) = self.in_progress_tokens.remove(&id) {
                token.store(true, Ordering::SeqCst);
            }
            self.in_progress.remove(&id);
        }
    }

    pub fn cancel_all(&mut self) {
        self.pending.clear();
        let tokens: Vec<Arc<AtomicBool>> =
            self.in_progress_tokens.drain().map(|(_, t)| t).collect();
        for t in tokens {
            t.store(true, Ordering::SeqCst);
        }
        self.in_progress.clear();
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len() + self.in_progress.len()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.in_progress.is_empty()
    }

    pub fn spawn_worker(&self) -> tokio::task::JoinHandle<()> {
        let task_rx = self
            .task_rx
            .as_ref()
            .expect("task_rx already taken")
            .clone();
        let result_tx = self.result_tx.clone();
        let (event_tx, _event_rx) = mpsc::unbounded_channel::<WorkerEvent>();
        tokio::spawn(worker_loop(task_rx, result_tx, event_tx))
    }

    pub fn start_processing(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let task_rx = self.task_rx.take().expect("task_rx already taken");
            let result_tx = self.result_tx.clone();
            let mut result_rx = self.result_rx.take().expect("result_rx already taken");
            let (event_tx, mut event_rx) = mpsc::unbounded_channel::<WorkerEvent>();
            drop(self.result_tx);

            let mut workers = Vec::new();
            for _ in 0..self.max_concurrent {
                let task_rx = task_rx.clone();
                let result_tx = result_tx.clone();
                let event_tx = event_tx.clone();
                workers.push(tokio::spawn(worker_loop(task_rx, result_tx, event_tx)));
            }
            drop(result_tx);
            drop(event_tx);

            loop {
                tokio::select! {
                    biased;
                    Some(event) = event_rx.recv() => {
                        match event {
                            WorkerEvent::Started { id, token } => {
                                self.pending.retain(|e| e.id != id);
                                self.in_progress.insert(id);
                                self.in_progress_tokens.insert(id, token);
                            }
                            WorkerEvent::Finished { id } => {
                                self.in_progress.remove(&id);
                                self.in_progress_tokens.remove(&id);
                            }
                        }
                    }
                    Some(_result) = result_rx.recv() => {}
                    else => break,
                }

                while self.in_progress.len() < self.max_concurrent {
                    let envelope = match self.pending.pop_front() {
                        Some(e) => e,
                        None => break,
                    };
                    match self.task_tx.try_send(envelope) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(env)) => {
                            self.pending.push_front(env);
                            break;
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            break;
                        }
                    }
                }
            }

            drop(self.task_tx);
            for w in workers {
                let _ = w.await;
            }
        })
    }
}

async fn worker_loop(
    task_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<Envelope>>>,
    result_tx: mpsc::UnboundedSender<TaskResult>,
    event_tx: mpsc::UnboundedSender<WorkerEvent>,
) {
    loop {
        let envelope_opt = {
            let mut rx = task_rx.lock().await;
            rx.recv().await
        };
        let Some(envelope) = envelope_opt else {
            break;
        };

        let token = Arc::new(AtomicBool::new(false));
        let _ = event_tx.send(WorkerEvent::Started {
            id: envelope.id,
            token: token.clone(),
        });

        let id = envelope.id;
        let task = envelope.task;
        let token_for_task = token;

        let outcome = std::panic::AssertUnwindSafe(async move {
            execute_task(id, task, token_for_task).await
        })
        .catch_unwind()
        .await;

        let result = match outcome {
            Ok(r) => r,
            Err(_) => TaskResult::Error {
                id,
                message: "task panicked during execution".to_string(),
            },
        };

        let _ = result_tx.send(result);
        let _ = event_tx.send(WorkerEvent::Finished { id });
    }
}

async fn execute_task(id: TaskId, task: Task, token: Arc<AtomicBool>) -> TaskResult {
    match task {
        Task::Autosave(buffer_id) => {
            let result = execute_autosave(buffer_id);
            let (success, error) = match result {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            };
            TaskResult::AutosaveResult { success, error }
        }
        Task::WorkspaceSearch { root, query, id: _ } => {
            if token.load(Ordering::SeqCst) {
                return TaskResult::WorkspaceSearchResult {
                    id,
                    results: Vec::new(),
                };
            }
            let root_c = root.clone();
            let query_c = query.clone();
            let join = tokio::task::spawn_blocking(move || {
                run_workspace_search(&root_c, &query_c)
            })
            .await;
            let results = match join {
                Ok(v) => v,
                Err(_) => Vec::new(),
            };
            TaskResult::WorkspaceSearchResult { id, results }
        }
        Task::FileIndex { root } => {
            if token.load(Ordering::SeqCst) {
                return TaskResult::FileIndexResult {
                    root,
                    entries: Vec::new(),
                };
            }
            let root_for_blocking = root.clone();
            let join = tokio::task::spawn_blocking(move || {
                run_file_index(&root_for_blocking)
            })
            .await;
            let entries = match join {
                Ok(v) => v,
                Err(_) => Vec::new(),
            };
            TaskResult::FileIndexResult { root, entries }
        }
        Task::CrashRecovery => {
            if token.load(Ordering::SeqCst) {
                return TaskResult::CrashRecoveryResult(None);
            }
            let recovered = tokio::task::spawn_blocking(run_crash_recovery)
                .await
                .unwrap_or(None);
            TaskResult::CrashRecoveryResult(recovered)
        }
        Task::FileWatch(_path) => TaskResult::FileWatchResult(Vec::new()),
        Task::SyntaxHighlight { buffer_id: _, lines: _ } => {
            TaskResult::Unimplemented { id }
        }
        Task::MarkdownRender { buffer_id: _, text: _ } => {
            TaskResult::Unimplemented { id }
        }
        Task::Lint => TaskResult::LintResult(Vec::new()),
    }
}

fn execute_autosave(buffer_id: usize) -> Result<(), anyhow::Error> {
    let _ = buffer_id;
    Ok(())
}

fn run_workspace_search(root: &PathBuf, query: &str) -> Vec<SearchResult> {
    let mut searcher = crate::workspace::search::WorkspaceSearcher::new(root.clone());
    searcher.set_query(query);
    let _ = searcher.search();
    searcher.results
}

fn run_file_index(root: &PathBuf) -> Vec<crate::workspace::FileEntry> {
    match crate::workspace::file_tree::FileTree::build_tree(root, 0, false, true) {
        Ok(entries) => entries,
        Err(_) => Vec::new(),
    }
}

fn run_crash_recovery() -> Option<Vec<RecoveryFile>> {
    let recovery_dir = dirs::config_dir()
        .map(|d| d.join("tflow").join("recovery"))
        .unwrap_or_else(|| PathBuf::from("~/.config/tflow/recovery"));

    if !recovery_dir.exists() {
        return None;
    }

    let mut files = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&recovery_dir) {
        for entry in read_dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let timestamp = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or_else(|| std::time::SystemTime::UNIX_EPOCH);

                let original_path = path.file_stem().and_then(|s| s.to_str()).map(|s| {
                    let parts: Vec<&str> = s.splitn(2, '_').collect();
                    if parts.len() == 2 {
                        PathBuf::from(parts[1])
                    } else {
                        PathBuf::from(s)
                    }
                });

                files.push(RecoveryFile {
                    original_path,
                    recovery_path: path,
                    timestamp,
                });
            }
        }
    }

    if files.is_empty() {
        None
    } else {
        Some(files)
    }
}
