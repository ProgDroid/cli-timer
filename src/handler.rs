use crate::app::{App, Result, State};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> Result<()> {
    match key_event.code {
        // Exit application on `ESC` or `q`
        KeyCode::Esc | KeyCode::Char('q') => match app.state {
            State::Restart => {
                app.state = State::Running;
            }
            _ => {
                app.running = false;
            }
        },
        // Exit application on `Ctrl-C`
        KeyCode::Char('c' | 'C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.running = false;
            }
        }
        KeyCode::Char(' ') => match app.state {
            State::Running => {
                app.pre_pause_state = Some(app.state);
                app.state = State::Paused;
            }
            State::Paused => {
                app.state = app.pre_pause_state.map_or(State::Running, |s| s);
                app.pre_pause_state = None;
            }
            State::Triggered => {
                app.restart();
            }
            State::Restart => {}
        },
        KeyCode::Char('r' | 'R') => match app.state {
            State::Running => {
                app.state = State::Restart;
            }
            State::Restart | State::Triggered => {
                app.restart();
            }
            State::Paused => {}
        },
        _ => {}
    }
    Ok(())
}
