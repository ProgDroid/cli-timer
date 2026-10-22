//! Desktop notifications and sound behind one trait, so the commands can be tested quietly.
//!
//! [`SystemAlerter`] pops a real notification and plays a real sound. [`QuietAlerter`] only
//! writes a line per call to a log, and [`from_env`] picks it when `CLI_TIMER_QUIET` is present and not `0`.

use crate::sound::{self, Playback, Sound, SoundError};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::{self, Write};

/// Title of every notification.
pub const TITLE: &str = "cli-timer";

/// Environment variable that switches to the [`QuietAlerter`].
const QUIET_VAR: &str = "CLI_TIMER_QUIET";

#[derive(Debug, thiserror::Error)]
pub enum AlertError {
    #[error("cannot show notification: {0}")]
    Notify(String),
    #[error(transparent)]
    Sound(#[from] SoundError),
}

/// Something that can show a notification and play a sound.
pub trait Alerter {
    /// Shows a desktop notification.
    ///
    /// # Errors
    /// Returns [`AlertError::Notify`] if the notification could not be shown.
    fn notify(&self, title: &str, body: &str) -> Result<(), AlertError>;

    /// Plays `sound` as `playback` says and blocks until it has finished.
    ///
    /// # Errors
    /// Returns [`AlertError::Sound`] if the sound could not be played.
    fn play(&self, sound: &Sound, playback: Playback) -> Result<(), AlertError>;
}

/// The real thing: a desktop notification and the audio output device.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemAlerter;

impl Alerter for SystemAlerter {
    fn notify(&self, title: &str, body: &str) -> Result<(), AlertError> {
        show_notification(title, body)
    }

    fn play(&self, sound: &Sound, playback: Playback) -> Result<(), AlertError> {
        sound::play(sound, playback)?;
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn show_notification(title: &str, body: &str) -> Result<(), AlertError> {
    notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show()
        .map(|_handle| ())
        .map_err(|e| AlertError::Notify(e.to_string()))
}

/// The `osascript` arguments that show a notification. Title and body are argv items of the
/// script, never spliced into its text, so no quoting can break out of it.
#[cfg(any(target_os = "macos", test))]
fn osascript_args<'a>(title: &'a str, body: &'a str) -> Vec<&'a str> {
    vec![
        "-e",
        "on run argv",
        "-e",
        "display notification (item 2 of argv) with title (item 1 of argv)",
        "-e",
        "end run",
        title,
        body,
    ]
}

// notify-rust is not used on macOS: its `show()` always returns `Ok` there and drops errors, and
// it uses an API deprecated from macOS 14.
#[cfg(target_os = "macos")]
fn show_notification(title: &str, body: &str) -> Result<(), AlertError> {
    let output = std::process::Command::new("osascript")
        .args(osascript_args(title, body))
        .output()
        .map_err(|e| AlertError::Notify(format!("cannot run osascript: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AlertError::Notify(format!(
            "osascript failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// How an alert went.
#[derive(Debug)]
pub enum Outcome {
    /// Notification shown and sound played.
    Ok,
    /// At least one of them failed; the other was still tried.
    Partial(Vec<AlertError>),
}

/// Shows the notification, then plays the sound. A failure of one does not stop the other.
pub fn deliver(alerter: &dyn Alerter, body: &str, sound: &Sound, playback: Playback) -> Outcome {
    let errors: Vec<AlertError> = [alerter.notify(TITLE, body), alerter.play(sound, playback)]
        .into_iter()
        .filter_map(Result::err)
        .collect();

    if errors.is_empty() {
        Outcome::Ok
    } else {
        Outcome::Partial(errors)
    }
}

/// Writes one line per call instead of notifying or making a sound.
#[derive(Debug)]
pub struct QuietAlerter<W: Write> {
    log: RefCell<W>,
}

impl<W: Write> QuietAlerter<W> {
    #[must_use]
    pub const fn new(log: W) -> Self {
        Self {
            log: RefCell::new(log),
        }
    }

    /// Gives back the log.
    #[must_use]
    pub fn into_inner(self) -> W {
        self.log.into_inner()
    }

    fn line(&self, args: std::fmt::Arguments<'_>) -> Result<(), AlertError> {
        writeln!(self.log.borrow_mut(), "{args}")
            .map_err(|e| AlertError::Notify(format!("cannot write the quiet log: {e}")))
    }
}

impl<W: Write> Alerter for QuietAlerter<W> {
    fn notify(&self, title: &str, body: &str) -> Result<(), AlertError> {
        self.line(format_args!("quiet notify: {title}: {body}"))
    }

    fn play(&self, sound: &Sound, playback: Playback) -> Result<(), AlertError> {
        self.line(format_args!("quiet play: {sound:?} {playback:?}"))
    }
}

/// Which alerter a `CLI_TIMER_QUIET` value selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlerterKind {
    Quiet,
    System,
}

/// Quiet when the variable is present with any value other than exactly `0` (empty and
/// non-Unicode values included); the system alerter when it is absent or `0`.
fn kind_for(value: Option<&OsStr>) -> AlerterKind {
    match value {
        Some(v) if v != "0" => AlerterKind::Quiet,
        _ => AlerterKind::System,
    }
}

/// The alerter for a `CLI_TIMER_QUIET` value: a [`QuietAlerter`] on stderr when it is present and
/// not exactly `0` (an empty value counts as present), otherwise the [`SystemAlerter`].
#[must_use]
pub fn from_env_value(value: Option<&OsStr>) -> Box<dyn Alerter> {
    match kind_for(value) {
        AlerterKind::Quiet => Box::new(QuietAlerter::new(io::stderr())),
        AlerterKind::System => Box::new(SystemAlerter),
    }
}

/// The alerter for the current environment: see [`from_env_value`].
#[must_use]
pub fn from_env() -> Box<dyn Alerter> {
    from_env_value(std::env::var_os(QUIET_VAR).as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::{Playback, Sound};
    use std::path::PathBuf;
    use std::time::Duration;

    fn log_of(alerter: QuietAlerter<Vec<u8>>) -> String {
        String::from_utf8(alerter.into_inner()).expect("log is utf-8")
    }

    #[test]
    fn title_is_cli_timer() {
        assert_eq!(TITLE, "cli-timer");
    }

    #[test]
    fn quiet_notify_records_one_exact_line() {
        let alerter = QuietAlerter::new(Vec::new());
        alerter
            .notify("cli-timer", "check the oven")
            .expect("notify");
        assert_eq!(log_of(alerter), "quiet notify: cli-timer: check the oven\n");
    }

    #[test]
    fn quiet_notify_keeps_colons_and_non_ascii_in_the_body() {
        let alerter = QuietAlerter::new(Vec::new());
        alerter.notify("t", "a: b \u{e9}\u{1f355}").expect("notify");
        assert_eq!(log_of(alerter), "quiet notify: t: a: b \u{e9}\u{1f355}\n");
    }

    #[test]
    fn quiet_play_records_sound_and_playback_debug() {
        let alerter = QuietAlerter::new(Vec::new());
        alerter
            .play(
                &Sound::Chime,
                Playback::Once {
                    max: Duration::from_secs(2),
                },
            )
            .expect("play");
        assert_eq!(log_of(alerter), "quiet play: Chime Once { max: 2s }\n");
    }

    #[test]
    fn deliver_notifies_with_the_title_then_plays_and_reports_ok() {
        let alerter = QuietAlerter::new(Vec::new());
        let outcome = deliver(
            &alerter,
            "body",
            &Sound::Chime,
            Playback::Once {
                max: Duration::from_secs(2),
            },
        );
        assert!(matches!(outcome, Outcome::Ok), "{outcome:?}");
        assert_eq!(
            log_of(alerter),
            "quiet notify: cli-timer: body\nquiet play: Chime Once { max: 2s }\n"
        );
    }

    /// Fails both calls, to see that `deliver` still tries both and keeps the order.
    struct AllFail;

    impl Alerter for AllFail {
        fn notify(&self, _title: &str, _body: &str) -> Result<(), AlertError> {
            Err(AlertError::Notify("no bus".into()))
        }

        fn play(&self, _sound: &Sound, _playback: Playback) -> Result<(), AlertError> {
            Err(AlertError::Notify("no speaker".into()))
        }
    }

    #[test]
    fn deliver_collects_both_errors_in_call_order() {
        let outcome = deliver(
            &AllFail,
            "body",
            &Sound::Chime,
            Playback::Once {
                max: Duration::from_secs(2),
            },
        );
        let Outcome::Partial(errors) = outcome else {
            panic!("expected Partial");
        };
        let texts: Vec<String> = errors.iter().map(ToString::to_string).collect();
        assert_eq!(texts.len(), 2);
        assert!(texts[0].contains("no bus"), "{texts:?}");
        assert!(texts[1].contains("no speaker"), "{texts:?}");
    }

    #[test]
    fn quiet_play_records_loops_and_files() {
        let alerter = QuietAlerter::new(Vec::new());
        alerter
            .play(
                &Sound::File(PathBuf::from("bell.wav")),
                Playback::Loop {
                    for_: Duration::from_secs(30),
                },
            )
            .expect("play");
        assert_eq!(
            log_of(alerter),
            "quiet play: File(\"bell.wav\") Loop { for_: 30s }\n"
        );
    }

    #[test]
    fn quiet_alerter_appends_one_line_per_call_in_order() {
        let alerter = QuietAlerter::new(Vec::new());
        alerter.notify("a", "b").expect("notify");
        alerter
            .play(
                &Sound::Alarm,
                Playback::Loop {
                    for_: Duration::from_secs(1),
                },
            )
            .expect("play");
        alerter.notify("c", "d").expect("notify");
        assert_eq!(
            log_of(alerter),
            "quiet notify: a: b\nquiet play: Alarm Loop { for_: 1s }\nquiet notify: c: d\n"
        );
    }

    #[test]
    fn quiet_alerter_works_through_the_trait_object() {
        let alerter: Box<dyn Alerter> = Box::new(QuietAlerter::new(Vec::new()));
        alerter.notify("a", "b").expect("notify");
    }

    #[test]
    fn a_failing_log_is_reported_not_swallowed() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk full"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let alerter = QuietAlerter::new(Broken);
        let err = alerter.notify("a", "b").expect_err("must fail");
        assert!(
            matches!(&err, AlertError::Notify(m) if m.contains("disk full")),
            "{err}"
        );
        let err = alerter
            .play(
                &Sound::Chime,
                Playback::Once {
                    max: Duration::from_secs(1),
                },
            )
            .expect_err("must fail");
        assert!(
            matches!(&err, AlertError::Notify(m) if m.contains("disk full")),
            "{err}"
        );
    }

    #[test]
    fn absent_selects_system() {
        assert_eq!(kind_for(None), AlerterKind::System);
    }

    #[test]
    fn zero_selects_system() {
        assert_eq!(kind_for(Some(OsStr::new("0"))), AlerterKind::System);
    }

    #[test]
    fn empty_selects_quiet() {
        assert_eq!(kind_for(Some(OsStr::new(""))), AlerterKind::Quiet);
    }

    #[test]
    fn other_values_select_quiet() {
        for v in ["1", "true", "yes", "false", "00", " 0", "0 "] {
            assert_eq!(kind_for(Some(OsStr::new(v))), AlerterKind::Quiet, "{v:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_selects_quiet() {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(
            kind_for(Some(OsStr::from_bytes(b"\xff\xfe"))),
            AlerterKind::Quiet
        );
    }

    #[test]
    fn alert_error_messages_name_the_cause() {
        assert_eq!(
            AlertError::Notify("no bus".into()).to_string(),
            "cannot show notification: no bus"
        );
        let sound = crate::sound::SoundError::Open {
            path: PathBuf::from("x.wav"),
            source: std::io::Error::other("nope"),
        };
        assert!(AlertError::from(sound).to_string().contains("x.wav"));
    }

    #[test]
    fn osascript_args_pass_title_and_body_as_separate_argv_items() {
        let evil = "x\" & (do shell script \"id\") & \"";
        let args = osascript_args("cli-timer", evil);
        assert_eq!(
            args,
            [
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                "cli-timer",
                evil,
            ]
        );
    }
}
