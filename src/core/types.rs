use crate::core::position::Position;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditMode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    Search,
}

impl EditMode {
    pub fn is_insert(&self) -> bool {
        matches!(self, EditMode::Insert)
    }

    pub fn is_normal(&self) -> bool {
        matches!(self, EditMode::Normal)
    }

    pub fn is_visual(&self) -> bool {
        matches!(self, EditMode::Visual | EditMode::VisualLine)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    Left,
    Right,
    Up,
    Down,
    StartOfLine,
    EndOfLine,
    StartOfFile,
    EndOfFile,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    WordForward,
    WordBackward,
    LineStart,
    LineEnd,
    MatchingBrace,
}

pub type BufferId = usize;

#[derive(Debug, Clone)]
pub struct BufferInfo {
    pub id: BufferId,
    pub path: Option<PathBuf>,
    pub name: String,
    pub is_dirty: bool,
    pub is_modified: bool,
    pub line_count: usize,
    pub cursor: Position,
    pub mode: EditMode,
}

#[derive(Debug, Clone)]
pub enum WorkspaceItem {
    File(PathBuf),
    Directory(PathBuf, Vec<WorkspaceItem>),
}

#[derive(Debug, Clone)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Success,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: std::time::Instant,
    pub duration: std::time::Duration,
}

impl Notification {
    pub fn new(message: String, level: NotificationLevel) -> Self {
        Self {
            message,
            level,
            timestamp: std::time::Instant::now(),
            duration: std::time::Duration::from_secs(3),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationLevel::Info)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationLevel::Error)
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationLevel::Success)
    }

    pub fn expired(&self) -> bool {
        self.timestamp.elapsed() > self.duration
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub direction: SearchDirection,
    pub case_sensitive: bool,
    pub is_regex: bool,
    pub matches: Vec<Position>,
    pub current_match: Option<usize>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            direction: SearchDirection::Forward,
            case_sensitive: false,
            is_regex: false,
            matches: Vec::new(),
            current_match: None,
        }
    }
}
