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
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<(Self, mpsc::UnboundedReceiver<Vec<u8>>), String> {
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

        let master_clone = master.clone();
        let child_clone = child.clone();
        std::thread::spawn(move || {
            let mut r = match master_clone.lock().try_clone_reader() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[tflow-pty] reader clone failed: {e}");
                    return;
                }
            };
            let mut buf = vec![0u8; 16384];
            let mut drained = false;
            let mut zero_count: u32 = 0;
            let mut zero_start: Option<Instant> = None;
            loop {
                match r.read(&mut buf) {
                    Ok(0) => {
                        // First Ok(0) = ConPTY flush signal.
                        if !drained {
                            drained = true;
                            zero_count = 0;
                            zero_start = None;
                            if tx.send(vec![]).is_err() { break; }
                            continue;
                        }
                        // Subsequent Ok(0)s = potential pipe break (ConPTY v2
                        // terminates the anonymous pipe on subprocess exit).
                        zero_count += 1;
                        if zero_start.is_none() {
                            zero_start = Some(Instant::now());
                        }

                        // Hard timeout: if the pipe has been broken for
                        // >30s without any data, force a restart regardless
                        // of whether the shell is alive.
                        if zero_start.map_or(false, |t| t.elapsed() > Duration::from_secs(30)) {
                            break;
                        }

                        // Liveness check (~10s): see if the shell truly exited.
                        if zero_count >= 100 {
                            let really_gone = child_clone.lock()
                                .as_mut()
                                .and_then(|c| c.try_wait().ok()?)
                                .is_some();
                            if really_gone {
                                break; // shell exited — permanent break
                            }
                            // Shell is still alive — the pipe may recover.
                            // Keep zero_start so the hard timeout keeps ticking.
                            zero_count = 0;
                            drained = false;
                        }

                        if tx.send(vec![]).is_err() { break; }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Ok(n) => {
                        drained = false;
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
        }, rx))
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
