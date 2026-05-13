pub mod task_queue;

pub use task_queue::TaskQueue;

use std::path::PathBuf;

pub type TaskId = u64;

#[derive(Debug, Clone)]
pub enum Task {
    FileWatch(PathBuf),
    Autosave(usize),
    WorkspaceSearch { root: PathBuf, query: String, id: TaskId },
    SyntaxHighlight { buffer_id: usize, lines: Vec<String> },
    MarkdownRender { buffer_id: usize, text: String },
    FileIndex { root: PathBuf },
    CrashRecovery,
    Lint,
}

#[derive(Debug, Clone)]
pub enum TaskResult {
    FileWatchResult(Vec<crate::core::types::Notification>),
    AutosaveResult { success: bool, error: Option<String> },
    WorkspaceSearchResult { id: TaskId, results: Vec<crate::workspace::SearchResult> },
    SyntaxHighlightResult { buffer_id: usize, highlights: Vec<Vec<(String, ratatui::style::Style)>> },
    MarkdownRenderResult { buffer_id: usize, lines: Vec<MarkdownLine> },
    FileIndexResult { root: PathBuf, entries: Vec<crate::workspace::FileEntry> },
    CrashRecoveryResult(Option<Vec<RecoveryFile>>),
    LintResult(Vec<crate::core::types::Notification>),
}

#[derive(Debug, Clone)]
pub struct RecoveryFile {
    pub original_path: Option<PathBuf>,
    pub recovery_path: PathBuf,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct MarkdownLine {
    pub content: String,
    pub style: ratatui::style::Style,
}
