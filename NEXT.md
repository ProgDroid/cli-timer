# NEXT

## Where we are
**v0.2.2 is merged into `main`, not released yet** (2026-10-27). It is the dependency refresh:
every direct dependency is current, and cli-timer should behave as 0.2.1 did, with one deliberate
exception (below).
- The full-screen UI moved from `tui` 0.19 (unmaintained) to `ratatui` 0.30.2, with crossterm 0.29
  coming through ratatui. The time sits on the same row as before.
- Sound moved from rodio 0.16 to 0.23. The alarm, chime, fade-in and ring times are unchanged.
  rodio 0.23 left the extra speakers of a 5.1/7.1 output silent, so cli-timer now fills them
  itself, as 0.2.1 did (not heard by anyone yet: no surround output here).
- Keys now act only when pressed. On Windows the new crossterm also reports key releases, which
  would have made `r` restart without asking, Space pause and resume at once, and Esc quit.
- `rand` 0.10, `clap` 4.6.7, `chrono` 0.4.45.
- **The deliberate change: `-s` now takes mp3, flac, ogg vorbis and wav only.** MKV/WebM audio and
  ADPCM wav played in 0.2.1 and are now rejected at start with
  `cannot decode sound file <path>: Unrecognized format`.

New tests pinned the old behaviour before each upgrade: keys, the full-screen layout, and real
mp3/flac/ogg files (`tests/fixtures/`, made with ffmpeg; the commands are in its README). 196
tests pass on Windows; CI passed on Linux, Windows and macOS. Design, plan and reviews:
`docs/superpowers/specs/2026-10-27-dependency-refresh-design.md`, `docs/superpowers/plans/`,
`docs/superpowers/reviews/2026-10-27-dependency-refresh-*`.

**Still to do: your hands-on check (below), then the release** (dry run, four archives, tag
`v0.2.2`). Tracked as the last open task of the v0.2.2 epic in `bd`.

## How to run
- Install: `cargo install --locked --force --git https://github.com/ProgDroid/cli-timer`, or a
  release archive, or `cargo install --locked --force --path .` from this checkout.
- `cli-timer remind 1m -l "stretch"` / `cli-timer ding "done"` / `cli-timer -t 00:00:10`.
- In Claude Code (plugin installed): "remind me in 20 minutes to stretch", "cancel that reminder",
  "ding me when you're done".
- Tests: `cargo test` (196 on Windows: 163 unit, 33 end-to-end). They never pop notifications or
  play sound.
- Open work: `bd ready`.

## How it behaves
`remind` prints a "set" line at once, waits, then shows a pop-up titled cli-timer with your label
(or "Timer finished") and loops the alarm for 30 s (`--ring` up to 10 m), prints "fired" and exits.
`ding` chimes once and shows a pop-up ("Done" by default). With `--json`, one JSON object per line.
Exit codes: 0 ok, 1 failed before anything fired, 3 fired but the sound or pop-up failed.
Cancelling a reminder in Claude Code kills the process, which also stops a ringing alarm.

## Try this
Install 0.2.2 first: `cargo install --locked --force --path .`, then on Windows:
- `cli-timer -t 20s`: the full screen looks as before. Press Space (pauses and stays paused), Space
  (resumes), `r` (asks to confirm), Esc (back to running, does not quit), `r` `r` (restarts), let it
  ring, then `q` (quits).
- `cli-timer -t 5s -s <some.mp3>`: your own mp3 rings.
- `cli-timer remind 5s -l test`: pop-up "test", 30 s alarm, nothing extra printed.
- `cli-timer ding "done"`: chime and pop-up.
- `cli-timer remind 5s --json`: only JSON lines, nothing else.

## Things you might notice
- `cli-timer --help` lines up one space narrower, and one usage error is worded differently
  (exit codes unchanged).
- When there is no audio device at all, the error text now comes from rodio 0.23 and no longer
  names the low-level cause.
- Your install points at this folder, so Claude Code reads the skill straight from
  `plugin/skills/timer/` at every session start (a resumed session keeps the old copy; restart
  fully to pick up a skill edit).
- The plugin validator warns that the plugin has no icon (only matters for Anthropic's directory).
- A sound file deleted after the timer started still fails silently when the timer ends.
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).

## Tell me if
- Any key acts twice, or on release, or the screen looks different from 0.2.1.
- The alarm, chime or an `-s` file sounds different, or plays from fewer speakers.
- `--json` ever prints anything that is not a JSON line.
- An mp3, flac, ogg or wav that played before is now rejected.
- A reminder fires late, twice, or not at all, or a `cli-timer.exe` lingers after one ends.

## Housekeeping
- **Bumping Rust is a deliberate change**: edit `channel` in `rust-toolchain.toml`, run
  `cargo clippy --all-targets -- -D warnings` and `cargo test`, fix any new lints in the same
  change.
- No PRs: work on a branch, squash-merge into `main` locally with a `handoff:` subject, push
  `main`.
- Next release: bump `version` in both `Cargo.toml` and `plugin/.claude-plugin/plugin.json` (a
  test fails if they differ), do a manual release run (dry run) and check four archives appear,
  then push the `vX.Y.Z` tag (lightweight). For v0.2.2 the bump is done; the dry run and tag are
  left. The release notes should mention the `-s` format change.

## Known rough edges
See `docs/parking-lot.md`: rodio 0.23 is an upstream stopgap before a bigger release (expect
another pass on the sound code); a rare mid-frame cut is possible on surround outputs with unusual
files; Git Bash rewrites a label starting with `/`; x86_64-only Windows/Linux binaries; the plugin
version is tied to the crate version. macOS and Linux sound and pop-ups have not been checked by a
person on 0.2.2.

## Feedback log
_Your answers to the session-start question, dated, newest first._
- 2026-10-27: "Not tried yet"; walked through the 0.2.1 checks (install, missing `-s` file,
  non-audio `-s` file, the day in the "set" line): "all working". Then asked to release; v0.2.1
  published. Picked the dependency refresh next, everything current, rodio 0.23 despite its
  stopgap status, `-s` narrowed to the four named formats, subagent-driven execution.
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
