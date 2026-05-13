use std::path::{Path, PathBuf};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use crate::workspace::FileEntry;

#[derive(Debug, Clone)]
pub struct FileTree {
    pub root: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub show_hidden: bool,
    pub respect_gitignore: bool,
    pub filter: Option<String>,
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
        }
    }

    pub fn refresh(&mut self) -> Result<(), anyhow::Error> {
        let entries = Self::build_tree(&self.root, 0, self.show_hidden, self.respect_gitignore)?;
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
        if let Some(entry_mut) = self.find_entry_mut(&entry_path) {
            let was_expanded = entry_mut.expanded;
            entry_mut.expanded = !was_expanded;
            if entry_mut.expanded && entry_mut.children.is_empty() {
                match Self::build_tree(&entry_mut.path, entry_mut.depth + 1, show_hidden, respect_gitignore) {
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
        let show_hidden = self.show_hidden;
        let respect_gitignore = self.respect_gitignore;
        let entries = &mut self.entries;
        Self::expand_recursive(entries, show_hidden, respect_gitignore);
    }

    fn expand_recursive(entries: &mut [FileEntry], show_hidden: bool, respect_gitignore: bool) {
        for entry in entries.iter_mut() {
            if entry.is_dir {
                entry.expanded = true;
                if entry.children.is_empty() {
                    if let Ok(children) = Self::build_tree(&entry.path, entry.depth + 1, show_hidden, respect_gitignore) {
                        entry.children = children;
                    }
                }
                Self::expand_recursive(&mut entry.children, show_hidden, respect_gitignore);
            }
        }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = root;
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
            if entry.path == path {
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
        let mut entries = Vec::new();
        let gitignore_matcher = if respect_gitignore {
            build_gitignore_matcher(path)
        } else {
            None
        };

        let read_dir = match std::fs::read_dir(path) {
            Ok(rd) => rd,
            Err(_) => return Ok(entries),
        };

        let mut dir_entries: Vec<_> = read_dir
            .filter_map(|e| e.ok())
            .filter(|e| {
                if show_hidden {
                    return true;
                }
                if let Some(name) = e.file_name().to_str() {
                    !name.starts_with('.')
                } else {
                    true
                }
            })
            .collect();

        dir_entries.sort_by_key(|e| {
            let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            (!is_dir, e.file_name().to_os_string())
        });

        for entry in dir_entries {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if file_name == ".git" && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                continue;
            }

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let is_symlink = entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);
            let is_gitignored = gitignore_matcher.as_ref().map(|m| m.matched(&entry_path, is_dir).is_ignore()).unwrap_or(false);

            let metadata = entry.metadata().ok();
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = metadata.and_then(|m| m.modified().ok()).unwrap_or_else(|| std::time::SystemTime::UNIX_EPOCH);

            let children = if is_dir {
                match Self::build_tree(&entry_path, depth + 1, show_hidden, respect_gitignore) {
                    Ok(child_entries) => child_entries,
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            };

            entries.push(FileEntry {
                path: entry_path,
                name: file_name,
                is_dir,
                is_symlink,
                depth,
                expanded: false,
                children,
                is_gitignored,
                size,
                modified,
            });
        }

        Ok(entries)
    }

    pub fn is_expandable(entry: &FileEntry) -> bool {
        entry.is_dir
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

fn build_gitignore_matcher(root: &Path) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(root);
    let gitignore_path = root.join(".gitignore");
    if gitignore_path.exists() {
        let _ = builder.add(gitignore_path);
    }
    let global_gitignore = dirs::home_dir().map(|h| h.join(".config").join("git").join("ignore"));
    if let Some(global_path) = global_gitignore {
        if global_path.exists() {
            let _ = builder.add(global_path);
        }
    }
    builder.build().ok()
}
