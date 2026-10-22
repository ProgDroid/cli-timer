//! The `remind` command: wait without a screen, then notify and ring.

use crate::alert::{self, Alerter, Outcome};
use crate::cli::{EXIT_ERROR, RemindArgs};
use crate::duration;
use crate::output::{self, Event, report};
use crate::sound::{self, Playback, Sound};
use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::io::Write;
use std::time::Duration;

/// The longest single sleep while waiting, so a clock change or a sleeping machine is noticed
/// within a second.
const SLICE: Duration = Duration::from_secs(1);

/// What `notify` says when there is no label.
const DEFAULT_BODY: &str = "Timer finished";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reminder {
    pub label: Option<String>,
    pub sound: Sound,
    pub ring: Duration,
    pub fires_at: DateTime<Local>,
}

/// Returns once `now()` is at or past `fires_at`, sleeping at most one second at a time and
/// re-reading the clock after each sleep.
pub fn wait_until(
    fires_at: DateTime<Local>,
    now: impl Fn() -> DateTime<Local>,
    mut sleep: impl FnMut(Duration),
) {
    loop {
        let current = now();
        if current >= fires_at {
            return;
        }
        let remaining = fires_at
            .signed_duration_since(current)
            .to_std()
            .unwrap_or_default();
        sleep(remaining.min(SLICE));
    }
}

/// Shows the notification, then rings. A failure of one does not stop the other.
pub fn fire(r: &Reminder, alerter: &dyn Alerter) -> Outcome {
    let body = r.label.as_deref().unwrap_or(DEFAULT_BODY);
    let playback = Playback::Loop {
        for_: sound::ring_for(r.ring),
    };
    alert::deliver(alerter, body, &r.sound, playback)
}

/// Everything `run` needs from the command line, once it has been checked.
fn prepare(args: RemindArgs) -> Result<Reminder, String> {
    let wait = duration::parse(&args.duration).map_err(|e| e.to_string())?;
    let ring = duration::parse(&args.ring).map_err(|e| format!("--ring: {e}"))?;
    let fires_at = ChronoDuration::from_std(wait)
        .ok()
        .and_then(|d| Local::now().checked_add_signed(d))
        .ok_or_else(|| format!("duration \"{}\" is too long", args.duration))?;
    let sound = args.sound.map_or(Sound::Alarm, Sound::File);
    sound::check(&sound).map_err(|e| e.to_string())?;

    Ok(Reminder {
        label: output::non_empty(args.label),
        sound,
        ring,
        fires_at,
    })
}

/// Runs `remind` and returns the process exit code: `0`, `1` for an error before anything fired,
/// or `3` when it fired but the notification or the sound failed.
pub fn run(args: RemindArgs, alerter: &dyn Alerter, out: &mut impl Write) -> u8 {
    let json = args.json;

    let reminder = match prepare(args) {
        Ok(r) => r,
        Err(e) => return output::fail(out, &e, json),
    };

    let set = Event::Set {
        label: reminder.label.clone(),
        fires_at: reminder.fires_at,
    };
    if let Err(e) = output::write_event(out, &set, json).and_then(|()| out.flush()) {
        report(out, &format!("cannot write to stdout: {e}"), false);
        return EXIT_ERROR;
    }

    wait_until(reminder.fires_at, Local::now, std::thread::sleep);

    let outcome = fire(&reminder, alerter);

    let fired = Event::Fired {
        label: reminder.label,
    };
    let written = output::write_event(out, &fired, json).and_then(|()| out.flush());

    output::finish(out, outcome, written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::{AlertError, QuietAlerter};
    use crate::sound::{MAX_RING, SoundError};
    use std::cell::{Cell, RefCell};
    use std::io;
    use std::path::PathBuf;

    fn base() -> DateTime<Local> {
        Local::now()
    }

    fn delta(d: Duration) -> ChronoDuration {
        ChronoDuration::from_std(d).expect("fits")
    }

    /// A clock that only moves when `sleep` is called, by exactly the time slept, plus a record of
    /// every sleep. `extra_on_first` is added to the clock on the first sleep (a machine asleep).
    fn drive(start_before: Duration, extra_on_first: Duration) -> Vec<Duration> {
        let deadline = base();
        let clock = Cell::new(deadline - delta(start_before));
        let sleeps = RefCell::new(Vec::new());
        wait_until(
            deadline,
            || clock.get(),
            |d| {
                let mut log = sleeps.borrow_mut();
                assert!(log.len() < 20, "wait_until never finishes");
                let extra = if log.is_empty() {
                    extra_on_first
                } else {
                    Duration::ZERO
                };
                log.push(d);
                clock.set(clock.get() + delta(d) + delta(extra));
            },
        );
        sleeps.into_inner()
    }

    #[test]
    fn three_seconds_short_takes_three_one_second_sleeps() {
        let sleeps = drive(Duration::from_secs(3), Duration::ZERO);
        assert_eq!(sleeps, vec![Duration::from_secs(1); 3]);
    }

    #[test]
    fn a_partial_last_slice_is_shorter() {
        let sleeps = drive(Duration::from_millis(2500), Duration::ZERO);
        assert_eq!(
            sleeps,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_millis(500)
            ]
        );
    }

    #[test]
    fn a_short_wait_sleeps_only_what_is_left() {
        let sleeps = drive(Duration::from_millis(300), Duration::ZERO);
        assert_eq!(sleeps, vec![Duration::from_millis(300)]);
    }

    #[test]
    fn no_slice_is_longer_than_one_second_however_far_away() {
        let sleeps = drive(Duration::from_secs(7), Duration::ZERO);
        assert_eq!(sleeps.len(), 7);
        assert!(sleeps.iter().all(|d| *d <= Duration::from_secs(1)));
    }

    #[test]
    fn a_deadline_already_passed_never_sleeps() {
        let deadline = base();
        wait_until(
            deadline,
            || deadline + ChronoDuration::seconds(5),
            |_| panic!("must not sleep"),
        );
    }

    #[test]
    fn a_deadline_exactly_now_never_sleeps() {
        let deadline = base();
        wait_until(deadline, || deadline, |_| panic!("must not sleep"));
    }

    #[test]
    fn a_machine_that_slept_past_the_deadline_returns_after_one_sleep() {
        let sleeps = drive(Duration::from_hours(1), Duration::from_hours(2));
        assert_eq!(sleeps.len(), 1);
        assert!(sleeps[0] <= Duration::from_secs(1));
    }

    fn reminder(label: Option<&str>) -> Reminder {
        Reminder {
            label: label.map(str::to_owned),
            sound: Sound::Chime,
            ring: Duration::from_secs(2),
            fires_at: base(),
        }
    }

    fn log_of(alerter: QuietAlerter<Vec<u8>>) -> String {
        String::from_utf8(alerter.into_inner()).expect("utf-8")
    }

    #[test]
    fn fire_notifies_then_plays() {
        let alerter = QuietAlerter::new(Vec::new());
        let outcome = fire(&reminder(Some("check the oven")), &alerter);
        assert!(matches!(outcome, Outcome::Ok), "{outcome:?}");
        assert_eq!(
            log_of(alerter),
            "quiet notify: cli-timer: check the oven\nquiet play: Chime Loop { for_: 2s }\n"
        );
    }

    #[test]
    fn fire_without_a_label_says_timer_finished() {
        let alerter = QuietAlerter::new(Vec::new());
        let _outcome = fire(&reminder(None), &alerter);
        assert!(log_of(alerter).starts_with("quiet notify: cli-timer: Timer finished\n"));
    }

    #[test]
    fn fire_caps_the_ring_at_the_maximum() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut r = reminder(None);
        r.ring = Duration::from_hours(5);
        let _outcome = fire(&r, &alerter);
        assert!(log_of(alerter).ends_with(&format!(
            "quiet play: Chime Loop {{ for_: {MAX_RING:?} }}\n"
        )));
    }

    /// An alerter that records what it was asked and fails on request.
    struct Scripted {
        fail_notify: bool,
        fail_play: bool,
        calls: RefCell<Vec<String>>,
    }

    impl Scripted {
        fn new(fail_notify: bool, fail_play: bool) -> Self {
            Self {
                fail_notify,
                fail_play,
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl Alerter for Scripted {
        fn notify(&self, title: &str, body: &str) -> Result<(), AlertError> {
            self.calls
                .borrow_mut()
                .push(format!("notify {title}|{body}"));
            if self.fail_notify {
                Err(AlertError::Notify("no bus".into()))
            } else {
                Ok(())
            }
        }

        fn play(&self, sound: &Sound, playback: Playback) -> Result<(), AlertError> {
            self.calls
                .borrow_mut()
                .push(format!("play {sound:?} {playback:?}"));
            if self.fail_play {
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
    fn a_failing_play_still_notifies_and_is_partial() {
        let alerter = Scripted::new(false, true);
        let Outcome::Partial(errors) = fire(&reminder(None), &alerter) else {
            panic!("expected Partial");
        };
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], AlertError::Sound(_)));
        let calls = alerter.calls.borrow();
        assert_eq!(calls[0], "notify cli-timer|Timer finished");
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn a_failing_notify_still_plays_and_is_partial() {
        let alerter = Scripted::new(true, false);
        let Outcome::Partial(errors) = fire(&reminder(Some("x")), &alerter) else {
            panic!("expected Partial");
        };
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], AlertError::Notify(_)));
        assert_eq!(alerter.calls.borrow().len(), 2);
    }

    #[test]
    fn both_failing_gives_two_errors_in_order() {
        let alerter = Scripted::new(true, true);
        let Outcome::Partial(errors) = fire(&reminder(None), &alerter) else {
            panic!("expected Partial");
        };
        assert_eq!(errors.len(), 2);
        assert!(matches!(errors[0], AlertError::Notify(_)));
        assert!(matches!(errors[1], AlertError::Sound(_)));
    }

    fn args(duration: &str) -> RemindArgs {
        RemindArgs {
            duration: duration.to_owned(),
            label: None,
            sound: None,
            ring: "30s".to_owned(),
            json: true,
        }
    }

    fn run_with(args: RemindArgs, alerter: &dyn Alerter) -> (u8, String) {
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
    fn run_prints_set_then_fired_and_rings_in_between() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args("0s");
        a.label = Some("oven".into());
        a.ring = "2s".into();
        let (code, out) = run_with(a, &alerter);
        assert_eq!(code, 0);
        let lines = json_lines(&out);
        assert_eq!(lines.len(), 2, "{out}");
        assert_eq!(lines[0]["event"], "set");
        assert_eq!(lines[0]["label"], "oven");
        assert!(lines[0]["fires_at"].is_string());
        assert_eq!(lines[1]["event"], "fired");
        assert_eq!(lines[1]["label"], "oven");
        assert_eq!(
            log_of(alerter),
            "quiet notify: cli-timer: oven\nquiet play: Alarm Loop { for_: 2s }\n"
        );
    }

    #[test]
    fn a_missing_sound_file_given_to_run_is_an_error() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args("0s");
        a.sound = Some(PathBuf::from("does-not-matter"));
        // The file is checked before anything is printed, so a missing one is an error.
        let (code, out) = run_with(a, &alerter);
        assert_eq!(code, 1);
        assert_eq!(json_lines(&out).len(), 1);
        assert_eq!(log_of(alerter), "");
    }

    #[test]
    fn a_bad_sound_file_fails_before_set_and_before_waiting() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args("2h");
        a.sound = Some(PathBuf::from("nope.wav"));
        let started = std::time::Instant::now();
        let (code, out) = run_with(a, &alerter);
        // Far below the 2 h wait, and far above any scheduling hiccup on a loaded runner.
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(code, 1);
        let lines = json_lines(&out);
        assert_eq!(lines.len(), 1, "{out}");
        assert!(
            lines[0]["error"]
                .as_str()
                .expect("error text")
                .contains("nope.wav")
        );
        assert!(lines[0].get("event").is_none());
    }

    #[test]
    fn a_bad_duration_is_exit_one_with_the_accepted_forms() {
        for bad in ["1.5h", "30m1h", "", "abc", "-5m", "99999999999h"] {
            let alerter = QuietAlerter::new(Vec::new());
            let (code, out) = run_with(args(bad), &alerter);
            assert_eq!(code, 1, "{bad:?}");
            let lines = json_lines(&out);
            assert_eq!(lines.len(), 1, "{bad:?}: {out}");
            assert!(lines[0]["error"].is_string(), "{bad:?}");
            assert_eq!(log_of(alerter), "");
        }
        let (_code, out) = run_with(args("1.5h"), &QuietAlerter::new(Vec::new()));
        assert!(out.contains("hh:mm:ss"), "{out}");
    }

    #[test]
    fn a_bad_ring_is_exit_one_and_names_the_option() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args("0s");
        a.ring = "forever".into();
        let (code, out) = run_with(a, &alerter);
        assert_eq!(code, 1);
        let lines = json_lines(&out);
        assert_eq!(lines.len(), 1);
        assert!(lines[0]["error"].as_str().expect("text").contains("--ring"));
    }

    #[test]
    fn human_mode_prints_text_lines_and_keeps_errors_off_stdout() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args("0s");
        a.json = false;
        a.label = Some("oven".into());
        let (code, out) = run_with(a, &alerter);
        assert_eq!(code, 0);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "{out}");
        assert!(lines[0].starts_with("Reminder set: \"oven\" at "), "{out}");
        assert_eq!(lines[1], "Reminder fired: \"oven\"");

        let mut bad = args("1.5h");
        bad.json = false;
        let (code, out) = run_with(bad, &QuietAlerter::new(Vec::new()));
        assert_eq!(code, 1);
        assert!(out.is_empty(), "human errors go to stderr: {out:?}");
    }

    #[test]
    fn an_empty_label_counts_as_no_label() {
        let alerter = QuietAlerter::new(Vec::new());
        let mut a = args("0s");
        a.label = Some(String::new());
        let (code, out) = run_with(a, &alerter);
        assert_eq!(code, 0);
        let lines = json_lines(&out);
        assert!(lines[0]["label"].is_null(), "{out}");
        assert!(lines[1]["label"].is_null(), "{out}");
        assert!(log_of(alerter).starts_with("quiet notify: cli-timer: Timer finished\n"));

        let mut human = args("0s");
        human.json = false;
        human.label = Some(String::new());
        let (_code, out) = run_with(human, &QuietAlerter::new(Vec::new()));
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("Reminder set at "), "{out}");
        assert_eq!(lines[1], "Reminder fired");
    }

    #[test]
    fn run_returns_three_when_the_ring_fails_but_still_prints_fired() {
        let alerter = Scripted::new(false, true);
        let (code, out) = run_with(args("0s"), &alerter);
        assert_eq!(code, 3);
        let lines = json_lines(&out);
        assert_eq!(lines.len(), 2, "errors never go to stdout: {out}");
        assert_eq!(lines[1]["event"], "fired");
    }

    #[test]
    fn run_returns_three_when_the_notification_fails() {
        let alerter = Scripted::new(true, false);
        let (code, _out) = run_with(args("0s"), &alerter);
        assert_eq!(code, 3);
        assert_eq!(alerter.calls.borrow().len(), 2);
    }
}
