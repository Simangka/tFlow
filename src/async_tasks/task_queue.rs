use tokio::sync::mpsc;
use std::collections::VecDeque;
use std::path::PathBuf;
use crate::async_tasks::{Task, TaskResult, TaskId, RecoveryFile, MarkdownLine};
use crate::workspace::SearchResult;

#[derive(Debug)]
pub struct TaskQueue {
    pub pending: VecDeque<Task>,
    pub in_progress: Option<TaskId>,
    pub max_concurrent: usize,
    pub task_tx: mpsc::UnboundedSender<Task>,
    pub task_rx: Option<mpsc::UnboundedReceiver<Task>>,
    pub result_tx: mpsc::UnboundedSender<TaskResult>,
    pub result_rx: Option<mpsc::UnboundedReceiver<TaskResult>>,
    pub next_id: TaskId,
}

impl TaskQueue {
    pub fn new(max_concurrent: usize) -> Self {
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = mpsc::unbounded_channel();
        TaskQueue {
            pending: VecDeque::new(),
            in_progress: None,
            max_concurrent,
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
        self.pending.push_back(task);
        id
    }

    pub fn enqueue_front(&mut self, task: Task) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push_front(task);
        id
    }

    pub fn dequeue(&mut self) -> Option<Task> {
        self.pending.pop_front()
    }

    pub fn cancel(&mut self, id: TaskId) {
        self.pending.retain(|_| {
            self.in_progress.map(|i| i != id).unwrap_or(true)
        });
    }

    pub fn cancel_all(&mut self) {
        self.pending.clear();
        self.in_progress = None;
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn spawn_worker(&self) -> tokio::task::JoinHandle<()> {
        let result_tx = self.result_tx.clone();
        let mut task_rx = mpsc::unbounded_channel::<Task>().1;

        tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                let result = match task {
                    Task::Autosave(buffer_id) => {
                        let result = execute_autosave(buffer_id);
                        let (success, error) = match result {
                            Ok(()) => (true, None),
                            Err(e) => (false, Some(e.to_string())),
                        };
                        TaskResult::AutosaveResult { success, error }
                    }
                    Task::WorkspaceSearch { root, query, id } => {
                        let results = run_workspace_search(&root, &query);
                        TaskResult::WorkspaceSearchResult { id, results }
                    }
                    Task::FileIndex { root } => {
                        let entries = run_file_index(&root);
                        TaskResult::FileIndexResult { root, entries }
                    }
                    Task::CrashRecovery => {
                        let recovered = run_crash_recovery();
                        TaskResult::CrashRecoveryResult(recovered)
                    }
                    Task::FileWatch(_path) => {
                        TaskResult::FileWatchResult(Vec::new())
                    }
                    Task::SyntaxHighlight { buffer_id, lines } => {
                        let highlights = run_syntax_highlight(&lines);
                        TaskResult::SyntaxHighlightResult { buffer_id, highlights }
                    }
                    Task::MarkdownRender { buffer_id, text } => {
                        let lines = run_markdown_render(&text);
                        TaskResult::MarkdownRenderResult { buffer_id, lines }
                    }
                    Task::Lint => {
                        TaskResult::LintResult(Vec::new())
                    }
                };
                let _ = result_tx.send(result);
            }
        })
    }

    pub fn start_processing(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let worker = self.spawn_worker();
            let mut result_rx = self.result_rx.take().expect("result_rx already taken");
            loop {
                if self.has_pending() && self.in_progress.is_none() {
                    if let Some(task) = self.dequeue() {
                        let id = self.next_id;
                        self.next_id += 1;
                        self.in_progress = Some(id);
                        let _ = self.task_tx.send(task);
                    }
                }
                match result_rx.try_recv() {
                    Ok(_result) => {
                        self.in_progress = None;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if !self.has_pending() && self.in_progress.is_none() {
                    break;
                }
            }
            let _ = worker.await;
        })
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
                let timestamp = entry.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or_else(|| std::time::SystemTime::UNIX_EPOCH);

                let original_path = path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| {
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

fn run_syntax_highlight(lines: &[String]) -> Vec<Vec<(String, ratatui::style::Style)>> {
    let mut result = Vec::new();
    let default_style = ratatui::style::Style::default();
    for line in lines {
        let mut segments = Vec::new();
        segments.push((line.clone(), default_style));
        result.push(segments);
    }
    result
}

fn run_markdown_render(text: &str) -> Vec<MarkdownLine> {
    let mut lines = Vec::new();
    let default_style = ratatui::style::Style::default();
    for line in text.lines() {
        lines.push(MarkdownLine {
            content: line.to_string(),
            style: default_style,
        });
    }
    if lines.is_empty() {
        lines.push(MarkdownLine {
            content: String::new(),
            style: default_style,
        });
    }
    lines
}
