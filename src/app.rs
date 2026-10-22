use crate::sound::{self, Sound};
use chrono::{DateTime, Duration, Local};
use rand::{Rng, thread_rng};
use rodio::{OutputStream, Sink, Source};
use std::{error, path::PathBuf, sync::mpsc::Sender, thread, time::Duration as StdDuration};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    terminal::Frame,
    widgets::{Block, Borders, Paragraph},
};

pub type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum State {
    Running,
    Paused,
    Triggered,
    Restart,
}

pub struct App {
    pub running: bool,
    pub state: State,
    pub pre_pause_state: Option<State>,
    pub duration: Duration,
    pub time_left: Duration,
    pub end_time: DateTime<Local>,
    pub colour: Color,
    pub message: Option<String>,
    pub sound: Sound,
    pub sender: Option<Sender<()>>,
}

fn random_color() -> Color {
    let mut rng = thread_rng();

    let index: u8 = rng.gen_range(0..15);

    match index {
        1 => Color::Green,
        2 => Color::Yellow,
        3 => Color::Blue,
        4 => Color::Magenta,
        5 => Color::Cyan,
        6 => Color::Gray,
        7 => Color::DarkGray,
        8 => Color::LightRed,
        9 => Color::LightGreen,
        10 => Color::LightYellow,
        11 => Color::LightBlue,
        12 => Color::LightMagenta,
        13 => Color::LightCyan,
        14 => Color::White,
        _ => Color::Red, // 0 handled here
    }
}

impl Default for App {
    #[allow(clippy::arithmetic_side_effects)]
    fn default() -> Self {
        let duration = Duration::seconds(5);
        let end_time = Local::now() + duration;

        Self {
            running: true,
            state: State::Running,
            pre_pause_state: None,
            duration,
            time_left: duration,
            end_time,
            colour: random_color(),
            message: None,
            sound: Sound::Alarm,
            sender: None,
        }
    }
}

impl App {
    /// Builds the app for a timer of `time`, with an optional sound file and label.
    ///
    /// # Errors
    /// Returns an error if the duration does not fit a `chrono::Duration`; the parser's
    /// `MAX_DURATION` cap makes that unreachable in practice.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn new(time: StdDuration, sound: Option<PathBuf>, label: Option<String>) -> Result<Self> {
        let time = Duration::from_std(time)?;
        let end_time = Local::now() + time;

        Ok(Self {
            duration: time,
            time_left: time,
            end_time,
            message: label,
            sound: sound.map_or(Sound::Alarm, Sound::File),
            ..Self::default()
        })
    }

    #[allow(clippy::arithmetic_side_effects)]
    pub fn tick(&mut self) {
        match self.state {
            State::Paused => {
                self.end_time = Local::now() + self.time_left;
            }
            State::Running | State::Restart => {
                self.time_left = self.end_time.signed_duration_since(Local::now());

                if self.time_left <= Duration::zero() {
                    if let Err(e) = self.start_sound() {
                        eprintln!("Error playing sound: {e}");
                    }

                    self.state = State::Triggered;
                }
            }
            State::Triggered => {
                self.time_left = self.end_time.signed_duration_since(Local::now());
            }
        }
    }

    #[allow(clippy::modulo_arithmetic, clippy::indexing_slicing)]
    pub fn render<B: Backend>(&self, frame: &mut Frame<'_, B>) {
        let seconds = self.time_left.num_seconds().abs() % 60;
        let minutes = self.time_left.num_minutes().abs() % 60;
        let hours = self.time_left.num_hours().abs();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(49),
                    Constraint::Length(1),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(frame.size());

        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Black)),
            layout[0],
        );

        let time_prefix = if self.state == State::Triggered {
            "-"
        } else {
            " "
        };
        let time_string = format!("{time_prefix}{hours:0>2}:{minutes:0>2}:{seconds:0>2}");

        frame.render_widget(
            Paragraph::new(time_string)
                .block(Block::default().borders(Borders::NONE))
                .style(Style::default().fg(self.colour).bg(Color::Black))
                .alignment(Alignment::Center),
            layout[1],
        );

        let widget = match self.state {
            State::Paused | State::Restart | State::Triggered => {
                let paragraph_string = match self.state {
                    State::Paused => " Paused",
                    State::Restart => {
                        " Are you sure you want to restart the timer? (Press again to confirm, Esc/q to cancel)"
                    }
                    State::Triggered => self.message.as_ref().map_or("", |message| message),
                    State::Running => "",
                };

                Paragraph::new(paragraph_string)
                    .block(Block::default().borders(Borders::NONE))
                    .style(Style::default().fg(self.colour).bg(Color::Black))
                    .alignment(Alignment::Center)
            }
            State::Running => {
                Paragraph::new("").block(Block::default().style(Style::default().bg(Color::Black)))
            }
        };

        frame.render_widget(widget, layout[2]);
    }

    #[allow(clippy::arithmetic_side_effects)]
    pub fn restart(&mut self) {
        let end_time = Local::now() + self.duration + Duration::seconds(1);

        self.state = State::Running;
        self.pre_pause_state = None;
        self.time_left = self.duration;
        self.end_time = end_time;

        if let Some(tx) = &self.sender {
            let _result = tx.send(());
        }

        self.sender = None;
    }

    /// Starts looping the sound on a background thread, until the thread is told to stop.
    ///
    /// # Errors
    /// Returns an error if the sound file cannot be opened or decoded.
    pub fn start_sound(&mut self) -> Result<()> {
        let source = sound::source(&self.sound)?
            .repeat_infinite()
            .fade_in(StdDuration::from_millis(500));

        let (tx, rx) = std::sync::mpsc::channel();

        self.sender = Some(tx);

        thread::spawn(move || {
            let (_stream, handle) = match OutputStream::try_default() {
                Ok((stream, handle)) => (stream, handle),
                Err(e) => {
                    eprintln!("Could not open output stream: {e}");
                    return;
                }
            };

            let sink = match Sink::try_new(&handle) {
                Ok(sink) => sink,
                Err(e) => {
                    eprintln!("Could not create Sink in sound thread: {e}");
                    return;
                }
            };

            sink.append(source);

            // Blocks until the app sends a stop, or drops the sender on restart or exit; either
            // way the answer is the same, so the result is not looked at.
            let _stop = rx.recv();
            sink.stop();
        });

        Ok(())
    }
}
