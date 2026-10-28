# Red team: silent looped Ogg/MP3 fix, implementation plan

Date: 2026-10-28. Reviewed: `docs/superpowers/plans/2026-10-28-silent-loop-fix.md` against
`docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md`, `src/sound.rs`, `src/app.rs`,
`Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `tests/cli.rs`, `docs/parking-lot.md` and
rodio 0.23.0's source (`decoder/mod.rs`, `decoder/builder.rs`, `decoder/symphonia.rs`, `queue.rs`,
`player.rs`, `mixer.rs`, `source/uniform.rs`, `source/take.rs`).

## How it was checked

A throwaway crate outside the repo (`<scratchpad>/redteam5`) with the repo's `Cargo.toml`
(same package name and features), the repo's `Cargo.lock` (unchanged by the build: `diff` clean,
so rodio 0.23.0 and the same symphonia versions) and `rust-toolchain.toml` (1.99). `src/sound.rs`
was copied, Task 1 applied (new `bound(&Sound, Playback) -> Result`, `looped` as
`repeat_infinite()`, the two existing tests updated, the plan's helpers and all six test
snippets pasted verbatim), and Task 2's switch put behind a `green` feature
(`Decoder::builder().with_data(BufReader::new(file)).with_byte_len(len).with_seekable(true).build_looped()`).
Extra fixtures were made with ffmpeg in the probe folder only (stereo 44.1 kHz Ogg and MP3, stereo
22.05 kHz MP3, 3-minute 48 kHz stereo Ogg and MP3). The repo was not modified, apart from this file.

## What holds (verified, not assumed)

| Plan claim | Measured |
|---|---|
| Snippets compile against rodio 0.23 | Yes, all six, plus the helpers. Only gap: the Interfaces line writes `SampleRate::new(rate)`; it returns `Option`, so the helper needs `.unwrap()` (the tripwire snippet already has it). |
| clippy `-D warnings` (repo's `[lints.clippy]`) on the snippets | Clean. A `pub fn looped` returning `Result` needs a `# Errors` doc section (`missing_errors_doc`); the plan's doc-comment instruction omits it, the gate will catch it. |
| Task 1 red: `looped_files_play_every_time_round_through_the_queue` fails on `tone.ogg` with `out.len()` 0 | Yes: `left: 0, right: 33792`. `tone.mp3` also 0 (diagnostic), `tone.flac` 33 075 = 3p. |
| Every other test passes in Task 1, incl. tripwire and one-play control | Yes (the tripwire passes on 0 samples, see Fixed check). |
| After the switch, exactly 3p, play 3 == play 2 | Yes for ogg (p = 11 264), mp3 (11 025), flac (11 025). Also play 1 == one fresh decode for all three, and raw `LoopedDecoder` samples == queue output. |
| Resampled window arithmetic (`p * 48000 / 22050`) | Real one play at 48 kHz through the queue is 24 508 / 23 983 / 23 997 (ogg/mp3/flac) against the plan's 24 520 / 24 000 / 24 000: 12 / 17 / 3 short per play, so the computed seam is at most ~34 samples late after two plays. Irrelevant against a 4 800-sample window. Residual decomposed below. |
| Ring tolerance 8 at 22.05 kHz, 2 s, `tone.ogg` | Exactly 44 100 (diff 0); same for mp3 and flac. |
| Wav repeats exactly | Yes, passes before and after (control). |
| Tripwire after the switch | Passes: `LoopedDecoder` still reports `Some(0)` first; raw converter yields 0 samples. |
| 196 today, 202 after | 197 `#[test]` attributes (164 in `src/`, 33 in `tests/cli.rs`) minus 1 `#[cfg(unix)]` (`alert.rs:382`) = 196 = 163 unit + 33 e2e, matching `NEXT.md`. +6 = 202 on Windows (169 unit); 203 on Linux/macOS. No doctests found. |
| `src/app.rs:208-210` | Correct lines (`sound::source(...)?`, `.repeat_infinite()`, `.fade_in(...)`). |
| `decoder/mod.rs:280-295` is `TryFrom<File>` | Correct. |
| Parking-lot entries named in Task 3 exist | "Leading `-`", "Release archive names", "rodio's next big release", "Surround channel-fill span length": all present. |
| Version lives in exactly two files | `git grep 0.2.2` outside docs/lock/`.beads`: only `Cargo.toml` and `plugin.json`. |

Extra measurements beyond the plan's fixtures (green build):

- Stereo 44.1 kHz Ogg and stereo 22.05 kHz MP3: first span `Some(0)`; looped through the queue:
  3p, play 1 == once, play 2 == play 3, left/right balance kept (L 0.032 / R 0.016 every play).
  Stereo 44.1 kHz MP3: first span `Some(94)` (not hit, as the spec says).
- Seam cost: a 3-minute 48 kHz stereo Ogg's seam `next()` (re-probe in `SymphoniaDecoder::new`)
  took 1.18 ms (release), against a worst 0.12 ms for any other `next()`; MP3 0.07 ms. This runs
  on the audio callback thread. Well under a typical 10 ms device period on this machine; not
  measured on a slow disk or debug build.

## The three objections

### 1. The full-screen alarm, half of the bug, has no oracle (spec fit, a)

The spec names two broken loop sites: `remind` (`bound`) and the full-screen timer
(`start_sound`). The plan changes `start_sound` by a sentence in Task 1 Step 2 and never tests it.
Every new test calls `looped` or `bound`; no test in `src/app.rs` reaches `start_sound`. So all
202 tests pass whether `start_sound` calls `sound::looped` or is left on
`sound::source(..)?.repeat_infinite()`, and in the second case the full-screen `-s x.ogg` timer
stays silent. Measured on rodio's real playback path without a device (objection 2): today's
`repeat_infinite().fade_in(500 ms)` for `tone.ogg` and `tone.mp3` gives 0 samples above 0.05;
`looped(..).fade_in(500 ms)` gives about 20 000. The only hands-on item the plan keeps
(Review Focus 5) checks stderr of `remind --json`; no one is asked to hear an Ogg ring, either
through `remind` or the full-screen timer. That is exactly how this bug shipped: the 2026-10-27
hands-on used a 44.1 kHz MP3.

Fix: move the full-screen source into `sound.rs` (for example `pub fn alarm(sound) ->
Result<Box<dyn Source + Send>, SoundError>` = `looped(sound)?.fade_in(500 ms)`), have
`start_sound` call it, and test it through the player path with `tone.ogg` (assert sound after
the fade, in play 2 and 3). Add a "Try this" line: `cli-timer -t 3s -s tests/fixtures/tone.ogg`
and `cli-timer remind 3s -s tests/fixtures/tone.ogg --ring 5s` both ring audibly.

### 2. The oracle models `Player` instead of using it, and rodio offers the real thing (6 months, c)

The tests read the sound through `rodio::queue::queue(false)` plus a `UniformSourceIterator`. The
app never uses that: `play` and `start_sound` use `Player::connect_new(output.mixer())`, which is
`queue(true)` (keep-alive, a different `current_span_len` branch), seven adapters (`speed`,
`track_position`, `pausable`, `amplify`, `skippable`, `stoppable`, `periodic_access`,
`player.rs:121-130`), `fit_channels`, and a mixer `UniformSourceIterator` bootstrapped while the
queue is still `Empty`. rodio 0.23 makes the real path testable with no device:
`let (mixer, out) = rodio::mixer::mixer(ch, rate); let player = Player::connect_new(&mixer);
player.append(fit_channels(src, ch)); out.take(n)`. Measured with that harness:

| | today (`repeat_infinite`) | after the switch |
|---|---|---|
| looped `tone.ogg` via Player, samples > 0.05 | 0 | 25 850 |
| looped `tone.mp3` via Player | 0 | 25 204 |
| looped `tone.flac` via Player (control) | 25 724 | 25 724 |
| looped `tone.ogg` straight into the mixer (tripwire's case) | 0 | 0 |

So the real path discriminates red from green just as well, at the same cost. The model matters
in six months: `docs/parking-lot.md` already expects rodio's next big release to rework sound
(span lengths may go). If that release keeps `rodio::queue` and `UniformSourceIterator` but
changes how `Player` feeds the mixer, the plan's tests stay green while the app rings silently,
which is the failure class this fix exists to close (the bug was invisible because the tests
read a path the app does not use). Also the spec's claim "Not testable without a device" (Review
Focus 5) is right for stderr of device opening, but not for whether the Player path produces
sound.

Fix: define `through_queue` as the player path above (call it `through_player`), keep a one-line
note that `queue(false)` was the earlier model, and keep the tripwire as the one test that feeds
the mixer directly.

### 3. The spec, plan and every review the plan points at are untracked, and no task commits them (codebase assumption, b)

`git status` on `main`: the spec, this plan, and all four 2026-10-28 reviews are `??`.
Task 3 commits exactly `Cargo.toml Cargo.lock plugin/.claude-plugin/plugin.json
docs/parking-lot.md`, and the parking-lot entry it writes points at
`superpowers/specs/2026-10-28-hardening-batch-design.md` and both
`superpowers/reviews/2026-10-28-hardening-batch-design-redteam*.md`, also untracked. The bead
description points at the spec and the design reviews. The plan's header recommends
subagent-driven development, whose setup normally makes a git worktree: a worktree has none of
these files, so an implementer told "Spec: docs/superpowers/specs/..." finds nothing, and the
squash merge lands a parking-lot entry whose pointers resolve only in this one checkout. The
v0.2.2 cycle carried its spec and plan in the squash (`ddace0a`), so the convention exists; this
plan just does not carry the step.

Fix: Task 0, commit the spec, plan and the four reviews (and this one) on the branch before Task 1,
or state that the controller does it; add them to the files list the final squash must contain.

## Fixed check: facts set side by side, and the claim a reader infers

1. "Test count is today's 196 plus the six added in Tasks 1-2: 202 on Windows." Inferred: no
   existing test is removed and the six are new. Supported. Decomposed: 164 + 33 = 197 attributes;
   minus 1 `cfg(unix)` = 196 (163 + 33); plus 3 + 3 = 202 (169 + 33). Linux/macOS: 203.
2. Global Constraints: "No new fixture files." beside "Loop tests ... never include the full-screen
   alarm's 500 ms fade-in." Inferred: the loop tests stand in for the full-screen alarm, so it is
   covered. **Unsupported**: no test reaches `start_sound` (objection 1).
3. Task 2 Step 1: "these three fail on the Ogg/MP3 cases (or wav passes already: it is a
   control)." Inferred: three red tests, and MP3 shown red by them. **Unsupported**: two fail
   (`looped_files_keep_sounding_when_resampled`, `a_looped_file_rings_for_the_requested_time`),
   both panic at `tone.ogg` first, so the MP3 case is never evaluated; the ring test has no MP3
   case at all; the wav test passes. MP3's red is recorded only by Task 1's manual edit.
4. Task 3's parking-lot text: direct mixer wiring "silences the looping decoder and one-shot Ogg
   (the tripwire test ...)". Inferred: the tripwire watches both. **Unsupported**: it feeds only
   looped `tone.ogg`. Measured today: one-shot `tone.ogg` and one-shot stereo 22.05 kHz MP3 into
   the raw converter give 0 samples, looped `tone.mp3` gives 0; none is pinned. The tripwire's
   comment ("The day it fails, handing sounds to the mixer directly ... has become possible")
   extends the same unsupported reach. Pin one-shot `tone.ogg` and looped `tone.mp3` in the same
   test, or narrow the wording.
5. Task 1 Step 3: "Every other test passes, including the tripwire". Inferred: the tripwire saw
   silent output. It saw no output: `all()` over an empty `Vec` (0 of 30 000 samples). Same
   conclusion for wiring, but the assertion cannot tell silence from nothing; asserting the length
   is 0 would say what is meant.
6. The ring test: tolerance "<= 8" beside "rodio rounds the cut-off to a frame or two". Inferred:
   the overshoot is bounded at a frame or two for any file or ring. **Unsupported**:
   `TakeDuration` truncates nanoseconds per sample, so the error grows with ring time, rate and
   channels. Decomposed: 2 s at 22.05 kHz mono: floor(1e9/22 050) = 45 351 ns, 2e9/45 351 =
   44 100.68, yields 44 100 (diff 0). 5 s 44.1 kHz stereo: 11 337 ns, 441 033 samples + 1 queue
   pad to finish the frame = 441 034 (measured). 5 s 22.05 kHz stereo: 220 507 + 1 pad = 220 508
   (measured). 185 s 48 kHz stereo: 17 761 136 (measured), 1 136 over. The test passes only for its
   fixture; the comment should say why.
7. Review Focus 4 beside its test: "One-shot playback ... (`ding -s x.ogg`): must still play once,
   whole" and the control reads through `queue(false)`. Inferred: `ding` is shown to work.
   Partly supported: it shows the model works; `ding` uses `Player` (objection 2). Both are fine
   today on the Player harness.
8. Resampled residual: plan's p at 48 kHz vs the real one-play length (12 / 17 / 3 samples short).
   Decomposed: `UniformSourceIterator` restarts its rate converter at every span the queue reports
   (22 / 20 / 6 spans for ogg / mp3 / flac); the sum of an isolated converter's output for each
   span's input length equals the total exactly (24 508 / 23 983 / 23 997), so the whole residual
   is per-span restart loss, about 0.5 to 0.85 samples per span.

## Smaller findings

- Task 2 Step 2 contradicts a Global Constraint. "SoundError::Open ... including a failed
  `metadata()`" plus "share the open-and-map code with `source`" moves `source`'s metadata
  failure from `Decode` (today, via `Decoder::try_from`'s `IoError`) to `Open`, against "Errors
  keep their wording". Practically unreachable; pick one and say so.
- Task 1 commits a failing test while the Global Constraints list `cargo test` as a gate after
  every task. The plan says the failure is expected for fmt/clippy; say it for `cargo test` too,
  so a reviewer does not bounce the red commit. Do not push the branch before Task 2 (CI runs on
  every push).
- No task closes bead `clitimer-4xc` (`bd close`); earlier cycles did it in their own commit
  (`9896606`, `990d8db`).
- Release notes (spec, Release) are left to release time; fine, but the plan could say so.
