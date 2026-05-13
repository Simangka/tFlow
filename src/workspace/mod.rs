pub mod file_tree;
pub mod search;

pub use file_tree::FileTree;
pub use search::WorkspaceSearcher;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub depth: usize,
    pub expanded: bool,
    pub children: Vec<FileEntry>,
    pub is_gitignored: bool,
    pub size: u64,
    pub modified: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}
