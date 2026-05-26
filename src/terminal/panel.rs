use crate::terminal::pty::TerminalProcess;
use tokio::sync::mpsc;

const MAX_SCROLLBACK: usize = 5000;

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalPosition {
    Bottom,
    Top,
    Right,
}

impl Default for TerminalPosition {
    fn default() -> Self {
        TerminalPosition::Right
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalState {
    Running,
    Exited(String),
}

pub struct TerminalInstance {
    pub process: TerminalProcess,
    pub rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub parser: vt100::Parser,
    pub title: String,
    pub scroll_offset: usize,
    pub shell: String,
    pub state: TerminalState,
}

impl TerminalInstance {
    fn new(shell: &str, cols: u16, rows: u16, title: String) -> Result<Self, String> {
        let (process, rx) = TerminalProcess::spawn(shell, cols, rows)?;
        Ok(TerminalInstance {
            process,
            rx,
            parser: vt100::Parser::new(rows.max(1), cols.max(10), MAX_SCROLLBACK),
            title,
            scroll_offset: 0,
            shell: shell.to_string(),
            state: TerminalState::Running,
        })
    }

    pub fn restart(&mut self) -> Result<(), String> {
        let (cur_rows, cur_cols) = self.parser.screen().size();
        let cols = cur_cols.max(10);
        let rows = cur_rows.max(1);
        let (process, rx) = TerminalProcess::spawn(&self.shell, cols, rows)?;
        self.process = process;
        self.rx = rx;
        self.parser = vt100::Parser::new(rows, cols, MAX_SCROLLBACK);
        self.state = TerminalState::Running;
        self.scroll_offset = 0;
        Ok(())
    }

    pub fn drain_output(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(bytes) if bytes.is_empty() => {
                    self.parser.process(b"\x1b[?1049l\x1b[?25h");
                }
                Ok(bytes) => self.parser.process(&bytes),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Reader thread exited because the PTY pipe was
                    // broken — the shell process has truly exited.
                    // Do NOT use try_wait() here because on ConPTY v2
                    // it can fire prematurely when a subprocess exits.
                    if matches!(self.state, TerminalState::Running) {
                        self.state = TerminalState::Exited("[shell exited]".to_string());
                        self.process.close();
                        self.reset_scroll();
                    }
                    break;
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, TerminalState::Running)
    }

    pub fn exit_message(&self) -> Option<&str> {
        match &self.state {
            TerminalState::Exited(msg) => Some(msg.as_str()),
            TerminalState::Running => None,
        }
    }

    pub fn scroll_up(&mut self) {
        let current = self.parser.screen().scrollback();
        self.parser.set_scrollback(usize::MAX);
        let max = self.parser.screen().scrollback();
        self.parser.set_scrollback(current);
        self.scroll_offset = self.scroll_offset.saturating_add(1).min(max);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn set_size(&mut self, cols: u16, rows: u16) {
        self.parser.set_size(rows, cols);
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
            position: TerminalPosition::Right,
            height: 12,
            width: 100,
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
        let c = self.width.max(10);
        let r = self.height.max(5);
        match TerminalInstance::new(shell, c, r, title.to_string()) {
            Ok(inst) => {
                self.instances.push(inst);
                self.active_idx = self.instances.len() - 1;
            }
            Err(e) => {
                eprintln!("Failed to spawn terminal: {e}");
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

    pub fn resize_active(&mut self, cols: u16, rows: u16) {
        if let Some(inst) = self.instances.get_mut(self.active_idx) {
            let c = cols.max(10);
            let r = rows.max(1);
            let (cur_rows, cur_cols) = inst.parser.screen().size();
            if cur_rows != r || cur_cols != c {
                inst.process.resize(c, r);
                inst.set_size(c, r);
            }
        }
    }

    pub fn restart_active(&mut self) -> bool {
        if let Some(inst) = self.instances.get_mut(self.active_idx) {
            inst.restart().is_ok()
        } else {
            false
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

/// Restart the active terminal and attempt to write data to the new session.
/// Returns `true` if the write succeeded, `false` otherwise.
pub fn retry_write(panel: &mut TerminalPanel, data: &[u8]) -> bool {
    if !panel.restart_active() {
        return false;
    }
    panel.active()
        .map_or(false, |inst| inst.process.write(data).is_ok())
}

/// Suspend tflow and hand the host terminal to a shell.
///
/// The user runs interactive CLI tools (opencode, vim, etc.) directly,
/// then types `exit` to return to tflow. Works on all platforms.
pub fn suspend_to_shell() {
    use std::io::Write;
    use crossterm::terminal::{LeaveAlternateScreen, EnterAlternateScreen};

    let _ = std::io::stdout().flush();
    let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = writeln!(
        std::io::stdout(),
        "\r\n[Suspended. Type 'exit' to return to tFlow.]\r\n"
    );

    let shell: String = if cfg!(windows) {
        "cmd.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    };

    let _ = std::process::Command::new(&shell)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    let _ = writeln!(std::io::stdout(), "\r\n[Resuming tFlow...]\r\n");
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), EnterAlternateScreen);
}
