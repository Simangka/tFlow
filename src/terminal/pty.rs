use portable_pty::{PtySize, CommandBuilder, Child};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::mpsc;
use parking_lot::Mutex;

pub struct TerminalProcess {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    pub child_exited: Arc<std::sync::atomic::AtomicBool>,
}

impl TerminalProcess {
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<(Self, mpsc::UnboundedReceiver<Vec<u8>>), String> {
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("PTY: {}", e))?;

        let cmd = CommandBuilder::new(shell);
        let child = pair.slave.spawn_command(cmd)
            .map_err(|e| format!("spawn: {}", e))?;

        let reader = pair.master.try_clone_reader()
            .map_err(|e| format!("reader: {}", e))?;
        let writer = pair.master.take_writer()
            .map_err(|e| format!("writer: {}", e))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let exited = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let exit = exited.clone();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            let mut r = reader;
            loop {
                match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if tx.send(data).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            exit.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        Ok((TerminalProcess {
            writer: Arc::new(Mutex::new(writer)),
            master: Arc::new(Mutex::new(pair.master)),
            child: Some(child),
            child_exited: exited,
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

    pub fn is_alive(&self) -> bool {
        !self.child_exited.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn check_exit(&mut self) -> Option<u32> {
        if let Some(ref mut child) = self.child {
            if let Ok(Some(status)) = child.try_wait() {
                self.child_exited.store(true, std::sync::atomic::Ordering::SeqCst);
                return Some(status.exit_code());
            }
        }
        None
    }
}

impl Drop for TerminalProcess {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
