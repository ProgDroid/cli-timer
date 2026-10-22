# NEXT

## Where we are
Agent-friendly timers are on `main` (v0.2.0, squash-merged 2026-10-22): `cli-timer remind
<duration>` (a reminder an agent runs in the background: pop-up + alarm for 30 s), `cli-timer ding
[message]` (a one-off "done" chime + pop-up), built-in sounds, short durations (`20m`, `1h30m`),
`--json` output, and a Claude Code plugin with a `timer` skill. The full-screen timer works as
before, with `-s` now optional. Every task was reviewed, then a whole-branch review and one fix
pass (157 tests passing). Merged at your request before CI or the checklist below ran, so
`main` is unverified on Windows and macOS until both are done (see Housekeeping).

## How to run
- Install from your checkout: `cargo install --locked --force --path .` (Rust 1.91 or later).
- `cli-timer remind 1m -l "stretch"` / `cli-timer ding "done"` / `cli-timer -t 00:00:10`.
- Tests: `cargo test` (157: 127 unit + 30 end-to-end; they never pop notifications or play sound).

## How it behaves
`remind` prints a "set" line at once, waits, then shows a pop-up titled cli-timer with your label
(or "Timer finished") and loops the alarm for 30 s (`--ring` up to 10 m), prints "fired" and exits.
`ding` chimes once and shows a pop-up ("Done" by default). With `--json`, one JSON object per line.
Exit codes: 0 ok, 1 failed before anything fired, 3 fired but the sound or pop-up failed.

## Try this (the acceptance checklist; CI can't hear sounds or see pop-ups)
Install the plugin from your checkout in Claude Code: `/plugin marketplace add <path to repo>`,
then `/plugin install cli-timer@cli-timer`. Add the allow rule `Bash(cli-timer:*)`.
1. "remind me in 1 minute to stretch": Claude confirms; a minute later a toast says "stretch" and
   the alarm loops about 30 s, then stops. No console window appears.
2. "remind me in 5 minutes", then "cancel that reminder": it never fires, and Task Manager shows no
   `cli-timer.exe` left running.
3. "remind me in 1 minute", let it ring, then stop the task while it rings: the sound stops and no
   `cli-timer.exe` remains.
4. "ding me when you've listed this folder": chime + toast, no permission prompt.
5. Note whether Claude told you the reminder fired without being asked.
6. `cli-timer -t 00:00:05` in a terminal behaves as before, with the built-in alarm; try `-s` with
   one of your own mp3/wav/flac/ogg files too (the decoder changed to symphonia).
7. After the workflows are active and merged: run the `release` workflow by hand and check all
   four archives appear, before pushing the `v0.2.0` tag.

## Things you might notice
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).
- A bad `-s` file in the full-screen timer only errors when the timer fires (as before).

## Tell me if
- Item 2 or 3 leaves `cli-timer.exe` running: then `remind` needs to exit when its parent dies
  (plan Task 9 Step 2).
- Item 5: whether Claude was told the reminder fired; if not, the skill shouldn't imply it.
- Anything in the skill's behaviour that felt wrong (wording, when it triggers).

## Housekeeping
- **Workflows are staged, not active.** This session couldn't push `.github/workflows/` (no
  `workflow` permission). Follow `ci/workflows/README.md`: move `check.yml` and `release.yml`,
  `git rm .github/workflows/build.yml`, delete `ci/workflows/`, update CLAUDE.md's CI line. Do it on
  a short branch with a PR so `check.yml` runs before it lands, and fix anything red on Windows or
  macOS before tagging v0.2.0: the macOS code has never been compiled and no tests have run on
  Windows or macOS.
- Project-start ritual still open here: Beads, secret scan, lessons search, repo visibility.

## Known rough edges
See `docs/parking-lot.md` (2026-10-22 entries): no date in the "set" line for reminders over a day;
Git Bash rewrites a label starting with `/`; x86_64-only Windows/Linux binaries; rodio 0.16.

## Feedback log
_Your answers to the session-start question, dated, newest first._
