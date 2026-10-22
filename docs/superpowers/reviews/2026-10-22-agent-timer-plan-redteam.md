# Red-team review: agent-timer implementation plan

2026-10-22. Reviewing `docs/superpowers/plans/2026-10-22-agent-timer.md` against
`docs/superpowers/specs/2026-10-22-agent-timer-design.md`, at HEAD `3b8224d`. Stance: a hostile staff
engineer looking for reasons this plan fails. I accept the spec's decisions, including the two earlier
design reviews as the spec answered them. I attack the plan's tasks, tests and dependency assumptions,
and I don't repeat points those reviews already made.

## Verdict

The task breakdown is sound and the Review Focus list names the right risks. Three things go wrong.
First, two of those risks are "tested" by tests that cannot fail: bad durations and huge durations get
through the plan's own tests while breaking its exit-code and JSON contract. Second, on macOS,
notify-rust's `show()` never reports a failure, and no automated or manual step ever shows a real
notification on macOS or Linux. Third, the binary, the plugin and the release workflow drift apart
with nothing to catch it, and the release workflow runs for the first time on the real tag.

## What I checked

All dependency claims were checked against the sources in `~/.cargo/registry` (index.crates.io), not
from memory, unless the "How" column says otherwise. "Scratch" means a copy of the repo in my
scratchpad, built offline with the locked dependencies and rustc/cargo/clippy 1.97.0 (2026-07-07). The
repo itself was not modified, apart from this file.

| Claim the plan relies on | How | Result |
|---|---|---|
| Edition 2024 migration of the tui 0.19 / crossterm 0.25 code is cheap | Scratch with `edition = "2024"`, `cargo clippy --all-targets -- -D warnings` | Clean. `cargo fmt --check` shows 4 diff hunks. `rng.gen_range` is not hit by the `gen` keyword reservation. **Holds.** |
| clap 4.1.1: `args_conflicts_with_subcommands` with an `Option<Command>` and a flattened `TuiArgs` | Scratch, the plan's three `try_parse_from` cases | All three behave as the plan expects. `-t 5m remind 1m` gives `UnknownArgument`. **Holds.** |
| A bad duration on `remind ... --json` gives `{"error":...}` and exit 1 | Scratch: `remind 1.5h --json` through a `value_parser`; clap 4.1.1 `error/mod.rs:214-219`, `util/mod.rs:27` | clap returns `ValueValidation` with `use_stderr() == true`. It prints **plain text to stderr, nothing on stdout, exit 2** (`USAGE_CODE = 2`). **Fails** the plan's exit table and the spec's JSON-error contract. |
| `99999999999h` never panics (Review Focus 2) | Scratch: parse to `std::time::Duration`, `chrono::Duration::from_std`, then `Local::now() + d` | `from_std` succeeds. The addition **panics**: "`DateTime + Duration` overflowed" (chrono 0.4.23 `src/datetime/mod.rs:911`, `.expect`). The threshold is between 2.0e9 h (fine) and 2.4e9 h (panics). |
| `#[derive(Serialize)]` on `Event::Set { fires_at: DateTime<Local> }` | chrono 0.4.23 `Cargo.toml` (`serde` optional), `src/datetime/mod.rs:45-46` | `DateTime`'s `Serialize` impl only exists with chrono's `serde` feature. The repo's `chrono = "0.4"` doesn't enable it, and the plan only adds `serde` and `serde_json`. **Does not compile as written.** It also serializes through `Debug`, giving nanosecond fractions (`...T14:32:00.123456789+01:00`), not the spec's example format. |
| rodio 0.16 `SineWave` matches `SAMPLE_RATE = 44_100` | rodio 0.16.0 `src/source/sine.rs` | `SineWave` is **fixed at 48 kHz**, mono, `f32`. The test's `/ SAMPLE_RATE` is wrong by 8.8 %. The test's windows (1–4 s, 0.2–1.5 s) are wide enough that it still passes, so it can't catch the mismatch. |
| rodio 0.16 has the other APIs the plan names | `src/source/mod.rs`, `src/sink.rs` | `take_duration`, `repeat_infinite`, `amplify`, `fade_in`, `convert_samples`, `from_iter`, `Zero`, `Sink::sleep_until_end`, and `Source` for `Box<dyn Source<Item = S> + Send>` (line 366) all exist. **Holds.** |
| The plan's test code passes `cargo clippy --all-targets -- -D warnings` | Scratch: the Task 3 line `let secs = \|n: usize\| n as f32 / (SAMPLE_RATE as f32);` | **2 errors**, both `cast_precision_loss` (pedantic): `usize`→`f32` and `u32`→`f32`. The plan's own gate rejects its verbatim test. |
| notify-rust `show()` reports failure (Review Focus 4, exit code 3) | notify-rust 4.18.2 `src/macos/nsusernotifications.rs`, `src/windows.rs`, `src/xdg/mod.rs:414` | **Linux**: sends synchronously over zbus and returns errors. **Windows**: `toast.show()` errors propagate. **macOS**: `show_notification` only wraps the notification in a handle and returns `Ok`. The send happens in `Drop`, asynchronously, with `.ok()` swallowing the error. The crate's own doc says this backend (`NSUserNotificationCenter`) "is deprecated on macOS 14+, but still works". |
| notify-rust MSRV against `rust-version` | notify-rust 4.18.2 `Cargo.toml` | `rust-version = "1.89.0"`. The plan sets the crate's `rust-version` to today's stable (1.97). |
| macOS audio deps have been built before | `Cargo.lock`; `build.yml` history | The lock pins `coreaudio-sys 0.2.11`, `bindgen 0.61.0`, `clang-sys 1.4.0` (2022-era). `build.yml` has only ever cross-built `x86_64-pc-windows-gnu`, so this macOS stack has never been compiled for this repo. |
| "rodio 0.16 (kept; upgrade parked)" | `docs/parking-lot.md` | 4 entries: MCP, restart/absolute times, ratatui, detached reminders. **0 mention rodio.** |
| "no `unwrap`/`expect`" is enforced by the clippy gate | `Cargo.toml` `[lints.clippy]`; grep of `src/` | `unwrap_used`/`expect_used` are not enabled. `src/` has 0 `unwrap()`/`expect(` today, so the rule holds by accident. |

**Not verifiable here** (not in the registry, or needs another OS or the network): the sources of
`mac-notification-sys`, `tauri-winrt-notification` and `zbus`; a macOS or Windows build; the GitHub
runner images; Claude Code's plugin-cache behaviour. Each claim below that rests on these says so.

---

## Objection 1 (a): the agent-facing contract breaks on bad and huge durations, and the plan's tests can't see it

**Claim.** The agent relies on one thing from this CLI: with `--json`, every outcome is a JSON line,
plus an exit code it can act on. The plan states that contract twice. "Errors: `{"error":"..."}` and a
non-zero exit" (Global Constraints) and "`1` error before anything fired (**bad duration**, unreadable
sound file)". Task 5 then wires the duration through clap's `value_parser`. A bad duration therefore
never reaches `run` and `write_error`. Clap prints a text error to stderr and exits **2**, with
nothing on stdout, even when `--json` is given. I verified this with clap 4.1.1 in a scratch build.
The plan's e2e test for this case asserts only "non-zero exit, and stderr names the accepted forms",
which is exactly the loose assertion that passes. Unit tests on `parse` pass too.

**The overflow case is worse.** Review Focus 2 promises that `99999999999h` gives "never a panic or
overflow. Test in Task 2." Task 2 swaps that input for `99999999999999999999h`, which overflows `u64`
inside the parser. `99999999999h` is 3.6e14 s: it fits in `u64`, so `parse` accepts it, and
`chrono::Duration::from_std` accepts it too. Then `Local::now() + d` panics: chrono 0.4.23
`datetime/mod.rs:911` is `.expect("DateTime + Duration overflowed")`. I reproduced this; it panics for
any duration above about 2.3e9 h. Both `remind` (computing `fires_at`) and the human TUI
(`App::new`, `src/app.rs:125`, which `-t` now feeds) hit it. The outcome is exit 101 with a Rust panic
message, not JSON. The plan's tests can't catch it, because they only test `parse`.

**Why it matters.** An agent that gets 2 or 101 with an empty stdout has nothing to parse. The skill
(Task 7) is written against "the exit codes, the `set`, `fired` and `dinged` lines" as built. It will
document exit 1 for bad durations, which never happens.

**Instead.**
1. Parse the duration inside `remind::run` and `ding::run`, not in clap. Make the positional a
   `String` and call `duration::parse` first. Or keep the `value_parser`, but have `main` catch
   `clap::Error` when `--json` appears in `argv`, and emit `{"error":...}` with exit 1.
2. Make `parse` reject anything with `Local::now().checked_add_signed(...) == None`. Better, cap
   durations at something sane (for example 1 year) with a `TooLong` variant. Add `99999999999h` to the
   Task 2 tests, and an e2e test `remind 99999999999h --json` asserting exit 1 and one JSON error line.
3. Tighten the Task 5 e2e test to `exit == 1` and "stdout is one valid `{"error":...}` line".

**What would refute this:** none of it, unless the plan accepts exit 2 with text for bad input. If so,
the exit-code table and the spec's "Errors are `{"error":"..."}`" must say that explicitly.

## Objection 2 (b): on macOS, notify-rust's `show()` cannot fail, and no step ever shows a real notification off Windows

**Claim.** The spec calls the notification "the thing relied on" and requires that "Windows, macOS
and Linux all work". The plan assumes `Notification::new().summary(..).body(..).show()` returns an
error when the notification fails, and builds Review Focus 4 and exit code `3` on that. On macOS,
in notify-rust 4.18.2, that assumption is false:

- `macos::show_notification` returns `Ok(NotificationHandle::new(notification.clone()))` and sends
  nothing (`src/macos/nsusernotifications.rs`, near the end of the file).
- The send happens in `impl Drop for NotificationHandle`, with `asynchronous: true`, and the result is
  discarded with `.ok()`.
- The backend is `NSUserNotificationCenter`, which the crate's own doc comment says is "deprecated on
  macOS 14+". The modern path (`preview-macos-un`) is opt-in and needs app-bundle checks
  (`check_bundle`, `request_auth`), which an unbundled `cargo install` binary is unlikely to pass.
  This is inferred from the API names, not run.

So on macOS a notification that never appears produces exit 0, a `fired` line, and an agent telling
the user "your reminder fired". That is the spec's worst case. Whether the asynchronous send even
completes before `ding` exits, about 0.5 s later, depends on `mac-notification-sys`, which I couldn't
read (not in the registry). It is unverified, and it is the kind of thing that only shows up on a
real Mac.

**Nothing in the plan would notice.** Count the cells: 3 OSes × {quiet, system} alerter = 6. CI runs
the 3 quiet cells (`CLI_TIMER_QUIET=1` at job level). Task 9 runs the Windows system cell by hand. The
macOS and Linux system cells are **never executed** by any task. "Confirm the `check` workflow still
builds on all three OSes" (Task 4 Step 4) proves compilation only. The macOS audio stack has never
been built for this repo either. `Cargo.lock` pins `bindgen 0.61.0` / `coreaudio-sys 0.2.11` from
2022 against a current Xcode. Task 1 will at least surface a build failure, but the plan has no
contingency for one: `cargo update -p coreaudio-sys`, or bringing the parked rodio upgrade forward.

**Instead.**
1. On macOS, hold the `NotificationHandle` until the ring or chime ends instead of dropping it, so
   the send happens while the process is alive. Or call `set_application` with a known bundle id. In
   the `Alerter` docs and the README, say that macOS notification failure is undetectable, so exit 3
   only means "sound failed" there.
2. Add an acceptance step, a one-off but written down, to run `remind 5s` and `ding` on a real Mac and
   on a Linux desktop. If nobody can, the spec's Goal should say "macOS and Linux: built and tested
   quietly in CI; delivery unverified", not "all work".
3. Add a `#[ignore]` integration test that uses `SystemAlerter`, so `cargo test -- --ignored` on a
   desktop is a two-second check.

**What would refute this:** a run on a Mac showing that the toast appears for both `remind` and `ding`
from a `cargo install`ed binary, and that a denied notification permission is reported somewhere.

## Objection 3 (c): in 6 months the binary, the plugin and the release workflow have drifted apart, and nothing checks any of it

**Claim.** The plan ships one contract (flags, JSON events, exit codes) through three channels that
version independently. Nothing ties them together:

- **The plugin is versioned by hand.** `plugin.json` copies the `Cargo.toml` version (Task 8 Step 4).
  The release job checks the tag only against `Cargo.toml`. Claude Code re-fetches a plugin only when
  its version changes (medium confidence, from memory of the plugin docs). So a skill fix without a
  manual `plugin.json` bump never reaches installed users. A bump without a release leaves the skill
  calling flags the user's binary doesn't have.
- **The skill checks presence, not compatibility.** Task 7 has it run `cli-timer --version` only to
  see whether the binary exists. When 0.3 adds or renames a flag, a user with a 0.2 binary and the 0.3
  skill gets clap exit 2. The skill then misreads that as an error the user caused (see Objection 1).
- **`cargo install --git` (Task 8 README) ignores `Cargo.lock` without `--locked`.** Users get a
  freshly resolved dependency set that CI never built. Edition 2024's resolver v3 limits that set to
  versions compatible with `rust-version`, and the plan pins `rust-version` to today's 1.97. That locks
  out anyone whose toolchain is older than this month, but does nothing to stop new crate versions.
- **`release.yml` first runs on the real `v0.2.0` tag** ("tagging … is the user's call"). As
  specified it has concrete gaps. It builds `x86_64-apple-darwin` on `macos-latest`, an arm64 image
  (from memory, high confidence), without saying to install that target with `rustup target add`. It
  builds the Linux binary on `ubuntu-latest`, which links against that image's glibc (2.39 on 24.04,
  from memory), so the "for everyone else" download won't start on Debian 12 or Ubuntu 22.04. It names
  no release-creation step and no `permissions: contents: write`, yet four matrix jobs each "upload to
  the GitHub release".

**How it fails.** Quietly. The plugin auto-updates, the binary doesn't, and the first sign is an
agent saying "I couldn't set that reminder" or, worse, an exit code it misreads.

**Instead.**
1. Give the CLI a machine-readable contract version: `cli-timer --version --json`, or a
   `contract: 1` field in every JSON line. Have the skill check it against the minimum it was
   written for, and print the upgrade route on a mismatch.
2. Have CI fail when `plugin.json`'s version differs from `Cargo.toml`'s. That is one `jq` and one
   `grep` in `check.yml`.
3. Document `cargo install --locked --git ...`. Set `rust-version` to the real MSRV, which is at
   least 1.89 for notify-rust and 1.85 for edition 2024, not to today's stable.
4. Give `release.yml` a `workflow_dispatch` dry run that builds and packages without publishing, and
   run it before tagging. Build Linux on the oldest supported Ubuntu image, or target musl. Add the
   macOS x86_64 target explicitly. Give one job the job of creating the release, with
   `contents: write`.

**What would refute this:** a decision that the plugin and the binary are only ever installed
together by the one user from one checkout. The spec's "prebuilt release binaries … for everyone
else" says otherwise.

---

## Fixed check: sentences that set two verified facts side by side

For each sentence that pairs a count with an absence, or a total with a remainder, this table gives
the inference a reader will draw and whether the facts establish it. Each residual is broken down
until its parts add up.

| # | Sentence (where) | Fact 1 / Fact 2 (verified?) | Inferred third claim | Supported? Decomposition |
|---|---|---|---|---|
| F1 | Global Constraints: "Exit codes … `0` success; `1` error before anything fired (bad duration, unreadable sound file); `3` fired, but …; clap's own usage errors keep clap's exit code `2`." | 4 codes listed (yes) / clap errors are `2` (yes: `USAGE_CODE = 2`) | Every outcome is one of these four, and a bad duration is `1`. | **Unsupported implicature.** The real outcomes of `remind` are 0 (ok) + 1 (sound check fails; the only pre-fire error that reaches `run`) + 2 (**every malformed duration**, plus missing or unknown arguments, via `value_parser`) + 3 (partial) + **101** (panic for durations above about 2.3e9 h). That is 5 codes, not 4. "Bad duration" belongs to 2 when malformed and 101 when huge, never to 1. This is Objection 1. |
| F2 | Review Focus 2: seven malformed inputs "… give a clear error … never a panic or overflow. Test in Task 2." | 7 inputs named (yes) / Task 2 has a `rejects_malformed` and a `rejects_overflow` test (yes) | All 7 are tested in Task 2. | **Unsupported implicature.** 7 = 6 tested verbatim (`""`, `abc`, `1.5h`, `30m1h`, `1h1h`, `-5m`) + 1 swapped (`99999999999h` → `99999999999999999999h`). The swapped-in input tests `u64` overflow in the parser. The original input gets through the parser and panics later in chrono (verified). Task 2 also tests 3 extras (`5x`, `1h 30m`, `::`), so it has 9 malformed inputs, not 7. |
| F3 | Review Focus 4: "the other channel is still delivered, the error goes to stderr, and the exit code is `3`. Test in Task 5." | 4 sub-claims (the either-direction delivery counts as 2) / Task 5 has a `fire` unit test (yes) | All four are tested in Task 5. | **Unsupported implicature.** 4 = 1 tested (`play` fails → `notify` still called → `Partial`) + 3 untested (`notify` fails → sound still plays; error on stderr; process exit 3). On macOS the "notify fails" branch can't happen at all, because `show()` always returns `Ok` (Objection 2). |
| F4 | Tech Stack: "rodio 0.16 (kept; upgrade parked)" | rodio is 0.16 (yes, `Cargo.toml`) / "parked" | The upgrade is recorded in `docs/parking-lot.md`, as `CLAUDE.md` requires for anything "deferred or later". | **False.** The parking lot has 4 entries = MCP + restarts/absolute times + ratatui + detached reminders; rodio appears in 0. This breaks the repo's working rule, and it matters because Objection 2's macOS build may force the upgrade. |
| F5 | Global Constraints: "`cargo clippy --all-targets -- -D warnings` clean under the `[lints.clippy]` policy; no `unwrap`/`expect` outside tests" | Clippy clean (yes, at HEAD and in scratch with edition 2024) / 0 `unwrap()`/`expect(` in `src/` (yes) | The clippy gate enforces the no-unwrap rule. | **Unsupported implicature** (already raised against the spec twice; still unfixed). `unwrap_used`/`expect_used` are restriction lints and aren't enabled. 0 = 0 by accident. Enabling them also needs `allow-unwrap-in-tests` in a `clippy.toml`, because the plan's tests use `.unwrap()` throughout. |
| F6 | Task 9 Step 2: "If 2 or 3 leave `cli-timer.exe` running, …" | Checklist has 6 items (yes) / remedies exist for 2 of them (yes) | The other 4 items either can't fail or need no fix. | **Unsupported implicature.** 6 = 2 with a remedy (2, 3: parent-pid polling) + 1 with a wording remedy (5, Step 3) + 1 regression check (6) + **2 with no remedy** (1: toast, sound, **no console window**; 4: chime, toast, no prompt). A console window (spec Constraint) has no planned fix, and the obvious one, `windows_subsystem = "windows"`, breaks the human TUI, an anti-goal. Separately, the remedy for 2 and 3 needs the parent pid, which `OpenProcess` can't give you (it needs `CreateToolhelp32Snapshot` or `NtQueryInformationProcess`). It also needs a `windows-sys` dependency the plan never adds, and it has a pid-reuse race unless the handle is opened at startup. |
| F7 | Spec Goals: "Windows, macOS and Linux all work and are tested in CI in this round." (The plan inherits it.) | CI matrix covers 3 OSes (plan, yes) / tests run with `CLI_TIMER_QUIET=1` (plan, yes) | CI tests that alerts work on all three. | **Unsupported implicature.** 6 cells = 3 quiet (CI) + 1 system (Windows, manual) + **2 never executed** (macOS system, Linux system). Objection 2. |
| F8 | Task 3 Step 3: "Mono at `SAMPLE_RATE = 44_100` … Build both from `rodio::source::SineWave`" | A sample rate stated / the source type stated | The built-in sounds run at 44.1 kHz, and the length test measures seconds. | **False.** `SineWave` is fixed at 48 kHz (`sine.rs`). The test divides by the wrong rate, and its wide windows hide the 8.8 % error. Its cast also fails clippy (2 × `cast_precision_loss`, verified). |

Checked and **supported**: `NEXT.md`'s "9 tasks" against its 9-item list; the Architecture line's 7
new modules against the Task 2–6 file lists (duration, sound, alert, cli, output, remind, ding = 7);
"existing TUI modules (`app`, `event`, `handler`, `tui`) stay" against `src/lib.rs` (4 modules);
Review Focus 1 and 3 against Task 5's tests; and Review Focus 5's 1 s slicing (worst case is about
1 s late after wake).

**Does a flagged implicature displace an objection?** No. F1, F2 and F3 are the evidence behind
Objections 1 and 2, and F7 is Objection 2. The two that stand alone are F4 and F6. F4 is a
bookkeeping breach with a one-line fix. F6 (no remedy for a console window) is real, but probably
unlikely: a child of Git Bash normally inherits a hidden console. Both are weaker than Objection 3,
which is near-certain to bite the first time the CLI surface changes.

## Runners-up (considered, not in the top three)

- **chrono `serde` feature missing** (verified). The plan's `Event` won't compile until
  `chrono = { version = "0.4", features = ["serde"] }` is set. Easy to fix, but the timestamp format
  then has nanoseconds. Pin it with `#[serde(serialize_with = ...)]` and `to_rfc3339_opts(Secs, false)`
  if the agent or tests compare it.
- **The verbatim test code fails the gate** (verified: `cast_precision_loss` ×2). Expect more pedantic
  and nursery hits on the plan's interfaces: `must_use_candidate` on `parse`/`ring_for`/`run`, and
  `needless_pass_by_value` on `run(args: RemindArgs, …)`. `Playback` needs `Debug` and `PartialEq`
  for the `quiet play:` line and the Task 6 equality test, but the interface declares no derives.
- **The spec says `log` over `eprintln!`; the plan writes straight to stderr.** The `QuietAlerter`
  stderr lines and the "error on stderr" are written directly, `log` is never added, and the 4
  existing `eprintln!` stay. If someone later moves these to `log` with no logger set, the e2e
  assertions on stderr break.
- **The spec's Structure says `main.rs` only parses and dispatches**, but the plan keeps the TUI loop
  in `main.rs` ("no subcommand runs the existing TUI loop unchanged").
- **`ding` "a second or two at most" next to a 5 s cut for `-s` files.** That is the plan's own call,
  but the skill should not promise two seconds when a sound file is given.
- **`check()` then `source()` opens the file twice**, so a sound file deleted or moved during a two-hour
  wait fails at fire time. That is accepted by exit 3, but worth one line in the skill.
- **The CI triggers `push` (all branches) plus `pull_request`**, which runs every PR twice across
  3 OSes.

## Method note

All "verified" rows were run or read in this session. The scratch build was a copy of the repo with
the locked dependencies, offline, and its probe code was thrown away. Claims marked "from memory"
(runner images, Claude Code plugin caching) were not checked against current documentation, because
the docs MCP server failed to connect.
