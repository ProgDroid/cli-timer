# NEXT

## Where we are
v0.2.0 (agent-friendly timers: `remind`, `ding`, built-in sounds, `--json`, the Claude Code
plugin) is on `main`, and CI is now live and green. On 2026-10-22 the old Windows-only `build.yml`
(which failed on the v0.2.0 merge: its Rust was too old for edition 2024) was replaced by
`check.yml` (fmt, clippy, tests on Linux, Windows, macOS on every push and PR) and `release.yml`.
Getting macOS green took a dependency bump (`coreaudio-sys` 0.2.11 → 0.2.18, whose old bindgen
crashed on the current Xcode SDK); macOS now builds and passes all 157 tests. Rust is pinned to
1.99 in `rust-toolchain.toml`. The release dry run builds all four archives. The only thing left
before tagging `v0.2.0` is your hands-on check below.

## How to run
- Install from your checkout: `cargo install --locked --force --path .` (rustup fetches Rust 1.99
  the first time, because of the pin).
- `cli-timer remind 1m -l "stretch"` / `cli-timer ding "done"` / `cli-timer -t 00:00:10`.
- Tests: `cargo test` (157 on Linux/macOS, 156 on Windows: one test is Unix-only). They never pop
  notifications or play sound.
- Open work is in Beads now: `bd ready` shows what's next (`clitimer-cco`, the checklist below),
  `bd list` shows all of it.

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
7. When 1–6 pass: push the tag (`git tag v0.2.0 && git push origin v0.2.0`). `release.yml` checks
   the tag matches `Cargo.toml`, builds the four archives and publishes the GitHub release. Then
   close `clitimer-cco` and `clitimer-ap6`. dotman installs cli-timer from this repo, so the tag
   reaches that too.

## Things you might notice
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).
- A bad `-s` file in the full-screen timer only errors when the timer fires (as before).
- New in this repo: a Beads tracker (`.beads/`), a gitleaks secret scan that runs before every
  commit (it prints "no leaks found"), and a CLAUDE.md section on Beads vs. to-do lists.

## Tell me if
- Item 2 or 3 leaves `cli-timer.exe` running: then `remind` needs to exit when its parent dies
  (plan Task 9 Step 2).
- Item 5: whether Claude was told the reminder fired; if not, the skill shouldn't imply it.
- Anything in the skill's behaviour that felt wrong (wording, when it triggers).

## Housekeeping
- **Bumping Rust is now a deliberate change**: edit `channel` in `rust-toolchain.toml`, run
  `cargo clippy --all-targets -- -D warnings` and `cargo test`, fix any new lints in the same
  change. Rust 1.99 alone flagged 8 spots in code that passed before, so expect some on each bump.
- The repo only allows squash merges, so a merged PR's subject should start `handoff:`.
- The public-repo sweep found no secrets. Minor, no action needed: your personal email is the
  author address on every commit (consider a GitHub noreply address for future ones), commit
  76bf28b carries a Claude session link, and the agent-timer plan names your private plugins.
- Project-start ritual: done 2026-10-22, nothing left open.

## Known rough edges
See `docs/parking-lot.md` (2026-10-22 entries): no date in the "set" line for reminders over a day;
Git Bash rewrites a label starting with `/`; x86_64-only Windows/Linux binaries; rodio 0.16.

## Feedback log
_Your answers to the session-start question, dated, newest first._
- 2026-10-22: "Not tried yet" (the v0.2.0 hands-on checklist hasn't been run).
