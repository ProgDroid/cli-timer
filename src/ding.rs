//! The `ding` command: a short sound and a notification, now.

use crate::alert::{self, Alerter, Outcome};
use crate::cli::DingArgs;
use crate::output::{self, Event};
use crate::sound::{self, DING_FILE_MAX, Playback, Sound};
use std::io::Write;
use std::time::Duration;

/// How long the built-in chime may play.
pub const CHIME_MAX: Duration = Duration::from_secs(2);

/// What the notification says when there is no message.
const DEFAULT_BODY: &str = "Done";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ding {
    pub message: Option<String>,
    pub sound: Sound,
    pub max: Duration,
}

/// Everything `run` needs from the command line, once it has been checked.
fn prepare(args: DingArgs) -> Result<Ding, String> {
    let (sound, max) = args.sound.map_or((Sound::Chime, CHIME_MAX), |path| {
        (Sound::File(path), DING_FILE_MAX)
    });
    sound::check(&sound).map_err(|e| e.to_string())?;

    Ok(Ding {
        message: output::non_empty(args.message),
        sound,
        max,
    })
}

/// Shows the notification, then plays the sound once. A failure of one does not stop the other.
pub fn send(d: &Ding, alerter: &dyn Alerter) -> Outcome {
    let body = d.message.as_deref().unwrap_or(DEFAULT_BODY);
    alert::deliver(alerter, body, &d.sound, Playback::Once { max: d.max })
}

/// Runs `ding` and returns the process exit code: `0`, `1` for an error before anything was
/// sent, or `3` when it was sent but the notification or the sound failed.
pub fn run(args: DingArgs, alerter: &dyn Alerter, out: &mut impl Write) -> u8 {
    let json = args.json;

    let ding = match prepare(args) {
        Ok(d) => d,
        Err(e) => return output::fail(out, &e, json),
    };

    let outcome = send(&ding, alerter);

    let dinged = Event::Dinged {
        message: ding.message,
    };
    let written = output::write_event(out, &dinged, json).and_then(|()| out.flush());

    output::finish(out, outcome, written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::{AlertError, QuietAlerter};
    use crate::sound::SoundError;
    use std::cell::RefCell;
    use std::io;
    use std::path::PathBuf;

    fn args(message: Option<&str>) -> DingArgs {
        DingArgs {
            message: message.map(str::to_owned),
            sound: None,
            json: true,
        }
    }

    /// A minimal 16-bit mono PCM WAV of 100 samples, in a fresh temp file.
    fn write_wav(name: &str) -> PathBuf {
        let data_len: u32 = 200;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_len).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes()); // PCM
        bytes.extend(1_u16.to_le_bytes()); // mono
        bytes.extend(8_000_u32.to_le_bytes());
        bytes.extend(16_000_u32.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_len.to_le_bytes());
        bytes.extend(vec![0_u8; 200]);
        let path =
            std::env::temp_dir().join(format!("cli-timer-{}-ding-{name}", std::process::id()));
        std::fs::write(&path, bytes).expect("write the wav");
        path
    }

    fn log_of(alerter: QuietAlerter<Vec<u8>>) -> String {
        String::from_utf8(alerter.into_inner()).expect("utf-8")
    }

    fn run_with(args: DingArgs, alerter: &dyn Alerter) -> (u8, String) {
        let mut out = Vec::new();
        let code = run(args, alerter, &mut out);
        (code, String::from_utf8(out).expect("utf-8"))
    }

    fn json_lines(text: &str) -> Vec<serde_json::Value> {
        text.lines()
            .map(|l| serde_json::from_str(l).expect("each line is JSON"))
            .collect()
    }

    #[test]
    fn without_a_sound_file_it_is_the_chime_played_once_for_at_most_two_seconds() {
        let ding = prepare(args(Some("x"))).expect("prepares");
        assert_eq!(ding.sound, Sound::Chime);
        assert_eq!(ding.max, Duration::from_secs(2));
        assert_eq!(CHIME_MAX, Duration::from_secs(2));
    }

    #[test]
    fn with_a_sound_file_it_is_that_file_cut_off_at_the_file_maximum() {
        let path = write_wav("prepare.wav");
        let mut a = args(None);
        a.sound = Some(path.clone());
        let ding = prepare(a).expect("prepares");
        assert_eq!(ding.sound, Sound::File(path));
        assert_eq!(ding.max, DING_FILE_MAX);
        assert_eq!(DING_FILE_MAX, Duration::from_secs(5));
    }

    #[test]
    fn a_missing_sound_file_is_an_error_naming_it() {
        let mut a = args(None);
        a.sound = Some(PathBuf::from("nope.wav"));
        let err = prepare(a).expect_err("must fail");
        assert!(err.contains("nope.wav"), "{err}");
    }

    #[test]
    fn an_empty_message_is_no_message_and_others_are_kept() {
        assert_eq!(prepare(args(Some(""))).expect("ok").message, None);
        assert_eq!(prepare(args(None)).expect("ok").message, None);
        assert_eq!(
            prepare(args(Some("Tests passed"))).expect("ok").message,
            Some("Tests passed".to_owned())
        );
    }

    fn ding(message: Option<&str>) -> Ding {
        Ding {
            message: message.map(str::to_owned),
            sound: Sound::Chime,
            max: CHIME_MAX,
        }
    }

    #[test]
    fn send_notifies_with_the_message_then_plays_once() {
        let alerter = QuietAlerter::new(Vec::new());
        let outcome = send(&ding(Some("Tests passed")), &alerter);
        assert!(matches!(outcome, Outcome::Ok), "{outcome:?}");
        assert_eq!(
            log_of(alerter),
            "quiet notify: cli-timer: Tests passed\nquiet play: Chime Once { max: 2s }\n"
        );
    }

    #[test]
    fn send_without_a_message_says_done() {
        let alerter = QuietAlerter::new(Vec::new());
        let _outcome = send(&ding(None), &alerter);
        assert!(log_of(alerter).starts_with("quiet notify: cli-timer: Done\n"));
    }

    #[test]
    fn send_plays_a_file_with_its_own_maximum() {
        let alerter = QuietAlerter::new(Vec::new());
        let d = Ding {
            message: None,
            sound: Sound::File(PathBuf::from("a.wav")),
            max: DING_FILE_MAX,
        };
        let _outcome = send(&d, &alerter);
        assert!(log_of(alerter).ends_with("quiet play: File(\"a.wav\") Once { max: 5s }\n"));
    }

    /// An alerter that fails on request.
    struct Failing {
        notify: bool,
        play: bool,
        calls: RefCell<u32>,
    }

    impl Alerter for Failing {
        fn notify(&self, _title: &str, _body: &str) -> Result<(), AlertError> {
            *self.calls.borrow_mut() += 1;
            if self.notify {
                Err(AlertError::Notify("no bus".into()))
            } else {
                Ok(())
            }
        }

        fn play(&self, _sound: &Sound, _playback: Playback) -> Result<(), AlertError> {
            *self.calls.borrow_mut() += 1;
            if self.play {
                Err(AlertError::Sound(SoundError::Open {
                    path: PathBuf::from("x.wav"),
                    source: io::Error::other("gone"),
                }))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn a_failing_notification_still_plays_and_is_partial() {
        let alerter = Failing {
            notify: true,
            play: false,
            calls: RefCell::new(0),
        };
        let Outcome::Partial(errors) = send(&ding(None), &alerter) else {
            panic!("expected Partial");
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(*alerter.calls.borrow(), 2);
        let (code, out) = run_with(args(None), &alerter);
        assert_eq!(code, 3);
        assert_eq!(
            json_lines(&out).len(),
            1,
            "errors never go to stdout: {out}"
        );
    }

    #[test]
    fn a_failing_sound_still_notifies_and_exits_three_with_the_event_printed() {
        let alerter = Failing {
            notify: false,
            play: true,
            calls: RefCell::new(0),
        };
        let (code, out) = run_with(args(Some("x")), &alerter);
        assert_eq!(code, 3);
        assert_eq!(*alerter.calls.borrow(), 2);
        let lines = json_lines(&out);
        assert_eq!(lines.len(), 1, "{out}");
        assert_eq!(lines[0]["event"], "dinged");
    }

    #[test]
    fn run_prints_one_dinged_line_and_alerts() {
        let alerter = QuietAlerter::new(Vec::new());
        let (code, out) = run_with(args(Some("Tests passed")), &alerter);
        assert_eq!(code, 0);
        assert_eq!(out, "{\"event\":\"dinged\",\"message\":\"Tests passed\"}\n");
        assert_eq!(
            log_of(alerter),
            "quiet notify: cli-timer: Tests passed\nquiet play: Chime Once { max: 2s }\n"
        );
    }

    #[test]
    fn run_in_human_mode_prints_a_text_line() {
        let mut a = args(Some("Tests passed"));
        a.json = false;
        let (code, out) = run_with(a, &QuietAlerter::new(Vec::new()));
        assert_eq!(code, 0);
        assert_eq!(out, "Ding: \"Tests passed\"\n");

        let mut bare = args(None);
        bare.json = false;
        let (_code, out) = run_with(bare, &QuietAlerter::new(Vec::new()));
        assert_eq!(out, "Ding\n");
    }

    #[test]
    fn an_empty_message_counts_as_no_message() {
        let alerter = QuietAlerter::new(Vec::new());
        let (code, out) = run_with(args(Some("")), &alerter);
        assert_eq!(code, 0);
        assert_eq!(out, "{\"event\":\"dinged\",\"message\":null}\n");
        assert!(log_of(alerter).starts_with("quiet notify: cli-timer: Done\n"));

        let mut human = args(Some(""));
        human.json = false;
        let (_code, out) = run_with(human, &QuietAlerter::new(Vec::new()));
        assert_eq!(out, "Ding\n");
    }

    #[test]
    fn a_nasty_message_stays_one_valid_json_line() {
        let message = "he said \"hi\"\n\\ \u{fc}n\u{ef}";
        let (_code, out) = run_with(args(Some(message)), &QuietAlerter::new(Vec::new()));
        assert_eq!(out.matches('\n').count(), 1, "{out:?}");
        assert_eq!(json_lines(&out)[0]["message"], message);
    }

    #[test]
    fn a_bad_sound_file_is_exit_one_with_nothing_alerted() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args(Some("x"));
        a.sound = Some(PathBuf::from("nope.wav"));
        let (code, out) = run_with(a, &alerter);
        assert_eq!(code, 1);
        let lines = json_lines(&out);
        assert_eq!(lines.len(), 1, "{out}");
        assert!(
            lines[0]["error"]
                .as_str()
                .expect("text")
                .contains("nope.wav")
        );
        assert!(lines[0].get("event").is_none());
        assert_eq!(log_of(alerter), "");
    }

    #[test]
    fn a_bad_sound_file_in_human_mode_keeps_stdout_empty() {
        let mut a = args(None);
        a.json = false;
        a.sound = Some(PathBuf::from("nope.wav"));
        let (code, out) = run_with(a, &QuietAlerter::new(Vec::new()));
        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out:?}");
    }
}
