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
    pub last_insert: Option<std::time::Instant>,
}

impl History {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            index: -1,
            max_entries,
            group_timeout_ms: 2000,
            last_entry_time: None,
            last_insert: None,
        }
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        let truncate_at = (self.index + 1) as usize;
        if truncate_at < self.entries.len() {
            self.entries.truncate(truncate_at);
        }
        self.last_entry_time = self.entries.last().map(|e| e.timestamp);
        let last_is_single_insert = matches!(
            self.entries.last(),
            Some(e) if e.changes.len() == 1 && matches!(e.changes[0], ChangeKind::Insert { .. })
        );
        self.last_insert = if last_is_single_insert {
            self.entries.last().map(|e| e.timestamp)
        } else {
            None
        };

        let now = std::time::Instant::now();
        let is_insert = entry.changes.len() == 1
            && matches!(entry.changes[0], ChangeKind::Insert { .. });

        if is_insert && self.try_coalesce_insert(&entry, now) {
            return;
        }

        let should_group = self.group_with_last(&entry);

        if should_group {
            if let Some(last) = self.entries.last_mut() {
                let start_len = last.changes.len();
                last.changes.extend(entry.changes);
                last.cursor_after = entry.cursor_after;
                last.timestamp = now;
                if start_len < last.changes.len() {
                    self.last_entry_time = Some(now);
                }
                self.last_insert = None;
                return;
            }
        }

        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
            self.index = (self.entries.len() as isize) - 1;
        }

        self.entries.push(entry);
        self.index = (self.entries.len() - 1) as isize;
        self.last_entry_time = Some(now);
        self.last_insert = if is_insert { Some(now) } else { None };
    }

    fn try_coalesce_insert(&mut self, entry: &HistoryEntry, now: std::time::Instant) -> bool {
        let last_insert_time = match self.last_insert {
            Some(t) => t,
            None => return false,
        };
        if last_insert_time.elapsed().as_millis() >= 500 {
            return false;
        }
        let (new_pos, new_text) = match &entry.changes[0] {
            ChangeKind::Insert { pos, text } => (*pos, text.clone()),
            _ => return false,
        };
        let last = match self.entries.last_mut() {
            Some(e) => e,
            None => return false,
        };
        if last.changes.len() != 1 {
            return false;
        }
        match &mut last.changes[0] {
            ChangeKind::Insert { pos, text } => {
                let end = end_of_insert(*pos, text);
                if end != new_pos {
                    return false;
                }
                text.push_str(&new_text);
                last.cursor_after = entry.cursor_after;
                last.timestamp = now;
                self.last_entry_time = Some(now);
                self.last_insert = Some(now);
                true
            }
            _ => false,
        }
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
        self.last_insert = None;
    }

    pub fn peek_last(&self) -> Option<&HistoryEntry> {
        self.entries.last()
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

pub fn end_of_insert(pos: Position, text: &str) -> Position {
    let mut line = pos.line;
    let mut col = pos.column;
    for c in text.chars() {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position { line, column: col }
}
