# Agent-friendly timers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Executed 2026-10-22.** Workflow files ended up in `ci/workflows/` instead of
> `.github/workflows/` (no `workflow` permission in the session); see the spec's "As built" section.

**Goal:** Add terminal-less `remind` and `ding` commands, built-in sounds and desktop notifications
to cli-timer, ship a Claude Code plugin whose skill drives them, and build and test on Windows,
macOS and Linux, all without changing how the full-screen timer works for a human.

**Architecture:** The library crate gains focused modules: `duration` (parsing), `sound`
(built-in and file sources, bounded playback), `alert` (notification and sound behind a trait,
with a quiet implementation for tests), `output` (human and JSON event lines), `remind` and
`ding` (the two commands), and `cli` (clap types). `main.rs` parses and dispatches only. The
existing TUI modules (`app`, `event`, `handler`, `tui`) stay, using `duration` and `sound`.

**Tech Stack:** Rust edition 2024, clap 4 (derive), chrono, rodio 0.16 (kept; upgrade parked),
tui 0.19 + crossterm (kept), notify-rust, serde + serde_json. GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-22-agent-timer-design.md`

## Global Constraints

- Old invocation keeps working: `cli-timer -t hh:mm:ss -s file -l label`. `-s` becomes optional.
- Durations: existing `hh:mm:ss` (1 to 3 colon-separated parts, first part is hours, as today, so
  a bare `5` is 5 hours) plus short forms `90s`, `20m`, `1h30m`, `2h`.
- Reminder notification: title `cli-timer`, body the label, or `Timer finished` with no label.
- `--ring` default 30 s; capped at 10 min (my call). Sound loops for the ring time, then the
  process exits. Nothing rings forever.
- `ding`: short sound played once plus a notification; a second or two at most (a file given with
  `-s` is cut off at 5 s, my call).
- `--json`: one JSON object per line on stdout. Errors: `{"error":"..."}` and a non-zero exit.
  `remind` and `ding` take the duration as a plain `String` and parse it themselves, so a bad
  duration goes through the same error path (exit 1, `{"error":...}` under `--json`), not clap's.
- Longest accepted duration: `MAX_DURATION` = 100 days (my call). Anything longer is an error,
  never a panic in date arithmetic.
- Exit codes (my call): `0` success; `1` error before anything fired (bad duration, unreadable
  sound file); `3` fired, but the notification or the sound failed (the error is on stderr).
  clap's own usage errors keep clap's exit code `2`.
- `remind` and `ding` never touch the terminal (no raw mode, no alternate screen).
- Tests never make noise or pop notifications: `CLI_TIMER_QUIET=1` switches to the quiet alerter.
- House Rust standards: edition 2024; `cargo clippy --all-targets -- -D warnings` clean under the
  `[lints.clippy]` policy; no `unwrap`/`expect` outside tests; `# Errors` docs on fallible public
  functions; tests in a `#[cfg(test)] mod tests` in the same file (CLI end-to-end tests go in
  `tests/cli.rs`, since they drive the binary).
- Before every commit: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`. Linux needs `libasound2-dev` and `pkg-config`.
- Plugin is separate from the user's `claude-setup`; allow rule `Bash(cli-timer:*)`; install
  route `cargo install --locked --git https://github.com/ProgDroid/cli-timer`.
- No `rust-version` pin. Edition 2024 already implies Rust 1.85 or later; CI uses current stable.

## Review Focus

1. **A bad `-s` path given to `remind`** must fail straight away (exit 1, error printed), not two
   hours later when the reminder fires. Test in Task 5.
2. **Malformed or huge durations** (`1.5h`, `30m1h`, `1h1h`, `""`, `abc`, `-5m`, `99999999999h`) give a
   clear error naming the accepted forms, never a panic or overflow. Test in Task 2.
3. **Labels with quotes, backslashes, newlines or non-ASCII** still produce one valid JSON line
   per event. Test in Task 5.
4. **Sound or notification failing at fire time**: the other channel is still delivered, the
   error goes to stderr, and the exit code is `3`. Test in Task 5.
5. **The machine sleeping past the fire time, or the clock jumping**: the reminder fires on the
   first check after wake, because it compares wall-clock time in slices of at most 1 s. Test in
   Task 5.

---

### Task 1: Edition 2024 and the CI check workflow

**Files:**
- Modify: `Cargo.toml` (edition)
- Modify: any `src/*.rs` that `cargo fix --edition` changes
- Create: `.github/workflows/check.yml`
- Modify: `CLAUDE.md` (Commands: note CI)

**Interfaces:**
- Produces: a CI job named `check` that later tasks rely on to prove Windows and macOS builds.

- [ ] **Step 1: Migrate the edition.** Run `cargo fix --edition --allow-dirty`, then set
  `edition = "2024"`. Don't add `rust-version`.
- [ ] **Step 2: Verify.** Run `cargo fmt --all && cargo clippy --all-targets -- -D warnings &&
  cargo test`. Expected: no warnings, `test result: ok`. The macOS audio stack in `Cargo.lock`
  (coreaudio/bindgen) has never been built for this repo. If the macOS job fails there, run
  `cargo update -p <crate>` within rodio 0.16's semver range before anything else, and note the
  result.
- [ ] **Step 3: Write `.github/workflows/check.yml`.** Triggers: `push` (all branches) and
  `pull_request`. Matrix `os: [ubuntu-latest, windows-latest, macos-latest]`, `fail-fast: false`.
  Steps: checkout; install the stable toolchain with `rustfmt` and `clippy`; a cargo cache; on
  Linux only, `sudo apt-get install -y libasound2-dev pkg-config`; then `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`. Job-level
  `env: CLI_TIMER_QUIET: "1"`. Pass `-D warnings` on the clippy command only, never through
  `RUSTFLAGS`. Pin each action to its current major version, found with `core:version-discovery`
  (never `@master` or `@latest`).
- [ ] **Step 4: Note CI in `CLAUDE.md`.** Under Commands add:
  `- CI: .github/workflows/check.yml runs fmt, clippy and test on Linux, Windows and macOS.`
- [ ] **Step 5: Commit and push**, then confirm all three matrix jobs pass on GitHub.

```bash
git add Cargo.toml src .github/workflows/check.yml CLAUDE.md
git commit -m "build: edition 2024 and a cross-platform check workflow"
```

### Task 2: Duration parser

**Files:**
- Create: `src/duration.rs`
- Modify: `src/lib.rs` (add `pub mod duration;`), `src/app.rs` (drop `parse_duration`, use the new parser)

**Interfaces:**
- Produces: `pub fn parse(arg: &str) -> Result<std::time::Duration, DurationError>`;
  `#[derive(Debug, thiserror::Error)] pub enum DurationError` whose `Display` names the accepted
  forms (e.g. `invalid duration "1.5h": use hh:mm:ss or forms like 90s, 20m, 1h30m`), plus an
  overflow variant (also returned above `MAX_DURATION`).
  `pub const MAX_DURATION: Duration = Duration::from_secs(100 * 24 * 3600);`. A clap-compatible
  wrapper `pub fn parse_arg(arg: &str) -> Result<std::time::Duration, String>`, used only by the
  TUI's `-t`.

- [ ] **Step 1: Write the failing tests** in `src/duration.rs`:

```rust
#[test]
fn colon_forms_keep_todays_meaning() {
    assert_eq!(parse("00:20:00").unwrap(), Duration::from_secs(20 * 60));
    assert_eq!(parse("1:30").unwrap(), Duration::from_secs(90 * 60)); // hh:mm
    assert_eq!(parse("5").unwrap(), Duration::from_secs(5 * 3600)); // bare number = hours, as today
}

#[test]
fn short_forms() {
    assert_eq!(parse("90s").unwrap(), Duration::from_secs(90));
    assert_eq!(parse("20m").unwrap(), Duration::from_secs(1200));
    assert_eq!(parse("2h").unwrap(), Duration::from_secs(7200));
    assert_eq!(parse("1h30m").unwrap(), Duration::from_secs(5400));
    assert_eq!(parse("1h0m5s").unwrap(), Duration::from_secs(3605));
    assert_eq!(parse("0s").unwrap(), Duration::ZERO);
}

#[test]
fn rejects_malformed() {
    for bad in ["", "abc", "1.5h", "30m1h", "1h1h", "-5m", "5x", "1h 30m", "::"] {
        assert!(parse(bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn rejects_overflow_without_panicking() {
    assert!(matches!(parse("99999999999999999999h"), Err(DurationError::Overflow(_))));
    assert!(matches!(parse("9999999999999999:00:00"), Err(DurationError::Overflow(_))));
    assert!(matches!(parse("99999999999h"), Err(DurationError::Overflow(_)))); // fits u64, panics chrono
    assert!(matches!(parse("2401h"), Err(DurationError::Overflow(_)))); // just over 100 days
    assert!(parse("2400h").is_ok());
}
```

- [ ] **Step 2: Run** `cargo test duration`. Expected: FAIL, `parse` not found.
- [ ] **Step 3: Implement `parse`.** Add `thiserror` with `cargo add thiserror`. Short form: one
  or more `<digits><unit>` with units `h`, `m`, `s`, each at most once and in that order, no
  whitespace. Anything containing `:` or only digits goes to the colon parser. Use checked
  arithmetic throughout. Then replace `app::parse_duration` with `duration::parse_arg` on `-t`,
  converting to `chrono::Duration` with `chrono::Duration::from_std` where `App` needs it.
- [ ] **Step 4: Run** `cargo test`. Expected: PASS. Then clippy, `-D warnings`.
- [ ] **Step 5: Commit.**

```bash
git add Cargo.toml Cargo.lock src/duration.rs src/lib.rs src/app.rs
git commit -m "feat: short-form durations (90s, 20m, 1h30m) alongside hh:mm:ss"
```

### Task 3: Sounds: built-in sources and bounded playback

**Files:**
- Create: `src/sound.rs`
- Modify: `src/lib.rs`, `src/app.rs` (`-s` optional, use `sound`), `src/main.rs` if `Args` moves

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub enum Sound { Alarm, Chime, File(PathBuf) }`.
  - `pub enum Playback { Once { max: Duration }, Loop { for_: Duration } }`.
  - `pub fn source(sound: &Sound) -> Result<Box<dyn Source<Item = f32> + Send>, SoundError>`:
    opens and decodes a file eagerly, so a bad path or format fails here.
  - `pub fn check(sound: &Sound) -> Result<(), SoundError>`, the same validation without playing.
  - `pub fn play(sound: &Sound, playback: Playback) -> Result<(), SoundError>`: blocks until done.
  - `pub const MAX_RING: Duration = Duration::from_secs(600);`
    `pub const DEFAULT_RING: Duration = Duration::from_secs(30);`
    `pub const DING_FILE_MAX: Duration = Duration::from_secs(5);`
  - `#[derive(Debug, thiserror::Error)] pub enum SoundError` (open, decode, output device).

- [ ] **Step 1: Write failing tests** in `src/sound.rs`. They must not need an audio device:

```rust
#[test]
fn alarm_is_a_few_seconds_and_chime_is_short() {
    let alarm: Vec<f32> = source(&Sound::Alarm).unwrap().collect();
    let chime: Vec<f32> = source(&Sound::Chime).unwrap().collect();
    let secs = |n: usize| n as f32 / (SAMPLE_RATE as f32);
    assert!((1.0..=4.0).contains(&secs(alarm.len())));
    assert!((0.2..=1.5).contains(&secs(chime.len())));
    assert!(alarm.iter().chain(&chime).all(|s| s.abs() <= 1.0));
}

#[test]
fn missing_file_fails_at_check_time() {
    assert!(check(&Sound::File("definitely/not/here.wav".into())).is_err());
}

#[test]
fn ring_time_is_capped() {
    assert_eq!(ring_for(Duration::from_secs(3600)), MAX_RING);
    assert_eq!(ring_for(Duration::from_secs(5)), Duration::from_secs(5));
}
```

- [ ] **Step 2: Run** `cargo test sound`. Expected: FAIL.
- [ ] **Step 3: Implement.** Mono at `pub const SAMPLE_RATE: u32 = 48_000` (rodio 0.16's
  `SineWave` is fixed at 48 kHz, checked in `rodio-0.16.0/src/source/sine.rs`). The alarm is three
  880 Hz beeps of 150 ms with 100 ms gaps, then 600 ms of silence, about 1.35 s per cycle; `Loop`
  repeats it. The chime is 660 Hz for 180 ms, then 990 Hz for 300 ms, with 10 ms fades at each
  edge. Synthesise both as a `Vec<f32>` (sine at amplitude 0.3), wrapped in
  `rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, samples)`: it is deterministic, needs no
  device, and concatenating is just extending the vector. Files:
  `Decoder::new(BufReader::new(File::open(..)?))?.convert_samples()`. In `play`, `Loop` uses
  `repeat_infinite().take_duration(for_)` and `Once` uses `take_duration(max)`; then
  `sink.sleep_until_end()`. Add `pub fn ring_for(requested: Duration) -> Duration` (min with
  `MAX_RING`).
- [ ] **Step 4: Move the TUI onto it.** In `app.rs`, `-s` becomes `Option<PathBuf>` (`None` maps
  to `Sound::Alarm`). `start_sound` builds its source with `sound::source` and keeps today's
  behaviour of looping until a key is pressed. Replace the busy `try_recv` loop with a blocking
  `rx.recv()`, so the thread no longer spins a CPU core while ringing.
- [ ] **Step 5: Run** `cargo test`, clippy. Smoke-test the TUI without `-s` in a terminal
  (`cargo run -- -t 3s`): it counts down and rings the built-in alarm until `q`.
- [ ] **Step 6: Commit.**

```bash
git add src/sound.rs src/lib.rs src/app.rs src/main.rs
git commit -m "feat: built-in alarm and chime; -s is now optional"
```

### Task 4: Alerts: notifications and sound behind one trait

**Files:**
- Create: `src/alert.rs`
- Modify: `src/lib.rs`, `Cargo.toml` (`cargo add notify-rust`)

**Interfaces:**
- Consumes: `sound::{Sound, Playback, play}` (Task 3).
- Produces:
  - `pub trait Alerter { fn notify(&self, title: &str, body: &str) -> Result<(), AlertError>;
    fn play(&self, sound: &Sound, playback: Playback) -> Result<(), AlertError>; }`
  - `pub struct SystemAlerter;` (notify-rust plus `sound::play`)
  - `pub struct QuietAlerter<W: Write> { log: RefCell<W> }`, which writes one line per call:
    `quiet notify: <title>: <body>` and `quiet play: <Sound:?> <Playback:?>`.
  - `pub fn from_env() -> Box<dyn Alerter>`: `QuietAlerter` on stderr when `CLI_TIMER_QUIET` is
    set and not `0`, otherwise `SystemAlerter`.
  - `pub const TITLE: &str = "cli-timer";`
  - `#[derive(Debug, thiserror::Error)] pub enum AlertError { Notify(String), Sound(#[from] SoundError) }`

- [ ] **Step 1: Write failing tests:** `QuietAlerter` over a `Vec<u8>` records exactly
  `quiet notify: cli-timer: check the oven\n` for `notify("cli-timer", "check the oven")`, and a
  `quiet play:` line for `play`. `from_env` honours `CLI_TIMER_QUIET=1` and ignores
  `CLI_TIMER_QUIET=0`; take the env reading as a parameter (`from_env_value(Option<&str>)`) so
  the test doesn't mutate process env, which is `unsafe` in edition 2024.
- [ ] **Step 2: Run** `cargo test alert`. Expected: FAIL.
- [ ] **Step 3: Implement.** On Windows and Linux, `SystemAlerter::notify` uses
  `notify_rust::Notification::new().summary(title).body(body).show()`; map its error to
  `AlertError::Notify`, and never `unwrap`. No app-id registration on Windows (the PowerShell
  attribution is accepted). On macOS, don't use notify-rust: its `show()` always returns `Ok`,
  drops errors, and uses an API deprecated from macOS 14 (plan red-team). Run
  `osascript -e 'on run argv' -e 'display notification (item 2 of argv) with title (item 1 of argv)' -e 'end run' <title> <body>`,
  passing title and body as arguments (never spliced into the script), and map a non-zero exit
  to `AlertError::Notify`. Make notify-rust a `cfg(not(target_os = "macos"))` dependency.
- [ ] **Step 4: Run** `cargo test` and clippy. Confirm the `check` workflow still builds on all
  three OSes after pushing, since notify-rust pulls in per-OS backends.
- [ ] **Step 5: Commit.**

```bash
git add Cargo.toml Cargo.lock src/alert.rs src/lib.rs
git commit -m "feat: desktop notifications and a quiet alerter for tests"
```

### Task 5: The `remind` command, CLI restructure and output events

**Files:**
- Create: `src/cli.rs`, `src/output.rs`, `src/remind.rs`, `tests/cli.rs`
- Modify: `src/lib.rs`, `src/main.rs`, `src/app.rs` (`Args` moves to `cli.rs` as `TuiArgs`),
  `Cargo.toml` (`cargo add serde --features derive`, `cargo add serde_json`)

**Interfaces:**
- Consumes: `duration::parse_arg` (Task 2), `sound::{Sound, Playback, check, ring_for,
  DEFAULT_RING}` (Task 3), `alert::{Alerter, TITLE, from_env}` (Task 4).
- Produces:
  - `cli.rs`: `#[derive(Parser)] #[command(version, about, args_conflicts_with_subcommands = true)]
    pub struct Cli { #[command(subcommand)] pub command: Option<Command>, #[command(flatten)]
    pub tui: TuiArgs }`; `TuiArgs { -t time: Option<Duration>, -s sound: Option<PathBuf>,
    -l label: Option<String> }`; `pub enum Command { Remind(RemindArgs) }` (Task 6 adds `Ding(DingArgs)`);
    `RemindArgs { duration: String (positional), -l label, -s sound, --ring: String (default
    "30s"), --json }`. Both strings are parsed by `duration::parse` inside `run`.
    With no subcommand and no `-t`, `main` raises clap's `MissingRequiredArgument` error.
  - `output.rs`: `#[derive(Serialize)] #[serde(tag = "event", rename_all = "lowercase")]
    pub enum Event { Set { label: Option<String>, fires_at: DateTime<Local> }, Fired { label:
    Option<String> }, Dinged { message: Option<String> } }`;
    `pub fn write_event(out: &mut impl Write, event: &Event, json: bool) -> io::Result<()>`;
    `pub fn write_error(out: &mut impl Write, err: &str, json: bool) -> io::Result<()>`. Human
    lines: `Reminder set: "check the oven" at 14:32:05`, `Reminder fired: "check the oven"`, and
    `Ding: "Tests passed"` (no quotes part when there is no label or message).
  - `remind.rs`: `pub struct Reminder { pub label: Option<String>, pub sound: Sound, pub ring:
    Duration, pub fires_at: DateTime<Local> }`;
    `pub fn wait_until(fires_at: DateTime<Local>, now: impl Fn() -> DateTime<Local>, sleep: impl
    FnMut(Duration))`, which sleeps in slices of at most 1 s, re-reading `now()` each time;
    `pub fn fire(r: &Reminder, alerter: &dyn Alerter) -> Outcome`, where `pub enum Outcome { Ok,
    Partial(Vec<AlertError>) }`; `pub fn run(args: RemindArgs, alerter: &dyn Alerter, out: &mut
    impl Write) -> u8`.

- [ ] **Step 1: Write failing unit tests.**
  - `cli.rs`: `Cli::try_parse_from(["cli-timer", "-t", "00:00:05", "-s", "x.wav", "-l", "hi"])`
    parses to `command: None` with all three TUI fields set; `["cli-timer", "remind", "20m",
    "-l", "oven"]` parses to `Remind` with ring 30 s and `json: false`; `["cli-timer", "-t",
    "5m", "remind", "1m"]` is an error; `duration::parse("30s")` equals `sound::DEFAULT_RING`
    (so the clap default string and the constant can't drift).
  - `output.rs`: the JSON for `Set` with label `he said "hi"\n\\ ünï` is one line, contains no
    raw newline, and round-trips through `serde_json::from_str::<serde_json::Value>` with
    `event == "set"` and the label intact.
  - `remind.rs`: `wait_until` with a fake clock that jumps forward 2 h on the first `sleep` (a
    machine sleeping) returns after that one sleep; with a normal fake clock 3 s short of the
    deadline, it makes exactly three sleeps of at most 1 s each. `fire` with a test alerter
    whose `play` fails still calls `notify` and returns `Partial` with one error, and the body
    is `Timer finished` when the label is `None`.
- [ ] **Step 2: Run** `cargo test`. Expected: FAIL (modules missing).
- [ ] **Step 3: Implement** `cli.rs`, `output.rs` and `remind.rs`. `run` order:
  1. Parse `duration` and `ring` with `duration::parse`; compute `fires_at` with
     `Local::now().checked_add_signed(..)`, which errors rather than panics. Then
     `sound::check(&args.sound)`. On any error, write it with `write_error` and return exit `1`
     before printing `set`.
  2. Print `set`, then flush.
  3. `wait_until`.
  4. `fire`: notify first, then play `Loop { for_: ring_for(args.ring) }`.
  5. Print `fired`. Return `0`, or `3` on `Partial`, with each error on stderr.
- [ ] **Step 4: Rewrite `main.rs` as dispatch only** (it turns `run`'s `u8` into `std::process::ExitCode`, which tests cannot compare). `Remind` goes to `remind::run(args,
  &*alert::from_env(), &mut stdout())`; no subcommand runs the existing TUI loop unchanged.
- [ ] **Step 5: Write failing end-to-end tests** in `tests/cli.rs`. Run
  `env!("CARGO_BIN_EXE_cli-timer")` with `CLI_TIMER_QUIET=1` and stdin null:
  - `remind 1s --ring 1s --json -l "check the oven"`: exit 0; stdout is exactly two JSON lines,
    `set` then `fired`, both labelled; stderr contains `quiet notify: cli-timer: check the oven`;
    the elapsed time is between 1 s and 5 s.
  - `remind 1s -s nope.wav --json`: exit 1 in under 1 s; stdout is one `{"error":...}` line and
    no `set` line.
  - `remind 1.5h --json`: exit exactly 1; stdout is exactly one `{"error":...}` line naming the
    accepted forms.
  - `remind 99999999999h --json`: exit exactly 1 with an `{"error":...}` line, and no panic
    (stderr has no `panicked`).
  - `-t 99999999999h`: exits non-zero without panicking (clap error from `parse_arg`), before
    touching the terminal.
  - With no arguments: exit 2 (clap usage error).
- [ ] **Step 6: Run** `cargo test`. Expected: PASS on Linux; push and confirm `check` passes on
  Windows and macOS.
- [ ] **Step 7: Commit.**

```bash
git add Cargo.toml Cargo.lock src tests
git commit -m "feat: remind command with JSON output for agents"
```

### Task 6: The `ding` command

**Files:**
- Create: `src/ding.rs`
- Modify: `src/cli.rs` (`DingArgs { message: Option<String> (positional), -s sound, --json }`),
  `src/lib.rs`, `src/main.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `output::{Event::Dinged, write_event, write_error}`, `alert::{Alerter, TITLE}`,
  `sound::{Sound, Playback, check, DING_FILE_MAX}`.
- Produces: `pub fn run(args: DingArgs, alerter: &dyn Alerter, out: &mut impl Write) -> u8`.

- [ ] **Step 1: Write failing tests.**
  - Unit: with no `-s`, the sound is `Sound::Chime` and the playback is `Once { max: 2 s }`. With
    `-s file`, it is `Once { max: DING_FILE_MAX }`. The notification body is the message, or
    `Done` with none (my call).
  - End-to-end: `ding --json "Tests passed"` exits 0 in under 3 s, prints one
    `{"event":"dinged","message":"Tests passed"}` line, and stderr has
    `quiet notify: cli-timer: Tests passed`. `ding -s nope.wav` exits 1.
- [ ] **Step 2: Run** `cargo test ding`. Expected: FAIL.
- [ ] **Step 3: Implement** `ding::run`, with the same order and exit codes as `remind::run` and
  no wait. Wire it into `main.rs`.
- [ ] **Step 4: Run** `cargo test` and clippy. Expected: PASS.
- [ ] **Step 5: Commit.**

```bash
git add src tests
git commit -m "feat: ding command for on-request done alerts"
```

### Task 7: Claude Code plugin and skill

**Files:**
- Create: `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `skills/timer/SKILL.md`

**Interfaces:**
- Consumes: the CLI surface from Tasks 5 and 6, exactly as built (`remind <duration> [-l] [-s]
  [--ring] [--json]`, `ding [message] [-s] [--json]`, the exit codes, the `set`, `fired` and
  `dinged` lines).

- [ ] **Step 1: Write the manifests.** `plugin.json` has name `cli-timer`, the version matching
  `Cargo.toml`, a description and the repository URL. `marketplace.json` is a single-plugin
  marketplace named `cli-timer` whose one entry points at `./`. Follow the current Claude Code
  plugin docs (`core:mcp-reference` and `personal:cloud-sessions` cover this user's
  conventions).
- [ ] **Step 2: Write `skills/timer/SKILL.md`.** Its `description` triggers on "remind / ping /
  alert me in X", "set a timer", "ding / let me know when it's done". Its body covers each
  point in the spec's "Agent integration" list:
  - Check `cli-timer --version` first. If it's missing, or older than `0.2.0` (no `remind`),
    give both install and upgrade routes, and stop.
  - Run `remind ... --json` as a background shell command. Don't hard-code Claude Code timeouts
    or tool names: tell the agent to read its own Bash tool's stated background limits, and in
    an unattended run either size the timeout to cover duration + ring + 1 min, or decline
    plainly.
  - Confirm in one line from the `set` line; report `fired` if told the task finished.
  - Prefer cli-timer over the agent's own scheduling tools when the user wants to be alerted.
  - List and cancel through the agent's background-task tools.
  - For done alerts, run `cli-timer --version` at request time (surfacing any permission prompt
    while the user is there) and recommend `Bash(cli-timer:*)`. Run `ding` as the very last
    step, success or failure, with a message saying which. The first time, tell the user an
    abnormal end skips it.
  - In a cloud session, say the alert can't reach the user.
- [ ] **Step 3: Validate.** Run `claude plugin validate .`. Expected: no errors. Add a test
  `plugin_version_matches_crate` in `tests/cli.rs`: it reads `.claude-plugin/plugin.json` and
  asserts its `version` equals `env!("CARGO_PKG_VERSION")`, so the two can't drift silently.
- [ ] **Step 4 (moved to Task 9, which runs on the user's PC): Try it.** Install from the local checkout (`/plugin marketplace add <path>`,
  then install `cli-timer@cli-timer`) in a throwaway local session. Ask "remind me in 1 minute
  to stretch", then "ding me when you've listed this directory". Confirm the skill triggers and
  uses `remind`/`ding` with `--json` in the background as described. (Locally with
  `CLI_TIMER_QUIET=1` if no audio.)
- [ ] **Step 5: Commit.**

```bash
git add .claude-plugin skills
git commit -m "feat: Claude Code plugin with a timer skill"
```

### Task 8: Release workflow, README and version

**Files:**
- Delete: `.github/workflows/build.yml`
- Create: `.github/workflows/release.yml`
- Modify: `README.md`, `Cargo.toml` (version `0.2.0`), `.claude-plugin/plugin.json` (version
  `0.2.0`)

- [ ] **Step 1: Write `release.yml`.** Triggers: `push` of tags `v*`, and `workflow_dispatch`
  as a dry run that builds and uploads workflow artifacts but publishes no release. That way the
  first real tag isn't the first run. `permissions: contents: write`. A first job fails unless
  the tag equals `v` + the `Cargo.toml` version (skipped on dry runs). Build matrix on native
  runners, each running `rustup target add <t>` first:
  - `windows-latest` / `x86_64-pc-windows-msvc`, packaged as `.zip`;
  - `macos-latest` / `aarch64-apple-darwin` and `x86_64-apple-darwin`, packaged as `.tar.gz`;
  - `ubuntu-22.04` (older glibc, so the binary runs on more distros; my call) /
    `x86_64-unknown-linux-gnu`, packaged as `.tar.gz`, with the ALSA packages installed
    (`sudo apt-get update && sudo apt-get install -y libasound2-dev pkg-config`).

  Each runs `cargo build --release --locked --target <t>`, then uploads the archive plus a
  SHA-256 file. A final job creates the GitHub release for the tag and attaches every archive
  (e.g. `softprops/action-gh-release`). Pin each action to its current major version
  (`core:version-discovery`).
- [ ] **Step 1b (moved to Task 9): Dry-run it** with `workflow_dispatch` once it's on `main`
  (GitHub only dispatches workflows on the default branch), and confirm all four archives are
  produced before any tag is pushed.
- [ ] **Step 2: Lint the workflows** with `actionlint` if available (otherwise review by hand
  against the action's docs). Expected: no findings.
- [ ] **Step 3: Rewrite `README.md`**, keeping its tone. Sections:
  - the full-screen timer (old usage, `-s` now optional, the short durations);
  - `remind` and `ding`, with one example each, including `--json`;
  - install: `cargo install --locked --git https://github.com/ProgDroid/cli-timer` (needs Rust; Linux
    also needs `libasound2-dev` and `pkg-config`), or download from Releases;
  - the Claude Code plugin: `/plugin marketplace add ProgDroid/cli-timer`, then
    `/plugin install cli-timer@cli-timer`, plus the allow rule `Bash(cli-timer:*)`, and a note
    that alerts only reach you in local sessions;
  - a short "Tested" note: real sound and pop-ups are checked by hand on Windows. macOS and Linux
    are built and tested in CI with alerts in quiet mode, but their real pop-ups and sound have
    not yet been checked by a person.
- [ ] **Step 4: Bump to `0.2.0`** in `Cargo.toml` and `plugin.json`. Run `cargo build` to update
  `Cargo.lock`, then `cargo test`.
- [ ] **Step 5: Commit.** Tagging `v0.2.0` happens after merge to `main` and is the user's call.

```bash
git add .github/workflows README.md Cargo.toml Cargo.lock .claude-plugin/plugin.json
git rm .github/workflows/build.yml
git commit -m "build: tag-triggered native release builds; README for remind, ding and the plugin"
```

### Task 9: Acceptance on the user's Windows PC

This is manual. CI cannot hear sounds, see pop-ups or drive Claude Code. The user runs it on
their PC with the branch built (`cargo install --path .`) and the plugin installed from the
checkout.

- [ ] **Step 1: Hand the user this checklist** (put it in `NEXT.md` under "Try this"):
  1. In Claude Code: "remind me in 1 minute to stretch". It confirms; a minute later a toast says
     "stretch" and the alarm loops about 30 s, then stops. No console window appears.
  2. "remind me in 5 minutes", then "cancel that reminder". It never fires, and Task Manager
     shows no `cli-timer.exe` left running.
  3. "remind me in 1 minute" and let it ring, then stop the task while it rings. The sound stops
     and no `cli-timer.exe` remains.
  4. "ding me when you've listed this folder". You get a chime and a toast, with no permission
     prompt once the allow rule is set.
  5. Note whether Claude told you the reminder fired, unprompted.
  6. `cli-timer -t 00:00:05` in a terminal behaves as before, with the built-in alarm.
  7. The plugin trial from Task 7 Step 4.
  8. After merging to `main` and before tagging: dry-run `release.yml` with `workflow_dispatch`
     and confirm all four archives are produced.
- [ ] **Step 2: If 2 or 3 leave `cli-timer.exe` running**, make `remind` exit when its parent
  process is gone. Poll the parent pid once a second in `wait_until`'s loop and during ringing;
  on Windows, open the parent with `OpenProcess` and check `WaitForSingleObject(h, 0)`. Add a
  test that fakes the parent check. Re-run the checklist.
- [ ] **Step 3: Record the results** in `NEXT.md`, including item 5's answer. If the agent isn't
  told, make sure the skill doesn't promise it. Commit as `handoff: ...`.
