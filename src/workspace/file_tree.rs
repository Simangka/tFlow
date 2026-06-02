use std::path::{Path, PathBuf};
use std::rc::Rc;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use crate::workspace::FileEntry;

const MAX_TREE_DEPTH: usize = 32;
const MAX_ENTRIES_PER_DIR: usize = 50_000;
const TRUNCATION_SENTINEL_NAME: &str = "[... truncated]";
const DEFAULT_EXPAND_CAP: usize = 1000;

pub struct TreeDisplayEntry {
    pub entry: Rc<FileEntry>,
    pub connector: String,
    pub icon: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct FileTree {
    pub root: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub show_hidden: bool,
    pub respect_gitignore: bool,
    pub filter: Option<String>,
    pub follow_symlinks: bool,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        FileTree {
            root: root.clone(),
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            show_hidden: false,
            respect_gitignore: true,
            filter: None,
            follow_symlinks: false,
        }
    }

    pub fn refresh(&mut self) -> Result<(), anyhow::Error> {
        let entries = Self::build_tree_with(&self.root, 0, self.show_hidden, self.respect_gitignore, self.follow_symlinks)?;
        self.entries = entries;
        if self.selected >= self.visible_entries().len() {
            self.selected = self.visible_entries().len().saturating_sub(1);
        }
        Ok(())
    }

    pub fn navigate_up(&mut self) -> bool {
        let visible = self.visible_entries();
        if visible.is_empty() || self.selected == 0 {
            return false;
        }
        let prev = self.selected;
        self.selected = self.selected.saturating_sub(1);
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        prev != self.selected
    }

    pub fn navigate_down(&mut self) -> bool {
        let visible = self.visible_entries();
        if visible.is_empty() || self.selected >= visible.len().saturating_sub(1) {
            return false;
        }
        let prev = self.selected;
        self.selected = self.selected.saturating_add(1).min(visible.len().saturating_sub(1));
        if self.selected >= self.scroll_offset + 20 {
            self.scroll_offset = self.selected.saturating_sub(20) + 1;
        }
        prev != self.selected
    }

    pub fn toggle_expand(&mut self) -> bool {
        let visible = self.visible_entries();
        if visible.is_empty() || self.selected >= visible.len() {
            return false;
        }
        let entry = visible[self.selected];
        if !Self::is_expandable(entry) {
            return false;
        }
        let entry_path = entry.path.clone();
        let show_hidden = self.show_hidden;
        let respect_gitignore = self.respect_gitignore;
        let follow_symlinks = self.follow_symlinks;
        if let Some(entry_mut) = self.find_entry_mut(&entry_path) {
            let was_expanded = entry_mut.expanded;
            entry_mut.expanded = !was_expanded;
            if entry_mut.expanded && entry_mut.children.is_empty() {
                match Self::build_tree_with(&entry_mut.path, entry_mut.depth + 1, show_hidden, respect_gitignore, follow_symlinks) {
                    Ok(children) => entry_mut.children = children,
                    Err(_) => {
                        entry_mut.expanded = false;
                    }
                }
            }
            return true;
        }
        false
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        let visible = self.visible_entries();
        if visible.is_empty() || self.selected >= visible.len() {
            return None;
        }
        Some(visible[self.selected].path.clone())
    }

    pub fn collapse_all(&mut self) {
        let entries = &mut self.entries;
        Self::collapse_recursive(entries);
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn collapse_recursive(entries: &mut [FileEntry]) {
        for entry in entries.iter_mut() {
            entry.expanded = false;
            Self::collapse_recursive(&mut entry.children);
        }
    }

    pub fn expand_all(&mut self) {
        self.expand_all_capped(DEFAULT_EXPAND_CAP);
    }

    pub fn expand_all_capped(&mut self, cap: usize) {
        let show_hidden = self.show_hidden;
        let respect_gitignore = self.respect_gitignore;
        let follow_symlinks = self.follow_symlinks;
        let entries = &mut self.entries;
        let mut count: usize = 0;
        Self::expand_recursive(entries, 0, cap, &mut count, show_hidden, respect_gitignore, follow_symlinks);
    }

    fn expand_recursive(entries: &mut [FileEntry], depth: usize, cap: usize, count: &mut usize, show_hidden: bool, respect_gitignore: bool, follow_symlinks: bool) {
        if depth >= MAX_TREE_DEPTH || *count >= cap {
            return;
        }
        for entry in entries.iter_mut() {
            if *count >= cap {
                break;
            }
            if entry.is_dir {
                entry.expanded = true;
                *count += 1;
                if entry.children.is_empty() {
                    if let Ok(children) = Self::build_tree_with(&entry.path, entry.depth + 1, show_hidden, respect_gitignore, follow_symlinks) {
                        entry.children = children;
                    }
                }
                Self::expand_recursive(&mut entry.children, depth + 1, cap, count, show_hidden, respect_gitignore, follow_symlinks);
            }
        }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = std::fs::canonicalize(&root).unwrap_or(root);
        self.selected = 0;
        self.scroll_offset = 0;
        let _ = self.refresh();
    }

    pub fn set_filter(&mut self, filter: Option<String>) {
        self.filter = filter;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        let _ = self.refresh();
    }

    pub fn visible_entries(&self) -> Vec<&FileEntry> {
        let mut result = Vec::new();
        let filter_lower = self.filter.as_ref().map(|f| f.to_lowercase());
        Self::collect_visible(&self.entries, &mut result, &filter_lower);
        result
    }

    fn collect_visible<'a>(entries: &'a [FileEntry], result: &mut Vec<&'a FileEntry>, filter: &Option<String>) {
        for entry in entries {
            let matches_filter = match filter {
                Some(f) => entry.name.to_lowercase().contains(f),
                None => true,
            };
            if matches_filter {
                result.push(entry);
            }
            if entry.expanded {
                Self::collect_visible(&entry.children, result, filter);
            }
        }
    }

    fn find_entry_mut(&mut self, path: &Path) -> Option<&mut FileEntry> {
        Self::find_entry_recursive(&mut self.entries, path)
    }

    fn find_entry_recursive<'a>(entries: &'a mut [FileEntry], path: &Path) -> Option<&'a mut FileEntry> {
        for entry in entries.iter_mut() {
            if paths_eq(&entry.path, path) {
                return Some(entry);
            }
            if entry.expanded {
                if let Some(found) = Self::find_entry_recursive(&mut entry.children, path) {
                    return Some(found);
                }
            }
        }
        None
    }

    pub fn build_tree(path: &Path, depth: usize, show_hidden: bool, respect_gitignore: bool) -> Result<Vec<FileEntry>, anyhow::Error> {
        Self::build_tree_with(path, depth, show_hidden, respect_gitignore, false)
    }

    pub fn build_tree_with(path: &Path, depth: usize, show_hidden: bool, respect_gitignore: bool, follow_symlinks: bool) -> Result<Vec<FileEntry>, anyhow::Error> {
        if depth >= MAX_TREE_DEPTH {
            return Ok(Vec::new());
        }

        let matchers = if respect_gitignore {
            build_gitignore_matchers_for(path)
        } else {
            Vec::new()
        };

        let read_dir = match std::fs::read_dir(path) {
            Ok(rd) => rd,
            Err(_) => return Ok(Vec::new()),
        };

        let mut dir_entries: Vec<_> = read_dir
            .filter_map(|e| e.ok())
            .filter(|e| {
                if show_hidden {
                    return true;
                }
                !is_hidden_entry(e)
            })
            .collect();

        let truncated = dir_entries.len() > MAX_ENTRIES_PER_DIR;
        if truncated {
            dir_entries.truncate(MAX_ENTRIES_PER_DIR);
        }

        dir_entries.sort_by_key(|e| {
            let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            (!is_dir, e.file_name().to_os_string())
        });

        let mut entries: Vec<FileEntry> = Vec::new();

        for entry in dir_entries {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if file_name == ".git" {
                continue;
            }

            let file_type = entry.file_type();
            let raw_is_dir = file_type.as_ref().map(|ft| ft.is_dir()).unwrap_or(false);
            let is_symlink = file_type.as_ref().map(|ft| ft.is_symlink()).unwrap_or(false);
            let effective_is_dir = if !follow_symlinks && is_symlink { false } else { raw_is_dir };

            let is_gitignored = if matchers.is_empty() {
                false
            } else {
                matchers.iter().any(|m| m.matched(&entry_path, effective_is_dir).is_ignore())
            };

            let metadata = entry.metadata().ok();
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = metadata.and_then(|m| m.modified().ok()).unwrap_or_else(|| std::time::SystemTime::UNIX_EPOCH);

            let children = if effective_is_dir {
                match Self::build_tree_with(&entry_path, depth + 1, show_hidden, respect_gitignore, follow_symlinks) {
                    Ok(child_entries) => child_entries,
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            };

            entries.push(FileEntry {
                path: entry_path,
                name: file_name,
                is_dir: effective_is_dir,
                is_symlink,
                depth,
                expanded: false,
                children,
                is_gitignored,
                size,
                modified,
            });
        }

        if truncated {
            entries.push(FileEntry {
                path: path.join(TRUNCATION_SENTINEL_NAME),
                name: TRUNCATION_SENTINEL_NAME.to_string(),
                is_dir: false,
                is_symlink: false,
                depth,
                expanded: false,
                children: Vec::new(),
                is_gitignored: false,
                size: 0,
                modified: std::time::SystemTime::UNIX_EPOCH,
            });
        }

        Ok(entries)
    }

    pub async fn build_tree_async(path: PathBuf, depth: usize, show_hidden: bool, respect_gitignore: bool, follow_symlinks: bool) -> Result<Vec<FileEntry>, anyhow::Error> {
        tokio::task::spawn_blocking(move || {
            Self::build_tree_with(&path, depth, show_hidden, respect_gitignore, follow_symlinks)
        })
        .await
        .map_err(|e| anyhow::anyhow!("build_tree_async join error: {}", e))?
    }

    pub fn is_expandable(entry: &FileEntry) -> bool {
        entry.is_dir
    }

    pub fn display_entries(&self) -> Vec<TreeDisplayEntry> {
        let mut result = Vec::new();
        Self::build_display(&self.entries, Vec::new(), &mut result);
        result
    }

    fn build_display(entries: &[FileEntry], ancestry: Vec<bool>, result: &mut Vec<TreeDisplayEntry>) {
        let count = entries.len();
        for (i, entry) in entries.iter().enumerate() {
            let is_last = i == count - 1;
            let mut connector = String::new();
            for &has_more in &ancestry {
                if has_more {
                    connector.push_str("\u{2502}  ");
                } else {
                    connector.push_str("   ");
                }
            }
            if is_last {
                connector.push_str("\u{2514}\u{2500}\u{2500}");
            } else {
                connector.push_str("\u{251C}\u{2500}\u{2500}");
            }

            let icon = if entry.is_dir {
                if entry.expanded { "\u{25BE} " } else { "\u{25B8} " }
            } else {
                Self::file_icon(&entry.name)
            };

            let display_name = if entry.is_symlink {
                format!("{}@", entry.name)
            } else {
                entry.name.clone()
            };

            let rc = Rc::new(entry.clone());
            result.push(TreeDisplayEntry {
                entry: rc.clone(),
                connector,
                icon: icon.to_string(),
                display_name,
            });

            if entry.expanded && !entry.children.is_empty() {
                let mut child_ancestry = ancestry.clone();
                child_ancestry.push(!is_last);
                Self::build_display(&rc.children, child_ancestry, result);
            }
        }
    }

    pub fn file_icon(name: &str) -> &'static str {
        let ext = name.rsplit('.').next().unwrap_or("");
        match ext {
            "rs" => "\u{2699} ",
            "py" => "\u{25C9} ",
            "js" | "jsx" => "\u{2731} ",
            "ts" | "tsx" => "\u{2731} ",
            "md" | "markdown" => "\u{2714} ",
            "json" => "\u{2774} ",
            "toml" => "\u{2699} ",
            "yaml" | "yml" => "\u{2630} ",
            "css" | "scss" | "less" => "\u{2726} ",
            "html" => "\u{2731} ",
            "sh" | "bash" | "zsh" => "\u{269B} ",
            "txt" => "\u{270E} ",
            "gitignore" | "dockerignore" => "\u{2691} ",
            _ => "  ",
        }
    }

    pub fn icon_for(entry: &FileEntry) -> &'static str {
        if entry.is_dir {
            if entry.expanded {
                "[-]"
            } else {
                "[+]"
            }
        } else {
            " "
        }
    }
}

fn build_gitignore_matchers_for(path: &Path) -> Vec<Gitignore> {
    let mut matchers: Vec<Gitignore> = Vec::new();
    let mut current = Some(path.to_path_buf());
    while let Some(p) = current {
        let gi = p.join(".gitignore");
        if gi.exists() {
            let mut builder = GitignoreBuilder::new(&p);
            let _ = builder.add(&gi);
            if let Ok(m) = builder.build() {
                matchers.push(m);
            }
        }
        current = p.parent().map(|x| x.to_path_buf());
    }
    if let Some(home) = dirs::home_dir() {
        let global_path = home.join(".config").join("git").join("ignore");
        if global_path.exists() {
            let mut builder = GitignoreBuilder::new(&home);
            let _ = builder.add(&global_path);
            if let Ok(m) = builder.build() {
                matchers.push(m);
            }
        }
    }
    matchers
}

fn is_hidden_entry(entry: &std::fs::DirEntry) -> bool {
    let name_os = entry.file_name();
    let lossy = name_os.to_string_lossy();
    if lossy.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if let Ok(md) = entry.metadata() {
            if md.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
                return true;
            }
        }
    }
    false
}

fn paths_eq(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}
