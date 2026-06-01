use portable_pty::{PtySize, CommandBuilder, Child, MasterPty, SlavePty};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
        let child = Arc::new(Mutex::new(Some(child)));
        let (tx, rx) = mpsc::unbounded_channel();
        let (redraw_tx, redraw_rx) = mpsc::unbounded_channel();

        let master_clone = master.clone();
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
            let mut zero_count: u32 = 0;
            let mut zero_start: Option<Instant> = None;
            let mut pipe_was_broken = false;
            loop {
                match r.read(&mut buf) {
                    Ok(0) => {
                        // ConPTY pipe broken (child process exited).
                        pipe_was_broken = true;
                        zero_count += 1;
                        if zero_start.is_none() {
                            zero_start = Some(Instant::now());
                        }

                        // Hard timeout: pipe broken too long → force break.
                        if zero_start.map_or(false, |t| t.elapsed() > Duration::from_secs(5)) {
                            break;
                        }

                        // NOTE: an earlier draft had a `try_wait()` liveness
                        // check here that broke out of the loop as soon as
                        // ConPTY reported the shell as exited. On Windows,
                        // ConPTY has a habit of briefly reporting the parent
                        // shell (cmd.exe) as exited when a child TUI (opencode,
                        // vim, …) shuts down — even though the shell is still
                        // alive and ready to accept more commands. Breaking
                        // out of the loop in that case would show a misleading
                        // "[shell exited]" message and force the user to
                        // restart the shell. We now rely solely on the hard
                        // 5-second timeout above, which is long enough to
                        // ride out ConPTY's transient exit reports but short
                        // enough that a truly dead shell is still detected
                        // quickly.

                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Ok(n) => {
                        if pipe_was_broken {
                            // Check whether the break was long enough to indicate a real
                            // child process exit (filtering out momentary ConPTY quirks).
                            let break_duration = zero_start
                                .map_or(Duration::ZERO, |t| t.elapsed());
                            if break_duration > Duration::from_millis(200) {
                                // Pipe recovered after a meaningful break — most likely a
                                // child TUI process (opencode, vim, less, etc.) just exited.
                                // Signal the panel to force-exit the alternate screen buffer
                                // so the shell prompt becomes visible immediately.
                                let _ = redraw_tx_clone.send(());
                            }
                            pipe_was_broken = false;
                        }
                        zero_count = 0;
                        zero_start = None;
                        let data = buf[..n].to_vec();
                        if tx.send(data).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        Ok((TerminalProcess {
            writer: Arc::new(Mutex::new(writer)),
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
