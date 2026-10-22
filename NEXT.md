# NEXT

## Where we are
**v0.2.0 is released** (2026-10-22): https://github.com/ProgDroid/cli-timer/releases/tag/v0.2.0,
with Windows, Linux and two macOS archives, each with a `.sha256`. The release run was green
(tag matched `Cargo.toml`), and the Windows archive was downloaded, its checksum matched, and its
binary reports 0.2.0. You ran the hands-on checklist with Claude this session and all six checks
passed (results in the feedback log). The old 0.1.0 copy in `dotfiles\rust-apps\cli-timer` was
deleted; `cli-timer` now resolves only to `~/.cargo/bin` (0.2.0). The plugin is installed from
this checkout and the `Bash(cli-timer:*)` allow rule is set.

Nothing is in progress. The one open item is a bug found while installing the plugin (below).

## How to run
- Install: `cargo install --locked --force --git https://github.com/ProgDroid/cli-timer`, or a
  release archive, or `cargo install --locked --force --path .` from this checkout.
- `cli-timer remind 1m -l "stretch"` / `cli-timer ding "done"` / `cli-timer -t 00:00:10`.
- In Claude Code (plugin installed): "remind me in 20 minutes to stretch", "cancel that reminder",
  "ding me when you're done".
- Tests: `cargo test` (157 on Linux/macOS, 156 on Windows). They never pop notifications or play
  sound.
- Open work: `bd ready`.

## How it behaves
`remind` prints a "set" line at once, waits, then shows a pop-up titled cli-timer with your label
(or "Timer finished") and loops the alarm for 30 s (`--ring` up to 10 m), prints "fired" and exits.
`ding` chimes once and shows a pop-up ("Done" by default). With `--json`, one JSON object per line.
Exit codes: 0 ok, 1 failed before anything fired, 3 fired but the sound or pop-up failed.
Cancelling a reminder in Claude Code kills the process, which also stops a ringing alarm.

## Try this
- Nothing new to try. Use it day to day; dotman's next run installs 0.2.0 from GitHub.

## Things you might notice
- **The plugin cache holds a 1.7 GB copy of this repo's `target/` folder**
  (`~/.claude/plugins/cache/cli-timer/cli-timer/0.2.0/target`, which lives under your dotfiles
  folder; it's git-ignored there). Installing the plugin from a local folder copies the whole repo.
  Safe to delete that `target` folder by hand; the plugin doesn't use it. Fix tracked as
  `clitimer-b1j`.
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).
- A reminder with no label prints `"label":null` in JSON and the pop-up says "Timer finished".

## Tell me if
- A reminder ever fires late, twice, or not at all, or a `cli-timer.exe` lingers after one ends.
- Claude reaches for its own scheduling instead of `cli-timer` when you ask to be reminded.
- Whether `clitimer-b1j` (move the plugin into a subfolder so a local install skips `target/`)
  is worth doing next, or something from the parking lot instead.

## Housekeeping
- **Bumping Rust is a deliberate change**: edit `channel` in `rust-toolchain.toml`, run
  `cargo clippy --all-targets -- -D warnings` and `cargo test`, fix any new lints in the same
  change.
- The repo only allows squash merges, so a merged PR's subject should start `handoff:`.
- Next release: bump `version` in `Cargo.toml` and `.claude-plugin/plugin.json`, do a manual
  release run (dry run) and check four archives appear, then push the `vX.Y.Z` tag.

## Known rough edges
See `docs/parking-lot.md`: no date in the "set" line for reminders over a day; Git Bash rewrites a
label starting with `/`; x86_64-only Windows/Linux binaries; rodio 0.16; `tui` unmaintained.

## Feedback log
_Your answers to the session-start question, dated, newest first._
- 2026-10-22 (second session): "Not tried yet"; then ran the checklist with Claude, all passed:
  (1) toast said "stretch", ~30 s alarm, no console window; (2) cancel left no `cli-timer.exe`;
  (3) stopping while ringing silenced it at once, no process left; (4) `ding` gave chime + toast
  with the message, no permission prompt; (5) Claude reported the reminder firing unprompted;
  (6) full-screen timer works with the built-in alarm and with your own `-s` file. v0.2.0 tagged.
- 2026-10-22: "Not tried yet" (the v0.2.0 hands-on checklist hasn't been run).
