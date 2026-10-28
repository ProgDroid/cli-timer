use crate::sound::{self, Sound};
use chrono::{DateTime, Duration, Local};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use rodio::Player;
use std::{error, path::PathBuf, sync::mpsc::Sender, thread, time::Duration as StdDuration};

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
    let index: u8 = rand::random_range(0..15);

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
    pub fn render(&self, frame: &mut Frame) {
        let seconds = self.time_left.num_seconds().abs() % 60;
        let minutes = self.time_left.num_minutes().abs() % 60;
        let hours = self.time_left.num_hours().abs();

        let area = frame.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    // ratatui rounds percentages where tui floored them; Length keeps tui's row at
                    // every height. It is height * 49 / 100 split up so it cannot overflow a u16.
                    Constraint::Length(area.height / 100 * 49 + area.height % 100 * 49 / 100),
                    Constraint::Length(1),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(area);

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
        let source = sound::alarm_loop(&self.sound)?;

        let (tx, rx) = std::sync::mpsc::channel();

        self.sender = Some(tx);

        thread::spawn(move || {
            let output = match sound::open_output() {
                Ok(output) => output,
                Err(e) => {
                    eprintln!("Could not open output stream: {e}");
                    return;
                }
            };

            let player = Player::connect_new(output.mixer());
            player.append(sound::fit_channels(source, output.config().channel_count()));

            // Blocks until the app sends a stop, or drops the sender on restart or exit; either
            // way the answer is the same, so the result is not looked at.
            let _stop = rx.recv();
            player.stop();
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn random_color_is_one_of_the_fifteen() {
        const FIFTEEN: [Color; 15] = [
            Color::Red,
            Color::Green,
            Color::Yellow,
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
            Color::Gray,
            Color::DarkGray,
            Color::LightRed,
            Color::LightGreen,
            Color::LightYellow,
            Color::LightBlue,
            Color::LightMagenta,
            Color::LightCyan,
            Color::White,
        ];
        let mut seen = Vec::new();
        for _ in 0..500 {
            let colour = random_color();
            assert!(FIFTEEN.contains(&colour), "unexpected colour {colour:?}");
            if !seen.contains(&colour) {
                seen.push(colour);
            }
        }
        // 500 draws from 15 colours: a single colour is astronomically unlikely.
        assert!(seen.len() > 1, "random_color never varied: {seen:?}");
    }

    fn app_at(state: State, time_left: Duration) -> App {
        App {
            state,
            time_left,
            colour: Color::Cyan,
            ..App::default()
        }
    }

    /// Draws `app` on a `w` x `h` test terminal; one string per screen row.
    fn screen(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buffer[(x, y)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect()
    }

    /// The `(fg, bg)` of the cell at (`x`, `y`) on a `w` x `h` test terminal.
    fn cell_at(app: &App, w: u16, h: u16, x: u16, y: u16) -> (Color, Color) {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let cell = terminal.backend().buffer()[(x, y)].clone();
        (cell.fg, cell.bg)
    }

    #[test]
    fn time_row_is_49_percent_down_rounded_down() {
        for (height, row) in [(24_u16, 11_usize), (25, 12), (40, 19)] {
            let rows = screen(&app_at(State::Paused, Duration::seconds(5)), 80, height);
            assert_eq!(rows[row].trim(), "00:00:05", "height {height}");
        }
        for height in 2..=80_u16 {
            let rows = screen(&app_at(State::Paused, Duration::seconds(5)), 80, height);
            let row = usize::from(height * 49 / 100);
            assert_eq!(rows[row].trim(), "00:00:05", "time row at height {height}");
            assert_eq!(
                rows[row + 1].trim(),
                "Paused",
                "line under at height {height}"
            );
        }
    }

    #[test]
    fn running_shows_the_time_centred() {
        let time_left = Duration::hours(1) + Duration::minutes(2) + Duration::seconds(3);
        let rows = screen(&app_at(State::Running, time_left), 80, 24);
        assert_eq!(rows[11].trim(), "01:02:03");
        // The time string carries a one-space prefix (a `-` when triggered), which is
        // part of the centred text.
        let left = rows[11].len() - rows[11].trim_start().len() - 1;
        let right = rows[11].len() - rows[11].trim_end().len();
        assert!(left.abs_diff(right) <= 1, "left {left}, right {right}");
        assert_eq!(rows[12].trim(), "");
    }

    #[test]
    fn triggered_shows_minus_and_the_label() {
        let mut app = app_at(State::Triggered, Duration::seconds(-65));
        app.message = Some("stretch".to_owned());
        let rows = screen(&app, 80, 24);
        assert_eq!(rows[11].trim(), "-00:01:05");
        assert_eq!(rows[12].trim(), "stretch");
    }

    #[test]
    fn paused_shows_paused() {
        let rows = screen(&app_at(State::Paused, Duration::seconds(5)), 80, 24);
        assert_eq!(rows[12].trim(), "Paused");
    }

    #[test]
    fn restart_prompt_text() {
        let rows = screen(&app_at(State::Restart, Duration::seconds(5)), 80, 24);
        assert!(
            rows[12]
                .trim_start()
                .starts_with("Are you sure you want to restart the timer?"),
            "row 12 was {:?}",
            rows[12]
        );
    }

    #[test]
    fn time_uses_the_colour_on_black() {
        let app = app_at(State::Running, Duration::seconds(5));
        let rows = screen(&app, 80, 24);
        let x = u16::try_from(rows[11].len() - rows[11].trim_start().len()).unwrap();
        assert_eq!(cell_at(&app, 80, 24, x, 11), (Color::Cyan, Color::Black));
        assert_eq!(cell_at(&app, 80, 24, 0, 0).1, Color::Black);
    }

    #[test]
    fn three_digit_hours_render_in_full() {
        let rows = screen(&app_at(State::Running, Duration::hours(100)), 80, 24);
        assert_eq!(rows[11].trim(), "100:00:00");
    }

    #[test]
    fn a_2000_row_terminal_renders_with_the_time_49_percent_down() {
        // 2000 * 49 overflows u16, which panics in a debug build.
        let rows = screen(&app_at(State::Paused, Duration::seconds(5)), 80, 2000);
        assert_eq!(rows[980].trim(), "00:00:05");
        assert_eq!(rows[981].trim(), "Paused");
    }

    #[test]
    fn renders_in_a_tiny_terminal() {
        let app = app_at(State::Paused, Duration::seconds(5));
        let _small = screen(&app, 10, 2);
        let _tiny = screen(&app, 1, 1);
    }
}
