# NEXT

## Where we are
**v0.2.0 is released** (2026-10-22): https://github.com/ProgDroid/cli-timer/releases/tag/v0.2.0,
with Windows, Linux and two macOS archives, each with a `.sha256`. The release run was green
(tag matched `Cargo.toml`), and the Windows archive was downloaded, its checksum matched, and its
binary reports 0.2.0. You ran the hands-on checklist with Claude this session and all six checks
passed (results in the feedback log). The old 0.1.0 copy in `dotfiles\rust-apps\cli-timer` was
deleted; `cli-timer` now resolves only to `~/.cargo/bin` (0.2.0). The plugin is installed from
this checkout and the `Bash(cli-timer:*)` allow rule is set.

**Plugin moved into `plugin/`** (2026-10-23, `clitimer-b1j`): installing it from this checkout had
copied the whole repo, `target/` included, into Claude Code's plugin cache (1.7 GB). The plugin now
lives in `plugin/` and the marketplace points at `./plugin`, so an install copies only its two
files: measured 9 KB, both from this checkout and from a GitHub clone. CI is green on all three
OSes. Your install was redone and the skill loads from `plugin/skills/timer` after a restart.

Nothing is in progress and `bd ready` is empty; pick the next thing from the parking lot.

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
- Your install points at this folder, so Claude Code reads the skill straight from
  `plugin/skills/timer/` at every session start (a resumed session keeps the old copy; restart
  fully to pick up a skill edit). Other people's 0.2.0 installs keep their old cached copy until
  the next version bump; harmless, the skill text is unchanged.
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).
- A reminder with no label prints `"label":null` in JSON and the pop-up says "Timer finished".

## Tell me if
- A reminder ever fires late, twice, or not at all, or a `cli-timer.exe` lingers after one ends.
- Claude reaches for its own scheduling instead of `cli-timer` when you ask to be reminded.
- What to do next from `docs/parking-lot.md`. Two new entries came out of this change: the
  plugin version is tied to the crate version (so a skill-only fix needs a release), and installed
  plugin copies carry no licence.

## Housekeeping
- **Bumping Rust is a deliberate change**: edit `channel` in `rust-toolchain.toml`, run
  `cargo clippy --all-targets -- -D warnings` and `cargo test`, fix any new lints in the same
  change.
- No PRs: work on a branch, squash-merge into `main` locally with a `handoff:` subject, push
  `main`.
- Next release: bump `version` in `Cargo.toml` and `plugin/.claude-plugin/plugin.json`, do a manual
  release run (dry run) and check four archives appear, then push the `vX.Y.Z` tag.

## Known rough edges
See `docs/parking-lot.md`: no date in the "set" line for reminders over a day; Git Bash rewrites a
label starting with `/`; x86_64-only Windows/Linux binaries; rodio 0.16; `tui` unmaintained.

## Feedback log
_Your answers to the session-start question, dated, newest first._
- 2026-10-23: asked to fix the plugin cache bug in the repo (approach: plugin in a subfolder); no
  PR, merge locally and push. Restart check passed.
- 2026-10-22 (second session): "Not tried yet"; then ran the checklist with Claude, all passed:
  (1) toast said "stretch", ~30 s alarm, no console window; (2) cancel left no `cli-timer.exe`;
  (3) stopping while ringing silenced it at once, no process left; (4) `ding` gave chime + toast
  with the message, no permission prompt; (5) Claude reported the reminder firing unprompted;
  (6) full-screen timer works with the built-in alarm and with your own `-s` file. v0.2.0 tagged.
- 2026-10-22: "Not tried yet" (the v0.2.0 hands-on checklist hasn't been run).
