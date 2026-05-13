use crate::core::{Position, Range, EditMode};
use ropey::Rope;
use std::path::PathBuf;

pub struct Buffer {
    pub id: usize,
    pub path: Option<PathBuf>,
    pub name: String,
    pub rope: Rope,
    pub dirty: bool,
    pub cursor: Position,
    pub saved_cursor: Position,
    pub mode: EditMode,
    pub visual_start: Option<Position>,
    pub scroll_offset: Position,
    pub modified_at: Option<std::time::SystemTime>,
    pub line_endings: LineEnding,
    pub encoding: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl Buffer {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            path: None,
            name: format!("[{}] Untitled", id),
            rope: Rope::new(),
            dirty: false,
            cursor: Position::zero(),
            saved_cursor: Position::zero(),
            mode: EditMode::Normal,
            visual_start: None,
            scroll_offset: Position::zero(),
            modified_at: None,
            line_endings: LineEnding::Lf,
            encoding: "utf-8".to_string(),
            readonly: false,
        }
    }

    pub fn from_string(id: usize, content: String) -> Self {
        let mut buf = Self::new(id);
        let (cleaned, ending) = Self::clean_line_endings(&content);
        buf.rope = Rope::from_str(&cleaned);
        buf.line_endings = ending;
        buf.name = format!("[{}] Untitled", id);
        buf
    }

    pub fn from_path(id: usize, path: PathBuf) -> Result<Self, anyhow::Error> {
        let mut buf = Self::new(id);
        buf.path = Some(path.clone());
        buf.name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("[{}] Unknown", id));
        buf.load()?;
        Ok(buf)
    }

    pub fn load(&mut self) -> Result<(), anyhow::Error> {
        let path = self.path.as_ref().ok_or_else(|| anyhow::anyhow!("No path set"))?;
        let content = std::fs::read_to_string(path)?;
        let (cleaned, ending) = Self::clean_line_endings(&content);
        self.rope = Rope::from_str(&cleaned);
        self.line_endings = ending;
        self.dirty = false;
        self.modified_at = path.metadata().ok().and_then(|m| m.modified().ok());
        self.cursor = Position::zero();
        self.saved_cursor = Position::zero();
        self.scroll_offset = Position::zero();
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), anyhow::Error> {
        let path = self.path.as_ref().ok_or_else(|| anyhow::anyhow!("No path set"))?;
        let text = self.rope.to_string();
        let output = match self.line_endings {
            LineEnding::Lf => text,
            LineEnding::CrLf => text.replace('\n', "\r\n"),
            LineEnding::Cr => text.replace('\n', "\r"),
        };
        std::fs::write(path, output.as_bytes())?;
        self.dirty = false;
        self.modified_at = Some(std::time::SystemTime::now());
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), anyhow::Error> {
        self.path = Some(path.clone());
        self.name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        self.save()
    }

    fn pos_to_char_idx(&self, pos: Position) -> usize {
        let line_start = self.rope.line_to_char(pos.line);
        let line_len = self.rope.line(pos.line).len_chars();
        let col = if pos.column > line_len { line_len } else { pos.column };
        line_start + col
    }

    fn clean_line_endings(content: &str) -> (String, LineEnding) {
        let ending = Self::detect_line_endings_from_str(content);
        let cleaned = match ending {
            LineEnding::CrLf => content.replace("\r\n", "\n"),
            LineEnding::Cr => content.replace('\r', "\n"),
            LineEnding::Lf => content.to_string(),
        };
        (cleaned, ending)
    }

    fn detect_line_endings_from_str(content: &str) -> LineEnding {
        if content.contains("\r\n") {
            LineEnding::CrLf
        } else if content.contains('\r') {
            LineEnding::Cr
        } else {
            LineEnding::Lf
        }
    }

    pub fn chars_at_line(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        self.rope.line(line).len_chars()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn total_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line_to_str(&self, line: usize) -> String {
        if line >= self.rope.len_lines() {
            return String::new();
        }
        self.rope.line(line).to_string()
    }

    pub fn char_at(&self, pos: Position) -> Option<char> {
        if !self.valid_position(pos) {
            return None;
        }
        let idx = self.pos_to_char_idx(pos);
        if idx >= self.rope.len_chars() {
            return None;
        }
        self.rope.chars_at(idx).next()
    }

    pub fn insert_char(&mut self, pos: Position, c: char) {
        let idx = self.pos_to_char_idx(pos);
        self.rope.insert_char(idx, c);
    }

    pub fn insert_str(&mut self, pos: Position, s: &str) {
        let idx = self.pos_to_char_idx(pos);
        self.rope.insert(idx, s);
    }

    pub fn delete_char(&mut self, pos: Position) -> Option<char> {
        if !self.valid_position(pos) {
            return None;
        }
        let idx = self.pos_to_char_idx(pos);
        if idx >= self.rope.len_chars() {
            return None;
        }
        let c = self.rope.chars_at(idx).next()?;
        self.rope.remove(idx..idx + 1);
        Some(c)
    }

    pub fn delete_range(&mut self, range: Range) -> String {
        let norm = range.normalized();
        let start_idx = self.pos_to_char_idx(norm.start);
        let end_idx = self.pos_to_char_idx(norm.end);
        if end_idx <= start_idx {
            return String::new();
        }
        let deleted = self.rope.slice(start_idx..end_idx).to_string();
        self.rope.remove(start_idx..end_idx);
        deleted
    }

    pub fn delete_backward(&mut self, pos: Position) -> Option<char> {
        if pos.column == 0 && pos.line == 0 {
            return None;
        }
        let idx = self.pos_to_char_idx(pos);
        if idx == 0 {
            return None;
        }
        let prev_idx = idx - 1;
        let c = self.rope.chars_at(prev_idx).next()?;
        self.rope.remove(prev_idx..idx);
        Some(c)
    }

    pub fn insert_newline(&mut self, pos: Position) {
        let idx = self.pos_to_char_idx(pos);
        self.rope.insert_char(idx, '\n');
    }

    pub fn get_line(&self, idx: usize) -> String {
        if idx >= self.rope.len_lines() {
            return String::new();
        }
        self.rope.line(idx).to_string()
    }

    pub fn get_text(&self) -> String {
        self.rope.to_string()
    }

    pub fn get_text_in_range(&self, range: Range) -> String {
        let norm = range.normalized();
        let start_idx = self.pos_to_char_idx(norm.start);
        let end_idx = self.pos_to_char_idx(norm.end);
        if end_idx <= start_idx {
            return String::new();
        }
        self.rope.slice(start_idx..end_idx).to_string()
    }

    pub fn indent_level(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_str = self.rope.line(line).to_string();
        let mut count = 0;
        for c in line_str.chars() {
            if c == ' ' {
                count += 1;
            } else if c == '\t' {
                count += 4;
            } else {
                break;
            }
        }
        count
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    pub fn clamp_position(&self, pos: Position) -> Position {
        let max_line = self.rope.len_lines().saturating_sub(1);
        let line = pos.line.min(max_line);
        let line_len = self.rope.line(line).len_chars();
        let column = pos.column.min(line_len);
        Position::new(line, column)
    }

    pub fn valid_position(&self, pos: Position) -> bool {
        if pos.line >= self.rope.len_lines() {
            return false;
        }
        let line_len = self.rope.line(pos.line).len_chars();
        pos.column <= line_len
    }

    pub fn detect_line_endings(&self) -> LineEnding {
        let text = self.rope.to_string();
        if text.contains("\r\n") {
            LineEnding::CrLf
        } else if text.contains('\r') {
            LineEnding::Cr
        } else {
            LineEnding::Lf
        }
    }

    pub fn set_modified(&mut self) {
        self.dirty = true;
        self.modified_at = Some(std::time::SystemTime::now());
    }

    pub fn clear_modified(&mut self) {
        self.dirty = false;
    }
}
