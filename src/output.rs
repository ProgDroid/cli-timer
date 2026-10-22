//! What the commands print: one event per line, as text or as JSON for agents.

use crate::alert::Outcome;
use crate::cli::{EXIT_ERROR, EXIT_OK, EXIT_PARTIAL};
use chrono::{DateTime, Local, SecondsFormat};
use serde::{Serialize, Serializer};
use std::io::{self, Write};

/// Something that happened, printed as it happens.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    /// A reminder is set and will fire at `fires_at`.
    Set {
        label: Option<String>,
        #[serde(serialize_with = "whole_seconds")]
        fires_at: DateTime<Local>,
    },
    /// A reminder's time is up.
    Fired { label: Option<String> },
    /// A `ding` was sent.
    Dinged { message: Option<String> },
}

/// Serialises a time as RFC 3339 to the second, with a numeric offset: `2026-10-22T14:32:00+01:00`.
fn whole_seconds<S: Serializer>(at: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&at.to_rfc3339_opts(SecondsFormat::Secs, false))
}

/// Text from the command line, where an empty string counts as nothing: a label or message of
/// `""` is the same as none.
#[must_use]
pub fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|t| !t.is_empty())
}

/// `: "text"` when there is text, nothing otherwise.
fn quoted(text: Option<&str>) -> String {
    text.map_or_else(String::new, |t| format!(": \"{t}\""))
}

/// Writes `event` as one line: a JSON object when `json`, otherwise text.
///
/// # Errors
/// Returns the error of the underlying write.
pub fn write_event(out: &mut impl Write, event: &Event, json: bool) -> io::Result<()> {
    if json {
        serde_json::to_writer(&mut *out, event)?;
        return writeln!(out);
    }
    match event {
        Event::Set { label, fires_at } => {
            let at = fires_at.format("%H:%M:%S");
            match label {
                Some(l) => writeln!(out, "Reminder set: \"{l}\" at {at}"),
                None => writeln!(out, "Reminder set at {at}"),
            }
        }
        Event::Fired { label } => writeln!(out, "Reminder fired{}", quoted(label.as_deref())),
        Event::Dinged { message } => writeln!(out, "Ding{}", quoted(message.as_deref())),
    }
}

/// Writes an error as one line: `{"error":"..."}` when `json`, otherwise `error: ...`.
///
/// # Errors
/// Returns the error of the underlying write.
pub fn write_error(out: &mut impl Write, err: &str, json: bool) -> io::Result<()> {
    if json {
        serde_json::to_writer(&mut *out, &serde_json::json!({ "error": err }))?;
        writeln!(out)
    } else {
        writeln!(out, "error: {err}")
    }
}

/// Writes an error where the mode says: stdout (`out`) under `--json`, stderr otherwise.
pub fn report(out: &mut impl Write, err: &str, json: bool) {
    let written = if json {
        write_error(out, err, true)
    } else {
        write_error(&mut io::stderr(), err, false)
    };
    if let Err(e) = written {
        eprintln!("error: {err} (and the report failed: {e})");
    }
}

/// The exit code for a command that got as far as alerting: `0`, or `3` on a partial failure.
///
/// Partial means the alert partly failed or the event could not be written. The errors go to
/// stderr, one line each, so a JSON reader of stdout sees only events.
pub fn finish(out: &mut impl Write, outcome: Outcome, written: io::Result<()>) -> u8 {
    let mut code = EXIT_OK;
    if let Outcome::Partial(errors) = outcome {
        code = EXIT_PARTIAL;
        for e in errors {
            report(out, &e.to_string(), false);
        }
    }
    if let Err(e) = written {
        code = EXIT_PARTIAL;
        report(out, &format!("cannot write to stdout: {e}"), false);
    }
    code
}

/// Reports `err` and gives the exit code for an error before anything fired.
pub fn fail(out: &mut impl Write, err: &str, json: bool) -> u8 {
    report(out, err, json);
    EXIT_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};
    use serde_json::Value;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 10, 22, h, m, s)
            .single()
            .expect("unambiguous local time")
    }

    fn event_text(event: &Event, json: bool) -> String {
        let mut out = Vec::new();
        write_event(&mut out, event, json).expect("write");
        String::from_utf8(out).expect("utf-8")
    }

    fn set(label: Option<&str>) -> Event {
        Event::Set {
            label: label.map(str::to_owned),
            fires_at: at(14, 32, 5),
        }
    }

    #[test]
    fn nasty_label_stays_one_valid_json_line() {
        let label = "he said \"hi\"\n\\ \u{fc}n\u{ef}";
        let text = event_text(&set(Some(label)), true);
        assert!(text.ends_with('\n'));
        assert_eq!(text.matches('\n').count(), 1, "one line only: {text:?}");
        let value: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["event"], "set");
        assert_eq!(value["label"], label);
    }

    #[test]
    fn set_json_carries_fires_at_as_rfc3339() {
        let text = event_text(&set(Some("x")), true);
        let value: Value = serde_json::from_str(&text).expect("valid JSON");
        let fires_at = value["fires_at"].as_str().expect("a string");
        let parsed = DateTime::parse_from_rfc3339(fires_at).expect("rfc3339");
        assert_eq!(parsed, at(14, 32, 5));
    }

    #[test]
    fn set_json_fires_at_is_whole_seconds_with_an_offset() {
        let with_fraction = at(14, 32, 0)
            .with_nanosecond(987_654_321)
            .expect("valid nanoseconds");
        let text = event_text(
            &Event::Set {
                label: None,
                fires_at: with_fraction,
            },
            true,
        );
        let value: Value = serde_json::from_str(&text).expect("valid JSON");
        let fires_at = value["fires_at"].as_str().expect("a string");
        // `2026-10-22T14:32:00+01:00`: 25 characters, no fractional part, a numeric offset.
        assert_eq!(fires_at.len(), 25, "{fires_at}");
        assert!(!fires_at.contains('.'), "{fires_at}");
        assert!(matches!(fires_at.as_bytes()[19], b'+' | b'-'), "{fires_at}");
        let parsed = DateTime::parse_from_rfc3339(fires_at).expect("rfc3339");
        assert_eq!(parsed, at(14, 32, 0));
    }

    #[test]
    fn no_label_is_json_null() {
        let value: Value =
            serde_json::from_str(&event_text(&Event::Fired { label: None }, true)).expect("json");
        assert_eq!(value["event"], "fired");
        assert!(value["label"].is_null());
    }

    #[test]
    fn fired_json_has_exactly_event_and_label() {
        let text = event_text(
            &Event::Fired {
                label: Some("a".into()),
            },
            true,
        );
        assert_eq!(text, "{\"event\":\"fired\",\"label\":\"a\"}\n");
    }

    #[test]
    fn dinged_json_is_exact() {
        let text = event_text(
            &Event::Dinged {
                message: Some("done".into()),
            },
            true,
        );
        assert_eq!(text, "{\"event\":\"dinged\",\"message\":\"done\"}\n");
        assert_eq!(
            event_text(&Event::Dinged { message: None }, true),
            "{\"event\":\"dinged\",\"message\":null}\n"
        );
    }

    #[test]
    fn human_set_line() {
        assert_eq!(
            event_text(&set(Some("check the oven")), false),
            "Reminder set: \"check the oven\" at 14:32:05\n"
        );
        assert_eq!(event_text(&set(None), false), "Reminder set at 14:32:05\n");
    }

    #[test]
    fn human_fired_line() {
        let fired = |label: Option<&str>| Event::Fired {
            label: label.map(str::to_owned),
        };
        assert_eq!(
            event_text(&fired(Some("check the oven")), false),
            "Reminder fired: \"check the oven\"\n"
        );
        assert_eq!(event_text(&fired(None), false), "Reminder fired\n");
    }

    #[test]
    fn human_ding_line() {
        let ding = |message: Option<&str>| Event::Dinged {
            message: message.map(str::to_owned),
        };
        assert_eq!(
            event_text(&ding(Some("Tests passed")), false),
            "Ding: \"Tests passed\"\n"
        );
        assert_eq!(event_text(&ding(None), false), "Ding\n");
    }

    #[test]
    fn json_error_is_one_escaped_line() {
        let mut out = Vec::new();
        write_error(&mut out, "bad \"x\"\nnext", true).expect("write");
        let text = String::from_utf8(out).expect("utf-8");
        assert_eq!(text.matches('\n').count(), 1);
        let value: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["error"], "bad \"x\"\nnext");
        assert_eq!(value.as_object().expect("object").len(), 1);
    }

    #[test]
    fn non_empty_turns_an_empty_string_into_none() {
        assert_eq!(non_empty(None), None);
        assert_eq!(non_empty(Some(String::new())), None);
        assert_eq!(non_empty(Some("x".into())), Some("x".into()));
        assert_eq!(non_empty(Some(" ".into())), Some(" ".into()));
    }

    #[test]
    fn report_under_json_writes_an_error_object_to_out() {
        let mut out = Vec::new();
        report(&mut out, "bad", true);
        assert_eq!(
            String::from_utf8(out).expect("utf-8"),
            "{\"error\":\"bad\"}\n"
        );
    }

    #[test]
    fn report_in_text_mode_keeps_out_empty() {
        let mut out = Vec::new();
        report(&mut out, "bad", false);
        assert_eq!(out, b"");
    }

    #[test]
    fn fail_reports_and_gives_exit_one() {
        let mut out = Vec::new();
        assert_eq!(fail(&mut out, "bad", true), 1);
        assert_eq!(
            String::from_utf8(out).expect("utf-8"),
            "{\"error\":\"bad\"}\n"
        );
    }

    #[test]
    fn finish_is_zero_when_all_went_well() {
        let mut out = Vec::new();
        assert_eq!(finish(&mut out, Outcome::Ok, Ok(())), 0);
        assert_eq!(out, b"");
    }

    #[test]
    fn finish_is_three_when_the_alert_partly_failed() {
        let mut out = Vec::new();
        let partial = Outcome::Partial(vec![crate::alert::AlertError::Notify("no bus".into())]);
        assert_eq!(finish(&mut out, partial, Ok(())), 3);
        assert!(out.is_empty(), "errors never go to out: {out:?}");
    }

    #[test]
    fn finish_is_three_when_the_event_could_not_be_written() {
        let mut out = Vec::new();
        let broken = Err(io::Error::other("closed"));
        assert_eq!(finish(&mut out, Outcome::Ok, broken), 3);
    }

    #[test]
    fn human_error_is_a_plain_line() {
        let mut out = Vec::new();
        write_error(&mut out, "nope", false).expect("write");
        assert_eq!(String::from_utf8(out).expect("utf-8"), "error: nope\n");
    }
}
