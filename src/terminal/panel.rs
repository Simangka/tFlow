use std::collections::VecDeque;
use tokio::sync::mpsc;
use crate::terminal::pty::TerminalProcess;

const MAX_SCROLLBACK: usize = 5000;

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalPosition {
    Bottom,
    Top,
    Right,
}

impl Default for TerminalPosition {
    fn default() -> Self {
        TerminalPosition::Bottom
    }
}

pub struct TerminalInstance {
    pub process: TerminalProcess,
    pub rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub scrollback: VecDeque<String>,
    pub title: String,
    pub scroll_offset: usize,
    pub shell: String,
}

impl TerminalInstance {
    fn new(shell: &str, cols: u16, rows: u16, title: String) -> Result<Self, String> {
        let (process, rx) = TerminalProcess::spawn(shell, cols, rows)?;
        Ok(TerminalInstance {
            process,
            rx,
            scrollback: VecDeque::with_capacity(MAX_SCROLLBACK),
            title,
            scroll_offset: 0,
            shell: shell.to_string(),
        })
    }

    pub fn drain_output(&mut self) {
        while let Ok(data) = self.rx.try_recv() {
            if let Ok(text) = String::from_utf8(data) {
                self.push_text(&text);
            }
        }
    }

    fn push_text(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\r' { continue; }
            if c == '\n' {
                if self.scrollback.is_empty() || !self.scrollback.back().map_or(false, |s| !s.is_empty()) {
                    self.scrollback.push_back(String::new());
                } else {
                    self.scrollback.push_back(String::new());
                }
            } else if c == '\x08' {
                if let Some(last) = self.scrollback.back_mut() {
                    last.pop();
                }
            } else {
                if self.scrollback.is_empty() {
                    self.scrollback.push_back(String::new());
                }
                if let Some(last) = self.scrollback.back_mut() {
                    last.push(c);
                }
            }
        }
        while self.scrollback.len() > MAX_SCROLLBACK {
            self.scrollback.pop_front();
        }
    }

    pub fn visible_lines(&self, height: usize) -> Vec<String> {
        let total = self.scrollback.len();
        let start = if self.scroll_offset > 0 {
            let s = total.saturating_sub(height).saturating_sub(self.scroll_offset);
            if s > total { 0 } else { s }
        } else {
            total.saturating_sub(height)
        };
        let end = total.min(start + height);
        (start..end).map(|i| self.scrollback[i].clone()).collect()
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1).min(self.scrollback.len().saturating_sub(1));
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }
}

pub struct TerminalPanel {
    pub visible: bool,
    pub focused: bool,
    pub instances: Vec<TerminalInstance>,
    pub active_idx: usize,
    pub position: TerminalPosition,
    pub height: u16,
    pub width: u16,
}

impl TerminalPanel {
    pub fn new() -> Self {
        TerminalPanel {
            visible: false,
            focused: false,
            instances: Vec::new(),
            active_idx: 0,
            position: TerminalPosition::Bottom,
            height: 12,
            width: 40,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.instances.is_empty() {
            self.spawn_default();
        }
        if !self.visible {
            self.focused = false;
        }
    }

    pub fn focus(&mut self) {
        self.focused = true;
    }

    pub fn unfocus(&mut self) {
        self.focused = false;
    }

    fn spawn_default(&mut self) {
        let shell = if cfg!(windows) { "cmd.exe" } else { "bash" };
        self.spawn(shell, "terminal");
    }

    pub fn spawn(&mut self, shell: &str, title: &str) {
        let c = 80u16;
        let r = self.height.max(5);
        match TerminalInstance::new(shell, c, r, title.to_string()) {
            Ok(inst) => {
                self.instances.push(inst);
                self.active_idx = self.instances.len() - 1;
            }
            Err(e) => {
                eprintln!("Failed to spawn terminal: {}", e);
            }
        }
    }

    pub fn close_active(&mut self) {
        if self.instances.is_empty() { return; }
        self.instances.remove(self.active_idx);
        if !self.instances.is_empty() {
            self.active_idx = self.active_idx.min(self.instances.len() - 1);
        } else {
            self.visible = false;
            self.focused = false;
        }
    }

    pub fn next_instance(&mut self) {
        if self.instances.len() <= 1 { return; }
        self.active_idx = (self.active_idx + 1) % self.instances.len();
        self.reset_scroll();
    }

    pub fn prev_instance(&mut self) {
        if self.instances.len() <= 1 { return; }
        self.active_idx = if self.active_idx == 0 {
            self.instances.len() - 1
        } else {
            self.active_idx - 1
        };
        self.reset_scroll();
    }

    pub fn active(&self) -> Option<&TerminalInstance> {
        self.instances.get(self.active_idx)
    }

    pub fn active_mut(&mut self) -> Option<&mut TerminalInstance> {
        self.instances.get_mut(self.active_idx)
    }

    pub fn write_active(&self, data: &[u8]) -> Result<(), String> {
        if let Some(inst) = self.instances.get(self.active_idx) {
            inst.process.write(data)
        } else {
            Err("No active terminal".to_string())
        }
    }

    pub fn drain_all(&mut self) {
        for inst in &mut self.instances {
            inst.drain_output();
        }
    }

    pub fn reset_scroll(&mut self) {
        if let Some(inst) = self.instances.get_mut(self.active_idx) {
            inst.reset_scroll();
        }
    }

    pub fn scroll_up(&mut self) {
        if let Some(inst) = self.instances.get_mut(self.active_idx) {
            inst.scroll_up();
        }
    }

    pub fn scroll_down(&mut self) {
        if let Some(inst) = self.instances.get_mut(self.active_idx) {
            inst.scroll_down();
        }
    }

    pub fn resize_active(&self, cols: u16, rows: u16) {
        if let Some(inst) = self.instances.get(self.active_idx) {
            inst.process.resize(cols, rows);
        }
    }

    pub fn cycle_position(&mut self) {
        self.position = match self.position {
            TerminalPosition::Bottom => TerminalPosition::Right,
            TerminalPosition::Right => TerminalPosition::Top,
            TerminalPosition::Top => TerminalPosition::Bottom,
        };
    }
}
