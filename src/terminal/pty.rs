use portable_pty::{PtySize, CommandBuilder, Child, MasterPty, SlavePty};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::mpsc;
use parking_lot::Mutex;

pub struct TerminalProcess {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    _slave: Option<Box<dyn SlavePty + Send>>,
    child: Arc<Mutex<Option<Box<dyn Child + Send + Sync>>>>,
}

impl TerminalProcess {
    pub fn spawn(
        shell: &str,
        cols: u16,
        rows: u16,
    ) -> Result<
        (
            Self,
            mpsc::UnboundedReceiver<Vec<u8>>,
            mpsc::UnboundedReceiver<()>,
        ),
        String,
    > {
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("PTY: {}", e))?;

        let mut cmd = CommandBuilder::new(shell);
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("CLICOLOR", "1");
        cmd.env("CLICOLOR_FORCE", "1");
        let child = pair.slave.spawn_command(cmd)
            .map_err(|e| format!("spawn: {}", e))?;

        let writer = pair.master.take_writer()
            .map_err(|e| format!("writer: {}", e))?;

        let master = Arc::new(Mutex::new(pair.master));
        let writer_arc = Arc::new(Mutex::new(writer));
        let child = Arc::new(Mutex::new(Some(child)));
        let (tx, rx) = mpsc::unbounded_channel();
        let (redraw_tx, redraw_rx) = mpsc::unbounded_channel();

        let master_clone = master.clone();
        let child_clone = child.clone();
        let redraw_tx_clone = redraw_tx.clone();
        std::thread::spawn(move || {
            let mut r = match master_clone.lock().try_clone_reader() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[tflow-pty] reader clone failed: {e}");
                    return;
                }
            };
            let mut buf = vec![0u8; 16384];
            loop {
                match r.read(&mut buf) {
                    Ok(0) => {
                        // Pipe break — a child inside the PTY may have exited.
                        // Signal the panel. Do NOT write anything to the PTY.
                        // Do NOT assume the shell exited.
                        let _ = redraw_tx_clone.send(());
                        // Only stop reading if the shell itself has exited.
                        let shell_exited = child_clone.lock()
                            .as_mut()
                            .and_then(|c| c.try_wait().ok())
                            .flatten()
                            .is_some();
                        if shell_exited { break; }
                        // Shell still alive — but the pipe is at EOF.  On
                        // Windows ConPTY, subsequent reads return 0 immediately,
                        // creating a busy-loop.  Sleep before retrying so the
                        // DSR nudge from the event loop has time to unblock it.
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if tx.send(data).is_err() { break; }
                    }
                    Err(_) => {
                        // r.read() returned an error — on Windows ConPTY this
                        // often happens when a child TUI (opencode, vim, …)
                        // exits, NOT because cmd.exe died.  If the shell is
                        // still alive we must NOT break (which drops tx and
                        // triggers Disconnected → shell-exited in the panel).
                        // Instead, enter a polling loop that checks try_wait
                        // periodically so tx stays open and the session lives.
                        let shell_exited = child_clone.lock()
                            .as_mut()
                            .and_then(|c| c.try_wait().ok())
                            .flatten()
                            .is_some();
                        if !shell_exited {
                            loop {
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                let se = child_clone.lock()
                                    .as_mut()
                                    .and_then(|c| c.try_wait().ok())
                                    .flatten()
                                    .is_some();
                                if se { break; }
                            }
                        }
                        break;
                    }
                }
            }
        });

        Ok((TerminalProcess {
            writer: writer_arc,
            master,
            _slave: Some(pair.slave),
            child,
        }, rx, redraw_rx))
    }

    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        self.writer.lock().write_all(data).map_err(|e| format!("write: {}", e))
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Some(m) = self.master.try_lock() {
            let _ = m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
    }

    pub fn close(&mut self) {
        *self.child.lock() = None;
        self._slave = None;
    }
}

impl Drop for TerminalProcess {
    fn drop(&mut self) {
        if let Some(ref mut child) = *self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
