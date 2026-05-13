use crossterm::event::{Event, EventStream, KeyEvent, KeyEventKind, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    FocusGained,
    FocusLost,
    Paste(String),
    Tick,
}

#[derive(Debug)]
pub struct InputHandler {
    pub tx: mpsc::UnboundedSender<InputEvent>,
    pub rx: mpsc::UnboundedReceiver<InputEvent>,
    pub mouse_enabled: bool,
    pub focus_change_enabled: bool,
    pub bracketed_paste_enabled: bool,
}

impl InputHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            mouse_enabled: false,
            focus_change_enabled: false,
            bracketed_paste_enabled: false,
        }
    }

    pub fn start_reading(&self) -> tokio::task::JoinHandle<()> {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let tick_interval = Duration::from_millis(50);
            let mut tick_timer = tokio::time::interval(tick_interval);
            loop {
                tokio::select! {
                    maybe_event = reader.next() => {
                        match maybe_event {
                            Some(Ok(event)) => {
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
                                if tx.send(input_event).is_err() {
                                    break;
                                }
                            }
                            Some(Err(_)) => {
                                continue;
                            }
                            None => {
                                break;
                            }
                        }
                    }
                    _ = tick_timer.tick() => {
                        if tx.send(InputEvent::Tick).is_err() {
                            break;
                        }
                    }
                }
            }
        })
    }

    pub fn send(&self, event: InputEvent) {
        let _ = self.tx.send(event);
    }

    pub async fn recv(&mut self) -> Option<InputEvent> {
        self.rx.recv().await
    }

    pub fn enable_mouse(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    pub fn disable_mouse(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    pub fn enable_focus_change(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    pub fn disable_focus_change(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    pub fn enable_bracketed_paste(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    pub fn disable_bracketed_paste(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
