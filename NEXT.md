# NEXT

## Where we are
**v0.2.3 is merged into `main` but not tagged or released yet** (2026-10-28). It fixes a bug that
shipped in v0.2.2: a looped `-s` sound was silent for **Ogg Vorbis files and for MP3s at 24 kHz or
below**.
- `remind -s x.ogg` skipped its ring: when the time was up it printed "fired" within about 70 ms
  and never made a sound.
- The full-screen timer with `-s x.ogg` stayed silent until a key was pressed.
- `ding -s x.ogg` was fine, and so were 44.1/48 kHz MP3s, FLAC and WAV. That is why your v0.2.2
  check with an mp3 passed.

The cause: rodio 0.23's decoder says the first chunk of such files is empty, and the looping code
took that as the end of the sound. Files now loop through rodio's own looping decoder, which goes
back to the start of the file. The built-in alarm is unchanged. As a side effect, a looped file is
no longer kept in memory as it plays (that only mattered for long files with long rings).

How we got here: the session started on a three-item "hardening batch" from the parking lot.
Reviews showed two items were not worth doing (written up in the parking lot as declined), and the
third, the 5.1/7.1 surround work, turned up both a bigger surround problem (now parked) and this
bug. 204 tests pass on Windows; new tests play sound through rodio's real playback path with no
device, and failed before the fix. Design, plan and six reviews:
`docs/superpowers/specs/2026-10-28-*`, `docs/superpowers/plans/2026-10-28-silent-loop-fix.md`,
`docs/superpowers/reviews/2026-10-28-*`.

There is no open work in `bd`. Not done yet: pushing `main`, release notes, and the v0.2.3 tag.

## How to run
- Install: `cargo install --locked --force --git https://github.com/ProgDroid/cli-timer`, or a
  release archive, or `cargo install --locked --force --path .` from this checkout.
- `cli-timer remind 1m -l "stretch"` / `cli-timer ding "done"` / `cli-timer -t 00:00:10`.
- In Claude Code (plugin installed): "remind me in 20 minutes to stretch", "cancel that reminder",
  "ding me when you're done".
- Tests: `cargo test` (204 on Windows: 171 unit, 33 end-to-end). They never pop notifications or
  play sound.
- Open work: `bd ready`.

## How it behaves
`remind` prints a "set" line at once, waits, then shows a pop-up titled cli-timer with your label
(or "Timer finished") and loops the alarm for 30 s (`--ring` up to 10 m), prints "fired" and exits.
`ding` chimes once and shows a pop-up ("Done" by default). With `--json`, one JSON object per line.
Exit codes: 0 ok, 1 failed before anything fired, 3 fired but the sound or pop-up failed.
Cancelling a reminder in Claude Code kills the process, which also stops a ringing alarm.

## Try this
Install this checkout first (`cargo install --locked --force --path .`), then, from the repo root:
- `cli-timer remind 3s -s tests/fixtures/tone.ogg --ring 5s`: a short beep repeating for about 5 s
  (in 0.2.2 this printed "fired" at once, silently).
- `cli-timer -t 3s -s tests/fixtures/tone.ogg`: the beep fades in and keeps repeating until you
  press a key.
- `cli-timer remind 3s -s tests/fixtures/tone.ogg --ring 5s --json`: only JSON lines, nothing else.
- If you have a long Ogg or low-rate MP3 of your own, loop it past its end once and listen at the
  restart.

## Things you might notice
- An Ogg file has about 8 ms of near-silence at the end of every loop (the decoder does not trim
  the end of the stream). It is short, and far better than the silence it replaces.
- On a 5.1/7.1 output, stereo `-s` files play with their channels shifted (left only from
  back-left). This was already true in 0.2.2 and is parked; the mono built-in alarm and chime are
  not affected. See "Surround output rewire" in `docs/parking-lot.md`.
- Your install points at this folder, so Claude Code reads the skill straight from
  `plugin/skills/timer/` at every session start (a resumed session keeps the old copy; restart
  fully to pick up a skill edit).
- The plugin validator warns that the plugin has no icon (only matters for Anthropic's directory).
- A sound file deleted after the timer started still fails silently when the timer ends.
- The toast shows as coming from "Windows PowerShell" (accepted).
- Reminders die if you exit Claude Code (accepted; the detached design is in the parking lot).

## Tell me if
- Any `-s` file that rang in 0.2.2 sounds different when it loops, or stops early.
- An Ogg or MP3 `-s` file is still silent, through `remind` or the full-screen timer.
- The full-screen alarm no longer fades in.
- `--json` ever prints anything that is not a JSON line.
- A reminder fires late, twice, or not at all, or a `cli-timer.exe` lingers after one ends.

## Housekeeping
- **To release v0.2.3** (the version is already bumped in both `Cargo.toml` and
  `plugin/.claude-plugin/plugin.json`): push `main`, do a manual release run (dry run) and check
  four archives appear, then push the `v0.2.3` tag (lightweight), then write the release notes by
  hand (`gh release edit v0.2.3 --notes-file <file>`) from the "Release" section of
  `docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md`.
- **Bumping Rust is a deliberate change**: edit `channel` in `rust-toolchain.toml`, run
  `cargo clippy --all-targets -- -D warnings` and `cargo test`, fix any new lints in the same
  change.
- No PRs: work on a branch, squash-merge into `main` locally with a `handoff:` subject, push
  `main`. The `silent-loop-fix` branch is squash-merged and safe to delete; so is `deps-refresh`
  (locally and on GitHub).

## Known rough edges
See `docs/parking-lot.md`: the surround output rewire (parked, with why); rodio 0.23 is an
upstream stopgap before a bigger release (expect another pass on the sound code; a "tripwire" test
says when the parked surround fix becomes possible); Git Bash rewrites a label starting with `/`;
x86_64-only Windows/Linux binaries; the plugin version is tied to the crate version. macOS and
Linux sound and pop-ups have not been checked by a person since 0.2.2.

## Feedback log
_Your answers to the session-start question, dated, newest first._
- 2026-10-28: "Still all good" (v0.2.2). Picked the small hardening batch next (surround span
  fix, hyphen-leading full-screen `-l`/`-s` values, version in release archive names). After the
  reviews, agreed to: rewire surround properly (then parked when the silent-loop bug turned up),
  post nothing to rodio's GitHub, switch to the silent-loop fix with rodio's looping decoder,
  native execution, fix both final-review minors, merge into `main` locally.
- 2026-10-27 (later): ran the v0.2.2 hands-on check on Windows: "all is well". v0.2.2 tagged and
  released.
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
