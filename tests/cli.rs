//! End-to-end tests: the real binary, run quietly (no notification, no sound) with stdin closed.

use serde_json::Value;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn run(args: &[&str]) -> (Output, Duration) {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_cli-timer"))
        .args(args)
        .env("CLI_TIMER_QUIET", "1")
        .stdin(Stdio::null())
        .output()
        .expect("run the binary");
    (output, started.elapsed())
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

fn json_lines(output: &Output) -> Vec<Value> {
    text(&output.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("every stdout line is JSON"))
        .collect()
}

#[test]
fn remind_prints_set_then_fired_and_notifies() {
    let (output, elapsed) = run(&[
        "remind",
        "1s",
        "--ring",
        "1s",
        "--json",
        "-l",
        "check the oven",
    ]);

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let lines = json_lines(&output);
    assert_eq!(lines.len(), 2, "{}", text(&output.stdout));
    assert_eq!(lines[0]["event"], "set");
    assert_eq!(lines[0]["label"], "check the oven");
    assert!(lines[0]["fires_at"].is_string());
    assert_eq!(lines[1]["event"], "fired");
    assert_eq!(lines[1]["label"], "check the oven");

    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("quiet notify: cli-timer: check the oven"),
        "{stderr}"
    );
    assert!(
        stderr.contains("quiet play: Alarm Loop { for_: 1s }"),
        "{stderr}"
    );
    assert!(elapsed >= Duration::from_secs(1), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}

#[test]
fn remind_human_mode_prints_text() {
    let (output, _elapsed) = run(&["remind", "0s", "-l", "tea"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(
        lines[0].starts_with("Reminder set: \"tea\" at "),
        "{stdout}"
    );
    assert_eq!(lines[1], "Reminder fired: \"tea\"");
}

#[test]
fn remind_without_a_label_notifies_timer_finished() {
    let (output, _elapsed) = run(&["remind", "0s", "--json"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stderr).contains("quiet notify: cli-timer: Timer finished"));
    assert!(json_lines(&output)[0]["label"].is_null());
}

#[test]
fn remind_treats_an_empty_label_as_no_label() {
    let (output, _elapsed) = run(&["remind", "0s", "--json", "-l", ""]);

    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stderr).contains("quiet notify: cli-timer: Timer finished"));
    for line in json_lines(&output) {
        assert!(line["label"].is_null(), "{line}");
    }

    let (output, _elapsed) = run(&["remind", "0s", "-l", ""]);
    let stdout = text(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines[0].starts_with("Reminder set at "), "{stdout}");
    assert_eq!(lines[1], "Reminder fired");
}

#[test]
fn the_set_events_fires_at_is_whole_seconds() {
    let (output, _elapsed) = run(&["remind", "0s", "--json"]);

    let lines = json_lines(&output);
    let fires_at = lines[0]["fires_at"].as_str().expect("a string");
    assert_eq!(fires_at.len(), 25, "{fires_at}");
    assert!(!fires_at.contains('.'), "{fires_at}");
    assert!(matches!(fires_at.as_bytes()[19], b'+' | b'-'), "{fires_at}");
}

#[test]
fn remind_survives_a_nasty_label_as_json() {
    let label = "he said \"hi\"\n\\ \u{fc}n\u{ef}";
    let (output, _elapsed) = run(&["remind", "0s", "--json", "-l", label]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    assert_eq!(stdout.lines().count(), 2, "{stdout}");
    for line in json_lines(&output) {
        assert_eq!(line["label"], label);
    }
}

#[test]
fn a_bad_sound_file_fails_at_once_without_printing_set() {
    // A 30 s reminder with a 10 s bound: failing "at once" means failing before the wait, and the
    // bound leaves room for a loaded runner.
    let (output, elapsed) = run(&["remind", "30s", "-s", "nope.wav", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
    let lines = json_lines(&output);
    assert_eq!(lines.len(), 1, "{}", text(&output.stdout));
    assert!(
        lines[0]["error"]
            .as_str()
            .expect("text")
            .contains("nope.wav")
    );
    assert!(lines[0].get("event").is_none());
}

#[test]
fn a_bad_sound_file_in_human_mode_goes_to_stderr() {
    let (output, elapsed) = run(&["remind", "30s", "-s", "nope.wav"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
    assert_eq!(text(&output.stdout), "");
    assert!(text(&output.stderr).contains("nope.wav"));
}

#[test]
fn a_malformed_duration_is_exit_one_naming_the_forms() {
    let (output, _elapsed) = run(&["remind", "1.5h", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    let lines = json_lines(&output);
    assert_eq!(lines.len(), 1, "{}", text(&output.stdout));
    let error = lines[0]["error"].as_str().expect("text");
    assert!(error.contains("hh:mm:ss"), "{error}");
    assert!(error.contains("1h30m"), "{error}");
}

#[test]
fn a_huge_duration_is_exit_one_not_a_panic() {
    let (output, _elapsed) = run(&["remind", "99999999999h", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json_lines(&output).len(), 1);
    assert!(json_lines(&output)[0]["error"].is_string());
    assert!(!text(&output.stderr).contains("panicked"));
}

#[test]
fn a_bad_ring_is_exit_one() {
    let (output, _elapsed) = run(&["remind", "1s", "--ring", "forever", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json_lines(&output).len(), 1);
}

#[test]
fn a_huge_tui_time_is_a_clap_error_not_a_panic() {
    let (output, _elapsed) = run(&["-t", "99999999999h"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = text(&output.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(stderr.contains("too long"), "{stderr}");
}

#[test]
fn no_arguments_is_a_usage_error() {
    let (output, _elapsed) = run(&[]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("-t"), "{stderr}");
    assert!(stderr.contains("Usage"), "{stderr}");
}

#[test]
fn tui_flags_with_a_subcommand_are_a_usage_error() {
    let (output, _elapsed) = run(&["-t", "5m", "remind", "1m"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn help_describes_the_program_and_shows_both_usages() {
    let (output, _elapsed) = run(&["--help"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    assert!(
        stdout.contains("A countdown timer for the terminal"),
        "{stdout}"
    );
    assert!(stdout.contains("cli-timer -t <TIME>"), "{stdout}");
    assert!(stdout.contains("cli-timer <COMMAND>"), "{stdout}");
}

#[test]
fn version_still_works() {
    let (output, _elapsed) = run(&["--version"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).starts_with("cli-timer "));
}

/// A minimal 16-bit mono PCM WAV of 100 samples, in a fresh temp file.
fn write_wav(name: &str) -> std::path::PathBuf {
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
    let path = std::env::temp_dir().join(format!("cli-timer-{}-e2e-{name}", std::process::id()));
    std::fs::write(&path, bytes).expect("write the wav");
    path
}

#[test]
fn ding_prints_one_dinged_line_and_notifies_fast() {
    let (output, elapsed) = run(&["ding", "--json", "Tests passed"]);

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "{\"event\":\"dinged\",\"message\":\"Tests passed\"}\n"
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("quiet notify: cli-timer: Tests passed"),
        "{stderr}"
    );
    assert!(
        stderr.contains("quiet play: Chime Once { max: 2s }"),
        "{stderr}"
    );
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}

#[test]
fn ding_takes_a_message_that_starts_with_a_hyphen() {
    // Every form the skill or a person might write must reach the alert, not exit 2.
    let forms: [&[&str]; 3] = [
        &["ding", "--json", "--", "-5 tests failed"],
        &["ding", "--json", "-5 tests failed"],
        &["ding", "-5 tests failed", "--json"],
    ];
    for args in forms {
        let (output, _elapsed) = run(args);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {stderr}");
        assert_eq!(
            text(&output.stdout),
            "{\"event\":\"dinged\",\"message\":\"-5 tests failed\"}\n",
            "{args:?}"
        );
        assert!(
            stderr.contains("quiet notify: cli-timer: -5 tests failed"),
            "{stderr}"
        );
    }
}

#[test]
fn ding_flags_after_the_double_dash_are_not_flags() {
    // Documented limit: after `--` everything is positional, so `--json` there is a second
    // message word, which is a usage error. The skill puts `--json` before the `--`.
    let (output, _elapsed) = run(&["ding", "--", "-5 tests failed", "--json"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn ding_takes_a_short_hyphen_message() {
    let (output, _elapsed) = run(&["ding", "--json", "-x"]);

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "{\"event\":\"dinged\",\"message\":\"-x\"}\n"
    );
}

#[test]
fn remind_takes_a_label_that_starts_with_a_hyphen() {
    let forms: [&[&str]; 3] = [
        &["remind", "0s", "-l", "-oops", "--json"],
        &["remind", "0s", "--json", "-l", "-oops"],
        &["remind", "0s", "-l=-oops", "--json"],
    ];
    for args in forms {
        let (output, _elapsed) = run(args);
        let stderr = text(&output.stderr);

        assert_eq!(output.status.code(), Some(0), "{args:?}: {stderr}");
        let lines = json_lines(&output);
        assert_eq!(lines.len(), 2, "{args:?}");
        assert_eq!(lines[0]["label"], "-oops", "{args:?}");
        assert_eq!(lines[1]["label"], "-oops", "{args:?}");
        assert!(
            stderr.contains("quiet notify: cli-timer: -oops"),
            "{stderr}"
        );
    }
}

#[test]
fn help_names_the_sound_option_file_in_every_command() {
    for args in [&["--help"][..], &["remind", "--help"], &["ding", "--help"]] {
        let (output, _elapsed) = run(args);
        let stdout = text(&output.stdout);

        assert_eq!(output.status.code(), Some(0), "{stdout}");
        assert!(!stdout.contains("<SOUND>"), "{stdout}");
        assert!(args.len() == 1 || stdout.contains("-s <FILE>"), "{stdout}");
    }
}

#[test]
fn ding_without_a_message_says_done() {
    let (output, _elapsed) = run(&["ding"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(text(&output.stdout), "Ding\n");
    assert!(text(&output.stderr).contains("quiet notify: cli-timer: Done"));
}

#[test]
fn ding_treats_an_empty_message_as_no_message() {
    let (output, _elapsed) = run(&["ding", "--json", ""]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        text(&output.stdout),
        "{\"event\":\"dinged\",\"message\":null}\n"
    );
    assert!(text(&output.stderr).contains("quiet notify: cli-timer: Done"));
}

#[test]
fn ding_survives_a_nasty_message_as_json() {
    let message = "he said \"hi\"\n\\ \u{fc}n\u{ef}";
    let (output, _elapsed) = run(&["ding", "--json", message]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(text(&output.stdout).lines().count(), 1);
    assert_eq!(json_lines(&output)[0]["message"], message);
}

#[test]
fn ding_with_a_sound_file_plays_it_once_for_at_most_five_seconds() {
    let wav = write_wav("ding.wav");
    let (output, _elapsed) = run(&["ding", "-s", wav.to_str().expect("utf-8 path"), "x"]);

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("Once { max: 5s }"), "{stderr}");
    assert!(stderr.contains("File("), "{stderr}");
}

#[test]
fn ding_with_a_bad_sound_file_is_exit_one_and_alerts_nothing() {
    let (output, _elapsed) = run(&["ding", "-s", "nope.wav", "--json", "x"]);

    assert_eq!(output.status.code(), Some(1));
    let lines = json_lines(&output);
    assert_eq!(lines.len(), 1, "{}", text(&output.stdout));
    assert!(
        lines[0]["error"]
            .as_str()
            .expect("text")
            .contains("nope.wav")
    );
    assert!(!text(&output.stderr).contains("quiet notify"));
}

#[test]
fn ding_with_a_bad_sound_file_in_human_mode_goes_to_stderr() {
    let (output, _elapsed) = run(&["ding", "-s", "nope.wav"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "");
    assert!(text(&output.stderr).contains("nope.wav"));
}

#[test]
fn ding_help_is_listed_in_the_program_help() {
    let (output, _elapsed) = run(&["--help"]);

    assert!(text(&output.stdout).contains("ding"));
}

#[test]
fn plugin_version_matches_crate() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/plugin/.claude-plugin/plugin.json"
    );
    let manifest = std::fs::read_to_string(path).expect("read plugin/.claude-plugin/plugin.json");
    let manifest: Value = serde_json::from_str(&manifest).expect("plugin.json is JSON");
    assert_eq!(
        manifest["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "plugin.json version must equal the crate version"
    );
}
