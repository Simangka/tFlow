use crate::git::status::{StatusEntry, HunkInfo};
use crate::git::GitManager;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum StagingEntry {
    File(StatusEntry),
    Hunk { file: String, hunk: HunkInfo, staged: bool },
}

#[derive(Debug, Clone)]
pub struct StagingPanel {
    pub visible: bool,
    pub entries: Vec<usize>,
    pub selected: usize,
    pub scroll: usize,
    pub expanded: HashMap<String, bool>,
    pub data: Vec<StagingEntry>,
}

impl StagingPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            expanded: HashMap::new(),
            data: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.selected = 0;
            self.scroll = 0;
        }
    }

    pub fn refresh(&mut self, git: &mut GitManager, file_path: &std::path::Path) {
        let Some(status) = git.get_status(file_path) else { return };
        self.data.clear();
        for entry in &status.entries {
            let is_expanded = self.expanded.get(&entry.path).copied().unwrap_or(false);
            self.data.push(StagingEntry::File(entry.clone()));
            if is_expanded {
                if let Some(diff) = status.diffs.iter().find(|d| d.path == entry.path) {
                    for hunk in &diff.hunks {
                        self.data.push(StagingEntry::Hunk {
                            file: entry.path.clone(),
                            hunk: hunk.clone(),
                            staged: false,
                        });
                    }
                }
            }
        }
        self.entries = (0..self.data.len()).collect();
        self.clamp_cursor();
    }

    pub fn clamp_cursor(&mut self) {
        let max = self.data.len().saturating_sub(1);
        if self.selected > max { self.selected = max; }
        if self.scroll > max { self.scroll = max; }
    }

    pub fn toggle_expand(&mut self, file_path: &str) {
        let current = self.expanded.get(file_path).copied().unwrap_or(false);
        self.expanded.insert(file_path.to_string(), !current);
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 { self.entries.len() - 1 } else { self.selected - 1 };
        }
    }

    pub fn selected_entry(&self) -> Option<&StagingEntry> {
        self.entries.get(self.selected).and_then(|&i| self.data.get(i))
    }
}
