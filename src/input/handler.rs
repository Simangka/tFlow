use crossterm::event::{DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste, EnableFocusChange, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use crossterm::execute;
use futures::StreamExt;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const TICK_INTERVAL_MS: u64 = 50;
const CHORD_TIMEOUT_MS: u64 = 1000;
const ERROR_BACKOFF_MS: u64 = 100;
const MAX_CONSECUTIVE_ERRORS: u32 = 10;
const TICK_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    FocusGained,
    FocusLost,
    Paste(String),
    Tick,
    ChordTimeout(KeyCode),
}

#[derive(Debug)]
pub struct InputHandler {
    pub tx: mpsc::Sender<InputEvent>,
    pub rx: mpsc::Receiver<InputEvent>,
    pub mouse_enabled: bool,
    pub focus_change_enabled: bool,
    pub bracketed_paste_enabled: bool,
    pub tick_enabled: Arc<AtomicBool>,
    chord_state: Arc<Mutex<Option<(KeyCode, Instant)>>>,
}

impl InputHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(TICK_CHANNEL_CAPACITY);
        Self {
            tx,
            rx,
            mouse_enabled: false,
            focus_change_enabled: false,
            bracketed_paste_enabled: false,
            tick_enabled: Arc::new(AtomicBool::new(true)),
            chord_state: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start_reading(&self) -> tokio::task::JoinHandle<()> {
        let tx = self.tx.clone();
        let tick_enabled = self.tick_enabled.clone();
        let chord_state = self.chord_state.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let tick_interval = Duration::from_millis(TICK_INTERVAL_MS);
            let mut tick_timer = tokio::time::interval(tick_interval);
            tick_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut last_tick_sent: Option<Instant> = None;
            let mut consecutive_errors: u32 = 0;
            loop {
                tokio::select! {
                    maybe_event = reader.next() => {
                        match maybe_event {
                            Some(Ok(event)) => {
                                consecutive_errors = 0;
                                let input_event = match event {
                                    Event::Key(key) => {
                                        if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                                            InputEvent::Key(key)
                                        } else {
                                            continue;
                                        }
                                    }
                                    Event::Mouse(mouse) => InputEvent::Mouse(mouse),
                                    Event::Resize(w, h) => InputEvent::Resize(w, h),
                                    Event::FocusGained => InputEvent::FocusGained,
                                    Event::FocusLost => InputEvent::FocusLost,
                                    Event::Paste(data) => InputEvent::Paste(data),
                                };
                                if tx.send(input_event).await.is_err() {
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                tracing::warn!("input stream error: {}", e);
                                tokio::time::sleep(Duration::from_millis(ERROR_BACKOFF_MS)).await;
                                consecutive_errors += 1;
                                if consecutive_errors > MAX_CONSECUTIVE_ERRORS {
                                    tracing::error!("input stream failed permanently after {} errors", consecutive_errors);
                                    break;
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                    _ = tick_timer.tick() => {
                        if !tick_enabled.load(Ordering::Relaxed) {
                            continue;
                        }
                        let now = Instant::now();
                        if let Some(prev) = last_tick_sent {
                            if now.duration_since(prev) < tick_interval {
                                continue;
                            }
                        }
                        let expired_chord = {
                            let mut cs = chord_state.lock().unwrap();
                            match *cs {
                                Some((code, started)) if now.duration_since(started) > Duration::from_millis(CHORD_TIMEOUT_MS) => {
                                    *cs = None;
                                    Some(code)
                                }
                                _ => None,
                            }
                        };
                        if let Some(code) = expired_chord {
                            if tx.send(InputEvent::ChordTimeout(code)).await.is_err() {
                                break;
                            }
                        }
                        if tx.send(InputEvent::Tick).await.is_err() {
                            break;
                        }
                        last_tick_sent = Some(now);
                    }
                }
            }
        })
    }

    pub fn send(&self, event: InputEvent) {
        let _ = self.tx.try_send(event);
    }

    pub async fn recv(&mut self) -> Option<InputEvent> {
        self.rx.recv().await
    }

    pub fn set_chord_prefix(&self, key: KeyCode) {
        let mut cs = self.chord_state.lock().unwrap();
        *cs = Some((key, Instant::now()));
    }

    pub fn clear_chord_prefix(&self) {
        let mut cs = self.chord_state.lock().unwrap();
        *cs = None;
    }

    pub fn set_tick_enabled(&self, enabled: bool) {
        self.tick_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enable_mouse(&self) -> Result<(), anyhow::Error> {
        execute!(stdout(), EnableMouseCapture)?;
        Ok(())
    }

    pub fn disable_mouse(&self) -> Result<(), anyhow::Error> {
        execute!(stdout(), DisableMouseCapture)?;
        Ok(())
    }

    pub fn enable_focus_change(&self) -> Result<(), anyhow::Error> {
        execute!(stdout(), EnableFocusChange)?;
        Ok(())
    }

    pub fn disable_focus_change(&self) -> Result<(), anyhow::Error> {
        execute!(stdout(), DisableFocusChange)?;
        Ok(())
    }

    pub fn enable_bracketed_paste(&self) -> Result<(), anyhow::Error> {
        execute!(stdout(), EnableBracketedPaste)?;
        Ok(())
    }

    pub fn disable_bracketed_paste(&self) -> Result<(), anyhow::Error> {
        execute!(stdout(), DisableBracketedPaste)?;
        Ok(())
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
