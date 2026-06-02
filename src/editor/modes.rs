use crate::core::EditMode;

#[derive(Debug, Clone)]
pub struct EditorMode {
    pub mode: EditMode,
    pub previous_mode: Option<EditMode>,
    pub command_buffer: String,
    pub search_buffer: String,
    pub message: Option<String>,
}

impl EditorMode {
    pub fn new() -> Self {
        Self {
            mode: EditMode::Normal,
            previous_mode: None,
            command_buffer: String::new(),
            search_buffer: String::new(),
            message: None,
        }
    }

    pub fn set(&mut self, mode: EditMode) {
        self.previous_mode = Some(self.mode);
        self.mode = mode;
    }

    pub fn restore(&mut self) -> EditMode {
        let prev = self.previous_mode.unwrap_or(EditMode::Normal);
        self.mode = prev;
        self.previous_mode = None;
        prev
    }

    pub fn switch_to_insert(&mut self) {
        self.set(EditMode::Insert);
    }

    pub fn switch_to_normal(&mut self) {
        self.mode = EditMode::Normal;
        self.previous_mode = None;
        self.command_buffer.clear();
        self.search_buffer.clear();
    }

    pub fn switch_to_visual(&mut self) {
        self.set(EditMode::Visual);
    }

    pub fn switch_to_visual_line(&mut self) {
        self.set(EditMode::VisualLine);
    }

    pub fn switch_to_command(&mut self) {
        self.set(EditMode::Command);
        self.command_buffer.clear();
    }

    pub fn switch_to_search(&mut self) {
        self.set(EditMode::Search);
        self.search_buffer.clear();
    }

    pub fn push_command_char(&mut self, c: char) -> bool {
        if c == '\n' || c == '\r' {
            let complete = !self.command_buffer.is_empty();
            return complete;
        }
        if c == '\u{7f}' || c == '\x08' {
            self.command_buffer.pop();
            return false;
        }
        self.command_buffer.push(c);
        false
    }

    pub fn take_command(&mut self) -> Option<String> {
        if self.command_buffer.is_empty() {
            return None;
        }
        let buf = std::mem::take(&mut self.command_buffer);
        Some(buf)
    }

    pub fn take_search(&mut self) -> Option<String> {
        if self.search_buffer.is_empty() {
            return None;
        }
        let buf = std::mem::take(&mut self.search_buffer);
        Some(buf)
    }

    pub fn clear_command(&mut self) {
        self.command_buffer.clear();
    }

    pub fn is_insert(&self) -> bool {
        self.mode.is_insert()
    }

    pub fn is_normal(&self) -> bool {
        self.mode.is_normal()
    }

    pub fn is_visual(&self) -> bool {
        self.mode.is_visual()
    }
}
