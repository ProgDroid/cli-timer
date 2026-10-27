use crate::app::{App, Result, State};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Applies a key press to the app state.
///
/// # Errors
/// Never fails at present; the `Result` leaves room for handlers that can.
pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> Result<()> {
    // Windows reports key releases from crossterm 0.26 on; act on presses only.
    if key_event.kind != KeyEventKind::Press {
        return Ok(());
    }
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
                app.state = app.pre_pause_state.unwrap_or(State::Running);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with_kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)
    }

    fn app_in(state: State) -> App {
        App {
            state,
            ..App::default()
        }
    }

    fn send(app: &mut App, event: KeyEvent) {
        handle_key_events(event, app).unwrap();
    }

    #[test]
    fn space_pauses_a_running_timer() {
        let mut app = app_in(State::Running);
        send(&mut app, press(KeyCode::Char(' ')));
        assert!(app.state == State::Paused);
        assert!(app.running);
    }

    #[test]
    fn space_resumes_a_paused_timer() {
        let mut app = app_in(State::Paused);
        app.pre_pause_state = Some(State::Running);
        send(&mut app, press(KeyCode::Char(' ')));
        assert!(app.state == State::Running);
        assert!(app.pre_pause_state.is_none());
    }

    #[test]
    fn r_asks_before_restarting() {
        let mut app = app_in(State::Running);
        send(&mut app, press(KeyCode::Char('r')));
        assert!(app.state == State::Restart);
    }

    #[test]
    fn r_twice_restarts() {
        let mut app = app_in(State::Running);
        send(&mut app, press(KeyCode::Char('r')));
        send(&mut app, press(KeyCode::Char('r')));
        assert!(app.state == State::Running);
    }

    #[test]
    fn esc_on_the_restart_prompt_cancels_without_quitting() {
        let mut app = app_in(State::Restart);
        send(&mut app, press(KeyCode::Esc));
        assert!(app.state == State::Running);
        assert!(app.running);
    }

    #[test]
    fn q_quits() {
        let mut app = app_in(State::Running);
        send(&mut app, press(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn ctrl_c_quits_and_plain_c_does_not() {
        let mut app = app_in(State::Running);
        send(&mut app, press(KeyCode::Char('c')));
        assert!(app.running);
        send(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(!app.running);
    }

    #[test]
    fn release_events_change_nothing() {
        for start in [State::Running, State::Paused, State::Restart] {
            for code in [
                KeyCode::Char(' '),
                KeyCode::Char('r'),
                KeyCode::Esc,
                KeyCode::Char('q'),
            ] {
                let mut app = app_in(start);
                send(&mut app, with_kind(code, KeyEventKind::Release));
                assert!(app.state == start, "{code:?} released changed the state");
                assert!(app.running, "{code:?} released stopped the app");
            }
        }
    }

    #[test]
    fn repeat_events_change_nothing() {
        for code in [KeyCode::Char(' '), KeyCode::Char('r'), KeyCode::Char('q')] {
            let mut app = app_in(State::Running);
            send(&mut app, with_kind(code, KeyEventKind::Repeat));
            assert!(
                app.state == State::Running,
                "{code:?} repeat changed the state"
            );
            assert!(app.running, "{code:?} repeat stopped the app");
        }
    }
}
