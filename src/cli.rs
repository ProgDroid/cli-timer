//! The command line: the full-screen timer (no subcommand) and the `remind` subcommand.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

/// Exit code when everything worked.
pub const EXIT_OK: u8 = 0;
/// Exit code for an error before anything fired (bad duration, unreadable sound file).
pub const EXIT_ERROR: u8 = 1;
/// Exit code when the reminder fired but its notification or sound failed.
pub const EXIT_PARTIAL: u8 = 3;

/// The default of `remind --ring`, as clap needs it: text, parsed by `duration::parse` in `run`.
/// A test keeps it equal to [`DEFAULT_RING`].
const DEFAULT_RING_ARG: &str = "30s";

#[derive(Parser)]
#[command(
    version,
    about = "A countdown timer for the terminal, with background reminders and done alerts for agents",
    override_usage = "cli-timer -t <TIME> [-s <FILE>] [-l <LABEL>]\n       cli-timer <COMMAND>",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub tui: TuiArgs,
}

// A plain comment, not a doc comment: clap would show a doc comment on a flattened struct as the
// program's description in `--help`. This is the full-screen timer's arguments, the invocation
// that predates the subcommands.
#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Timer duration: hh:mm:ss, or forms like 90s, 20m, 1h30m
    #[arg(short, value_parser = crate::duration::parse_arg)]
    pub time: Option<Duration>,

    /// Path to the sound file to use (default: a built-in alarm)
    #[arg(short, value_name = "FILE")]
    pub sound: Option<PathBuf>,

    /// An optional label for when the timer goes off
    #[arg(short)]
    pub label: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Wait without a screen, then notify and ring; prints one line per event
    Remind(RemindArgs),
    /// Notify and chime once, now: a short "done" alert for an agent to give on request
    Ding(DingArgs),
}

#[derive(Args, Debug)]
pub struct RemindArgs {
    /// How long to wait: hh:mm:ss, or forms like 90s, 20m, 1h30m
    pub duration: String,

    /// What to say when the time is up (default: "Timer finished")
    #[arg(short, allow_hyphen_values = true)]
    pub label: Option<String>,

    /// Path to the sound file to use (default: a built-in alarm)
    #[arg(short, value_name = "FILE")]
    pub sound: Option<PathBuf>,

    /// How long the sound rings, at most 10m
    #[arg(long, default_value = DEFAULT_RING_ARG)]
    pub ring: String,

    /// Print one JSON object per line instead of text
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct DingArgs {
    /// What to say (default: "Done"); put `--` before it if it starts with a hyphen
    #[arg(allow_hyphen_values = true)]
    pub message: Option<String>,

    /// Path to the sound file to use, cut off after 5s (default: a built-in chime)
    #[arg(short, value_name = "FILE")]
    pub sound: Option<PathBuf>,

    /// Print one JSON object per line instead of text
    #[arg(long)]
    pub json: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration;
    use crate::sound::DEFAULT_RING;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn old_invocation_parses_to_tui_args() {
        let cli = parse(&["cli-timer", "-t", "00:00:05", "-s", "x.wav", "-l", "hi"])
            .expect("old invocation parses");
        assert!(cli.command.is_none());
        assert_eq!(cli.tui.time, Some(Duration::from_secs(5)));
        assert_eq!(cli.tui.sound, Some(PathBuf::from("x.wav")));
        assert_eq!(cli.tui.label.as_deref(), Some("hi"));
    }

    #[test]
    fn tui_sound_and_label_are_optional() {
        let cli = parse(&["cli-timer", "-t", "20m"]).expect("parses");
        assert_eq!(cli.tui.time, Some(Duration::from_mins(20)));
        assert!(cli.tui.sound.is_none());
        assert!(cli.tui.label.is_none());
    }

    #[test]
    fn no_arguments_parse_to_nothing_for_main_to_reject() {
        let cli = parse(&["cli-timer"]).expect("parses");
        assert!(cli.command.is_none());
        assert!(cli.tui.time.is_none());
    }

    #[test]
    fn bad_tui_time_is_a_clap_error() {
        assert!(parse(&["cli-timer", "-t", "1.5h"]).is_err());
        assert!(parse(&["cli-timer", "-t", "99999999999h"]).is_err());
    }

    #[test]
    fn remind_parses_with_defaults() {
        let cli = parse(&["cli-timer", "remind", "20m", "-l", "oven"]).expect("parses");
        let Some(Command::Remind(args)) = cli.command else {
            panic!("expected remind");
        };
        assert_eq!(args.duration, "20m");
        assert_eq!(args.label.as_deref(), Some("oven"));
        assert!(args.sound.is_none());
        assert_eq!(args.ring, "30s");
        assert!(!args.json);
    }

    #[test]
    fn remind_parses_every_option() {
        let cli = parse(&[
            "cli-timer",
            "remind",
            "1h30m",
            "-l",
            "x",
            "-s",
            "a.mp3",
            "--ring",
            "2m",
            "--json",
        ])
        .expect("parses");
        let Some(Command::Remind(args)) = cli.command else {
            panic!("expected remind");
        };
        assert_eq!(args.duration, "1h30m");
        assert_eq!(args.sound, Some(PathBuf::from("a.mp3")));
        assert_eq!(args.ring, "2m");
        assert!(args.json);
    }

    #[test]
    fn remind_takes_a_malformed_duration_as_text() {
        // It is parsed in `run`, so that a bad one takes the same error path as everything else.
        let cli = parse(&["cli-timer", "remind", "1.5h"]).expect("clap accepts any text");
        let Some(Command::Remind(args)) = cli.command else {
            panic!("expected remind");
        };
        assert_eq!(args.duration, "1.5h");
    }

    #[test]
    fn remind_needs_a_duration() {
        assert!(parse(&["cli-timer", "remind"]).is_err());
    }

    #[test]
    fn ding_parses_with_defaults() {
        let cli = parse(&["cli-timer", "ding"]).expect("parses");
        let Some(Command::Ding(args)) = cli.command else {
            panic!("expected ding");
        };
        assert!(args.message.is_none());
        assert!(args.sound.is_none());
        assert!(!args.json);
    }

    #[test]
    fn ding_parses_every_option() {
        let cli =
            parse(&["cli-timer", "ding", "Tests passed", "-s", "a.mp3", "--json"]).expect("parses");
        let Some(Command::Ding(args)) = cli.command else {
            panic!("expected ding");
        };
        assert_eq!(args.message.as_deref(), Some("Tests passed"));
        assert_eq!(args.sound, Some(PathBuf::from("a.mp3")));
        assert!(args.json);
    }

    #[test]
    fn ding_takes_an_empty_message_as_text() {
        let cli = parse(&["cli-timer", "ding", ""]).expect("parses");
        let Some(Command::Ding(args)) = cli.command else {
            panic!("expected ding");
        };
        assert_eq!(args.message.as_deref(), Some(""));
    }

    fn ding_args(args: &[&str]) -> DingArgs {
        let cli = parse(args).expect("parses");
        let Some(Command::Ding(args)) = cli.command else {
            panic!("expected ding");
        };
        args
    }

    fn remind_args(args: &[&str]) -> RemindArgs {
        let cli = parse(args).expect("parses");
        let Some(Command::Remind(args)) = cli.command else {
            panic!("expected remind");
        };
        args
    }

    #[test]
    fn ding_message_may_start_with_a_hyphen() {
        let args = ding_args(&["cli-timer", "ding", "-5 tests failed"]);
        assert_eq!(args.message.as_deref(), Some("-5 tests failed"));
        let args = ding_args(&["cli-timer", "ding", "-x"]);
        assert_eq!(args.message.as_deref(), Some("-x"));
        assert!(!args.json);
    }

    #[test]
    fn ding_hyphen_message_leaves_json_alone_in_either_order() {
        let args = ding_args(&["cli-timer", "ding", "--json", "-x"]);
        assert_eq!(args.message.as_deref(), Some("-x"));
        assert!(args.json);
        let args = ding_args(&["cli-timer", "ding", "-x", "--json"]);
        assert_eq!(args.message.as_deref(), Some("-x"));
        assert!(args.json);
        let args = ding_args(&["cli-timer", "ding", "--json", "--", "-5 tests failed"]);
        assert_eq!(args.message.as_deref(), Some("-5 tests failed"));
        assert!(args.json);
    }

    #[test]
    fn ding_json_alone_is_not_taken_as_the_message() {
        let args = ding_args(&["cli-timer", "ding", "--json"]);
        assert!(args.message.is_none());
        assert!(args.json);
    }

    #[test]
    fn ding_hyphen_message_still_allows_a_sound_file() {
        let args = ding_args(&["cli-timer", "ding", "-s", "a.wav", "-x", "--json"]);
        assert_eq!(args.sound, Some(PathBuf::from("a.wav")));
        assert_eq!(args.message.as_deref(), Some("-x"));
        assert!(args.json);
    }

    #[test]
    fn remind_label_may_start_with_a_hyphen() {
        let args = remind_args(&["cli-timer", "remind", "0s", "-l", "-oops", "--json"]);
        assert_eq!(args.label.as_deref(), Some("-oops"));
        assert!(args.json);
        let args = remind_args(&["cli-timer", "remind", "0s", "--json", "-l", "-oops"]);
        assert_eq!(args.label.as_deref(), Some("-oops"));
        assert!(args.json);
        let args = remind_args(&["cli-timer", "remind", "0s", "-l=-oops"]);
        assert_eq!(args.label.as_deref(), Some("-oops"));
    }

    #[test]
    fn every_sound_option_is_named_file_in_help() {
        let mut cli = Cli::command();
        let mut helps = vec![cli.render_help().to_string()];
        for name in ["remind", "ding"] {
            helps.push(
                cli.find_subcommand_mut(name)
                    .expect("subcommand exists")
                    .render_help()
                    .to_string(),
            );
        }
        for help in helps {
            assert!(help.contains("-s <FILE>"), "{help}");
            assert!(!help.contains("<SOUND>"), "{help}");
        }
    }

    #[test]
    fn tui_flags_conflict_with_a_subcommand() {
        assert!(parse(&["cli-timer", "-t", "5m", "remind", "1m"]).is_err());
    }

    #[test]
    fn clap_default_ring_equals_the_constant() {
        assert_eq!(
            duration::parse(DEFAULT_RING_ARG).expect("default parses"),
            DEFAULT_RING
        );
    }

    #[test]
    fn thirty_seconds_is_the_default_ring() {
        assert_eq!(duration::parse("30s").expect("parses"), DEFAULT_RING);
    }

    fn help() -> String {
        Cli::command().render_help().to_string()
    }

    #[test]
    fn help_has_a_user_facing_description_and_no_developer_comment() {
        let help = help();
        assert!(
            help.contains("A countdown timer for the terminal"),
            "{help}"
        );
        // The doc comment of `TuiArgs` is for developers; clap must not show it.
        assert!(!help.contains("predates"), "{help}");
    }

    #[test]
    fn help_usage_shows_that_t_is_needed_without_a_subcommand() {
        let help = help();
        assert!(help.contains("cli-timer -t <TIME>"), "{help}");
        assert!(help.contains("cli-timer <COMMAND>"), "{help}");
        assert!(!help.contains("[OPTIONS]"), "{help}");
    }

    #[test]
    fn subcommand_help_shows_no_developer_comment() {
        for name in ["remind", "ding"] {
            let mut cli = Cli::command();
            let help = cli
                .find_subcommand_mut(name)
                .expect("subcommand exists")
                .render_help()
                .to_string();
            assert!(!help.contains("predates"), "{help}");
        }
    }

    #[test]
    fn exit_codes_are_the_documented_ones() {
        assert_eq!((EXIT_OK, EXIT_ERROR, EXIT_PARTIAL), (0, 1, 3));
    }
}
