use crate::app::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct Handler {
    receiver: mpsc::Receiver<Event>,
}

impl Handler {
    /// Spawns a thread that emits a tick every `tick_rate` milliseconds plus any terminal events.
    ///
    /// The thread is the only sender, so if it stops (terminal read failure, or the receiver is
    /// gone) the channel closes and [`Handler::next`] returns an error instead of blocking forever.
    #[must_use]
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(tick_rate);

                let Ok(has_event) = event::poll(timeout) else {
                    break;
                };

                if has_event {
                    let Ok(terminal_event) = event::read() else {
                        break;
                    };

                    let sent = match terminal_event {
                        CrosstermEvent::Key(e) => sender.send(Event::Key(e)),
                        CrosstermEvent::Mouse(e) => sender.send(Event::Mouse(e)),
                        CrosstermEvent::Resize(w, h) => sender.send(Event::Resize(w, h)),
                        _ => Ok(()),
                    };

                    if sent.is_err() {
                        break;
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if sender.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self { receiver }
    }

    /// Blocks until the next event arrives.
    ///
    /// # Errors
    /// Returns an error if the event thread has stopped.
    pub fn next(&self) -> Result<Event> {
        Ok(self.receiver.recv()?)
    }
}
