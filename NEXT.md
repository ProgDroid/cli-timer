# NEXT

## Where we are
**v0.2.1 is released** (2026-10-27): https://github.com/ProgDroid/cli-timer/releases/tag/v0.2.1,
with archives for Windows, macOS (Apple Silicon and Intel) and Linux, each with a `.sha256` file.
A dry run first showed all four archives; the tag then published them. You installed 0.2.1 from
this checkout and checked its three fixes by hand, and all of them worked:
- The full-screen timer checks the `-s` sound file before it opens its screen, so a missing or
  non-audio file gives an error at once instead of a silent alarm at the end.
- A reminder's "set" line shows the day when the reminder is not today (`at Sat 24 Oct 01:00:00`),
  and the year too when it falls in another year. A same-day reminder still shows just the time.
- `Cargo.toml` and the plugin manifest say `GPL-3.0-or-later`.

There is no open work in `bd`. The next piece of work has not been picked.

## How to run
- Install: `cargo install --locked --force --git https://github.com/ProgDroid/cli-timer`, or a
  release archive, or `cargo install --locked --force --path .` from this checkout.
- `cli-timer remind 1m -l "stretch"` / `cli-timer ding "done"` / `cli-timer -t 00:00:10`.
- In Claude Code (plugin installed): "remind me in 20 minutes to stretch", "cancel that reminder",
  "ding me when you're done".
- Tests: `cargo test` (163 on Windows; Linux and macOS run one more). They never pop notifications
  or play sound.
- Open work: `bd ready`.

## How it behaves
`remind` prints a "set" line at once, waits, then shows a pop-up titled cli-timer with your label
(or "Timer finished") and loops the alarm for 30 s (`--ring` up to 10 m), prints "fired" and exits.
`ding` chimes once and shows a pop-up ("Done" by default). With `--json`, one JSON object per line.
Exit codes: 0 ok, 1 failed before anything fired, 3 fired but the sound or pop-up failed.
Cancelling a reminder in Claude Code kills the process, which also stops a ringing alarm.

## Try this
- If you want, download one archive from the release page and run it, to confirm the published
  binary (not just your local build) says `0.2.1` with `cli-timer --version`.
- Look through `docs/parking-lot.md` and pick what comes next.

## Things you might notice
- Your install points at this folder, so Claude Code reads the skill straight from
  `plugin/skills/timer/` at every session start (a resumed session keeps the old copy; restart
  fully to pick up a skill edit).
- The plugin validator warns that the plugin has no icon. It only matters if you ever submit the
  plugin to Anthropic's directory.
- A sound file deleted after the timer started still fails silently when the timer ends (the check
  is at start only).
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).
- A reminder with no label prints `"label":null` in JSON and the pop-up says "Timer finished".

## Tell me if
- A release archive doesn't run on its platform, or its `.sha256` doesn't match.
- A reminder ever fires late, twice, or not at all, or a `cli-timer.exe` lingers after one ends.
- Claude reaches for its own scheduling instead of `cli-timer` when you ask to be reminded.
- What to do next from `docs/parking-lot.md`.

## Housekeeping
- **Bumping Rust is a deliberate change**: edit `channel` in `rust-toolchain.toml`, run
  `cargo clippy --all-targets -- -D warnings` and `cargo test`, fix any new lints in the same
  change.
- No PRs: work on a branch, squash-merge into `main` locally with a `handoff:` subject, push
  `main`.
- Next release: bump `version` in both `Cargo.toml` and `plugin/.claude-plugin/plugin.json` (a
  test fails if they differ), do a manual release run (dry run) and check four archives appear,
  then push the `vX.Y.Z` tag (lightweight, like the earlier ones).

## Known rough edges
See `docs/parking-lot.md`: Git Bash rewrites a label starting with `/`; x86_64-only Windows/Linux
binaries; rodio 0.16; `tui` unmaintained; the plugin version is tied to the crate version.

## Feedback log
_Your answers to the session-start question, dated, newest first._
- 2026-10-27: "Not tried yet"; walked through the 0.2.1 checks (install, missing `-s` file,
  non-audio `-s` file, the day in the "set" line): "all working". Then asked to release; v0.2.1
  published.
- 2026-10-26: asked to fix the review's small notes, then merge into `main` locally and push
  (v0.2.1 merged; the release itself was not asked for).
- 2026-10-23 (second session): "Not tried yet" (the plugin-subfolder install, since the restart).
  Picked the 0.2.1 polish batch next.
- 2026-10-23: asked to fix the plugin cache bug in the repo (approach: plugin in a subfolder); no
  PR, merge locally and push. Restart check passed.
- 2026-10-22 (second session): "Not tried yet"; then ran the checklist with Claude, all passed:
  (1) toast said "stretch", ~30 s alarm, no console window; (2) cancel left no `cli-timer.exe`;
  (3) stopping while ringing silenced it at once, no process left; (4) `ding` gave chime + toast
  with the message, no permission prompt; (5) Claude reported the reminder firing unprompted;
  (6) full-screen timer works with the built-in alarm and with your own `-s` file. v0.2.0 tagged.
- 2026-10-22: "Not tried yet" (the v0.2.0 hands-on checklist hasn't been run).
