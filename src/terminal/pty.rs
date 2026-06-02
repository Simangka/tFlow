use portable_pty::{PtySize, CommandBuilder, Child, MasterPty, SlavePty};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::thread::JoinHandle;
use tokio::sync::mpsc;
use parking_lot::Mutex;

const ALLOWED_SHELLS: &[&str] = &[
    "/bin/sh", "/bin/bash", "/usr/bin/bash", "/usr/bin/zsh", "/bin/zsh",
    "/usr/bin/fish", "/bin/fish",
    "cmd.exe", "cmd", "powershell.exe", "powershell", "pwsh.exe", "pwsh",
];

const PTY_CHANNEL_CAPACITY: usize = 1024;

pub struct TerminalProcess {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    _slave: Option<Box<dyn SlavePty + Send>>,
    child: Arc<Mutex<Option<Box<dyn Child + Send + Sync>>>>,
    stop_flag: Arc<AtomicBool>,
    reader_handle: Option<JoinHandle<()>>,
    dropped_bytes: Arc<AtomicU64>,
}

impl TerminalProcess {
    pub fn spawn(
        shell: &str,
        cols: u16,
        rows: u16,
    ) -> Result<
        (
            Self,
            mpsc::Receiver<Vec<u8>>,
            mpsc::Receiver<()>,
        ),
        String,
    > {
        let shell_name = std::path::Path::new(shell)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(shell);
        let allowed = ALLOWED_SHELLS.iter().any(|&a| {
            let an = std::path::Path::new(a)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(a);
            an.eq_ignore_ascii_case(shell_name) || a == shell
        });
        if !allowed {
            return Err(format!("shell '{}' not in allow-list", shell));
        }

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
        let (tx, rx) = mpsc::channel(PTY_CHANNEL_CAPACITY);
        let (redraw_tx, redraw_rx) = mpsc::channel(PTY_CHANNEL_CAPACITY);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let dropped_bytes = Arc::new(AtomicU64::new(0));

        let master_clone = master.clone();
        let redraw_tx_clone = redraw_tx.clone();
        let stop_flag_clone = stop_flag.clone();
        let dropped_bytes_clone = dropped_bytes.clone();
        let reader_handle = std::thread::spawn(move || {
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
                if stop_flag_clone.load(Ordering::SeqCst) {
                    break;
                }
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
                                let _ = redraw_tx_clone.try_send(());
                            }
                            pipe_was_broken = false;
                        }
                        zero_count = 0;
                        zero_start = None;
                        let data = buf[..n].to_vec();
                        match tx.try_send(data) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                dropped_bytes_clone.fetch_add(n as u64, Ordering::Relaxed);
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                break;
                            }
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
            stop_flag,
            reader_handle: Some(reader_handle),
            dropped_bytes,
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

    pub fn dropped_bytes(&self) -> u64 {
        self.dropped_bytes.load(Ordering::Relaxed)
    }
}

impl Drop for TerminalProcess {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(ref mut child) = *self.child.lock() {
            let _ = child.kill();
            let start = Instant::now();
            while start.elapsed() < Duration::from_millis(500) {
                if let Ok(Some(_)) = child.try_wait() { break; }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}
