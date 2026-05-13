use crate::core::{Position, Range};

#[derive(Debug, Clone)]
pub enum ChangeKind {
    Insert { pos: Position, text: String },
    Delete { pos: Position, text: String, range: Range },
    Replace { range: Range, old: String, new: String },
    Indent { line: usize },
    Unindent { line: usize },
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub changes: Vec<ChangeKind>,
    pub timestamp: std::time::Instant,
    pub cursor_before: Position,
    pub cursor_after: Position,
}

#[derive(Debug)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
    pub index: isize,
    pub max_entries: usize,
    pub group_timeout_ms: u64,
    pub last_entry_time: Option<std::time::Instant>,
}

impl History {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            index: -1,
            max_entries,
            group_timeout_ms: 2000,
            last_entry_time: None,
        }
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        let should_group = self.group_with_last(&entry);

        if should_group {
            if let Some(last) = self.entries.last_mut() {
                let start_len = last.changes.len();
                last.changes.extend(entry.changes);
                last.cursor_after = entry.cursor_after;
                last.timestamp = std::time::Instant::now();
                if start_len < last.changes.len() {
                    self.last_entry_time = Some(std::time::Instant::now());
                }
                return;
            }
        }

        let truncate_at = (self.index + 1) as usize;
        if truncate_at < self.entries.len() {
            self.entries.truncate(truncate_at);
        }

        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
            self.index = (self.entries.len() as isize) - 1;
        }

        self.entries.push(entry);
        self.index = (self.entries.len() - 1) as isize;
        self.last_entry_time = Some(std::time::Instant::now());
    }

    pub fn can_undo(&self) -> bool {
        self.index >= 0 && !self.entries.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        (self.index as usize + 1) < self.entries.len()
    }

    pub fn undo(&mut self) -> Option<&HistoryEntry> {
        if !self.can_undo() {
            return None;
        }
        let entry = &self.entries[self.index as usize];
        self.index -= 1;
        Some(entry)
    }

    pub fn redo(&mut self) -> Option<&HistoryEntry> {
        if !self.can_redo() {
            return None;
        }
        self.index += 1;
        Some(&self.entries[self.index as usize])
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index = -1;
        self.last_entry_time = None;
    }

    pub fn group_with_last(&self, _entry: &HistoryEntry) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let last_time = match self.last_entry_time {
            Some(t) => t,
            None => return false,
        };
        let elapsed = last_time.elapsed().as_millis() as u64;
        elapsed < self.group_timeout_ms
    }
}
