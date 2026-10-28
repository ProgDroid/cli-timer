# Red team 2: silent looped Ogg/MP3 fix, revised implementation plan

Date: 2026-10-28. Reviewed: `docs/superpowers/plans/2026-10-28-silent-loop-fix.md` (the revision
made after `2026-10-28-silent-loop-fix-plan-redteam.md`) against
`docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md`, `src/sound.rs`, `src/app.rs`,
`src/remind.rs`, `src/alert.rs`, `NEXT.md` and rodio 0.23.0's source (`mixer.rs`, `player.rs`,
`queue.rs`, `decoder/mod.rs`, `source/take.rs`, `source/mod.rs`).

## How it was checked

A throwaway crate outside the repo (`<scratchpad>/redteam6/crate`): `git archive HEAD` of the repo
(same `Cargo.toml`, rodio features, `rust-toolchain.toml`), with the repo's `Cargo.lock`
unchanged by the builds (`diff` clean before and after). Task 1 was applied as written: new
`looped`, `alarm_loop`, `ALARM_FADE_IN`, `bound(&Sound, Playback) -> Result`, `play` calling
`bound(sound, playback)?`, `start_sound` calling `sound::alarm_loop(&self.sound)?`, the two
existing tests updated, the three helpers built from the Interfaces text, and all eight test
snippets (six from Task 1, two from Task 2) pasted verbatim. Task 2's file branch
(`Decoder::builder().with_data(BufReader::new(file)).with_byte_len(len).with_seekable(true).build_looped()`,
`metadata()` failure mapped to `Decode`/`IoError`) sits behind a `green` cargo feature, so the
same code runs in both states. Extra diagnostic modules measured beyond the plan's assertions.
The repo was not modified, apart from this file.

## What holds (measured, both states)

| Plan claim | RED (`repeat_infinite`) | GREEN (`build_looped`) |
|---|---|---|
| Snippets and helpers compile against rodio 0.23 | Yes, all eight plus helpers, as written | Yes |
| `cargo clippy --all-targets -- -D warnings` | Clean **only after** removing `Source` from `src/app.rs:9` (objection 2) | Same |
| `looped_files_sound_every_time_round_through_the_player` (loud per play window, threshold p/4) | ogg [0,0,0], mp3 [0,0,0] -> FAIL on `tone.ogg: play 1 silent`; flac [7967, 8140, 8140] | ogg [8012, 8182, 8182], mp3 [7805, 7976, 7976], flac as red -> pass |
| `looped_files_repeat_exactly` | FAIL `left: 0, right: 33792` (tone.ogg); mp3 also 0 | 3p exactly, play 2 == play 3, and play 1 == one fresh decode, for all three |
| `the_full_screen_alarm_sounds_after_its_fade_in` (loud in `out[p..]` > p/2) | ogg 0, mp3 0 -> FAIL `tone.ogg: silent after the fade-in` | ogg 16 364 (thr 5 632), mp3 15 952 (thr 5 512) |
| `a_looped_file_rings_for_the_requested_time` == 44 100 | ogg 0 -> FAIL (`left: 0`); mp3 0; flac 44 100 | 44 100 for all three |
| `one_play_of_each_fixture_through_the_player_has_sound` (control) | ogg 7 983, mp3 7 776, flac 7 938 (thr ~2 800): pass | same |
| Tripwire, three cases assert 0 samples | looped ogg 0, looped mp3 0, one ogg 0: pass | same: pass |
| `looped_files_keep_sounding_when_resampled` (Task 2 Step 1 red) | FAIL `tone.ogg: silent after seam 1` | seams [3538, 3538] / [3470, 3473] / [3542, 3543] loud of 4 800 |
| `a_looped_wav_repeats_exactly` (control) | pass | pass |
| Task 1 Step 3: "the same four fail on tone.mp3" | Every mp3 quantity behind the four asserts is 0 (diagnostic, not by editing the lists) | n/a |
| 204 tests on Windows | `cargo test`: 166 passed, 5 failed (4 Task 1 + the Task 2 resampled test) | 171 unit + 0 main + 33 e2e + 0 doctests = **204**, exit 0 |

The player-path oracle is representative of real devices: GREEN looped and `alarm_loop` sound
through `fit_channels` + `Player` + mixer at 2 ch 48 kHz, 6 ch 48 kHz, 2 ch 44.1 kHz and 1 ch
22.05 kHz (about 34 800 loud samples per channel in the second second); RED is 0 in every
configuration for ogg and mp3. Appending after the mixer has already been pulled (the app's real
order: the device starts pulling before `append`), with 1, 300, 511, 512, 513 or 4 801 frames
pulled first, changes nothing (all within 30 samples of the pre-0 figure). So the revision's main
change, `through_player`, does what the first red team asked.

## The three objections

### 1. The count thresholds detect only total silence; the spec asked for measured, pinned ones (spec fit, a)

The spec leaves "how the tests define 'has sound' in a window (a peak or energy threshold),
measured on the fixtures and pinned" to the builder. The plan pins the peak (`PEAK = 0.05`) but
not the counts, which are unmeasured round fractions:

| Test | Threshold | Measured GREEN | What still passes |
|---|---|---|---|
| per-play window, 22.05 kHz | `> p/4` (2 756-2 816) | 7 805-8 182 (about 0.72 p) | each play about 65% silent |
| after the fade | `> p/2` over 2 p | 15 952-16 364 | play 3 entirely silent (play 2 alone is ~8 000 > 5 632) |
| resampled seam window | `> 0` of 4 800 | 3 470-3 543 | 4 799 silent samples (100 ms) after every seam |

Review Focus 1 says "sound after every seam"; the test answers "one sample above 0.05 somewhere
in the 100 ms after it". These tests catch this bug (all-zero), which is why they go red, but
they cannot see a partial regression at the seam, which is the new risk the switch introduces
(each seam now re-probes and re-decodes the file). A gap already exists and is invisible to
them: the GREEN Ogg loop's longest quiet run is 176 samples at 22.05 kHz (382 at 48 kHz) against
7 (15) for FLAC, because Vorbis plays 11 264 samples per pass against 11 025 for the same 0.5 s
tone. It is identical to what `repeat_infinite` would have produced (play 1 == one decode), so it
is not a regression, but nothing in the plan would notice if it grew. Fix: derive the thresholds
from the measurement and pin them with a stated margin, for example `> p * 6 / 10` per play,
each of plays 2 and 3 separately for the alarm, and for the seam window either `> 4 800 / 2` or
"longest quiet run after the first loud sample < N" with N measured (ogg 176/382, flac 7/15).

### 2. Task 1's clean-up names the wrong import, and the literal instruction breaks the build (codebase assumption, b)

Task 1 Step 2: "Remove the now-unused `fade_in`/`StdDuration` use from `app.rs` if clippy flags
it." In `src/app.rs`, `fade_in` is a method, not an import, and `StdDuration` stays in use at
`src/app.rs:85` (`App::new(time: StdDuration, ...)`). What becomes unused is the trait import
`use rodio::{Player, Source};` at `src/app.rs:9`: measured, `cargo clippy --all-targets -- -D
warnings` fails with `error: unused import: Source --> src\app.rs:9:21` (exit 101) in both
states, and plain `cargo build` warns. An executor that does what the sentence says removes
`StdDuration` and gets an unresolved-name error; one that does not touch it still fails the
Task 1 gate ("fmt and clippy must still be clean"). Fix: "remove `Source` from the `rodio` import
at `src/app.rs:9`; `StdDuration` is still used by `App::new`".

### 3. Task 0 commits a spec that contradicts the tests and misstates the symptom, and the release notes are to be written from it (six months, c)

The revision moved the oracle and grew the tripwire, but no task touches the spec, and Task 0
commits it as is. The committed design record will then say, against the committed tests:

- Tests read "through rodio's queue (`rodio::queue::queue(false)`, then a
  `UniformSourceIterator`)"; the plan's Global Constraints forbid exactly that model.
- The tripwire is "the looped `tone.ogg` ... yields no sound"; the test is three cases asserting
  0 samples.
- Review Focus 5's "Not testable without a device" stands in the plan beside a helper that tests
  the Player path without one.
- "The bug": "`remind -s x.ogg` rings silently for the whole ring time and still prints
  'fired'". Measured on the RED build with a real device: `play(tone.ogg, Loop { for_: 3 s })`
  returned `Ok` after **68 ms**. `bound` yields 0 samples (`a_looped_file_rings_for_the_requested_time`
  measures that), so `sleep_until_end` returns at once: `remind` does not ring silently for the
  ring time, it skips the ring and reports "fired" immediately. The plan's Goal repeats
  "instead of ringing silently".

The plan says release notes "are written at release time from the spec's Release section", and
the v0.2.3 author will read the spec's bug section beside it. In six months the spec is where a
reader (or the parked surround rewire, which the parking-lot entry points at) learns how this was
tested and what the bug did; it will send them to a `queue(false)` model and a wrong symptom. A
user who noticed "remind returns instantly with my .ogg" will not recognise the release note.
Fix: in Task 0 (or a Task 3 step), amend the spec's Tests section to the player-path oracle and
the three-case tripwire, and correct "The bug" to "returns at once without sound"; same in the
plan's Goal.

## Fixed check: facts set side by side, and the claim a reader infers

1. Task 2 Step 3: "196 today + 6 (Task 1) + 2 (Task 2) = 204 on Windows (171 unit + 33
   end-to-end); 205 on Linux/macOS". Inferred: no existing test is removed and the two updated
   tests are not counted as new. **Supported**, measured: 164 `#[test]` in `src/` (alert 17, app
   10, cli 24, ding 15, duration 15, handler 9, output 22, remind 22, sound 30) minus 1
   `#[cfg(unix)]` (`alert.rs:382`) = 163; + 6 + 2 = 171; + 33 = 204, exactly what `cargo test`
   printed (171 / 0 / 33 / 0 doctests). The other `cfg(` lines in `src/alert.rs` (55, 67, 83) gate
   non-test code. Linux/macOS 205 is arithmetic, not run.
2. Task 3's parking-lot text: direct wiring "today silences looped Ogg and low-rate MP3 and
   one-shot Ogg (pinned by `ogg_and_low_rate_mp3_need_rodios_player_to_be_heard`)". Inferred: the
   list is complete, so one-shot low-rate MP3 (`ding -s x.mp3`) would survive direct wiring.
   **Unsupported**: one-shot `tone.mp3` fed to the raw converter also yields 0 samples (both
   states; FLAC 11 025 as control). The rewire would silence `ding` for 22.05 kHz MP3 too, and
   nothing pins it. Add it to the list (and to the tripwire, as a fourth case) or say "including".
3. Tripwire comment ("fed straight to rodio's converter ... yield no samples at all. While this
   holds, sounds must go through `Player`. The day it fails, the parked surround rewire may be
   possible") beside `assert_eq!(n, 0)`. Inferred: a non-zero count means direct wiring now makes
   sound. **Unsupported**: the assertion counts samples, not loud ones; a rodio change that emits
   padding zeros instead of nothing trips it while the rewire would still be silent. The three
   cases also share one loop, so the first that changes hides the other two. Assert per case and
   measure `loud(..)`, not `count()`. (The test also lacks an in-test positive control; outside
   it, looped FLAC gives 30 000 and one FLAC 11 025, so the probe can return something.)
4. Spec, "The bug": "`remind -s x.ogg` rings silently for the whole ring time and still prints
   'fired', exit 0" beside "`ding -s x.ogg` plays". Inferred: only loudness is wrong; remind's
   timing is fine. **Unsupported**: 68 ms for a 3 s ring (objection 3).
5. Task 1 Step 3: "Expected FAIL, on `tone.ogg` ... in each [four]. ... Everything else passes,
   including the tripwire and the one-play control." Inferred: exactly four failures in that run.
   **Supported**: the four fail with the stated messages; my combined run showed five because it
   also held Task 2's resampled test, which fails as Task 2 Step 1 expects. The follow-up "with
   `tone.ogg` temporarily removed from each list" is ambiguous for the tripwire, whose list is a
   typed `[_; 3]` array: removing one entry does not compile. Say "from the four failing tests'
   lists".
6. Ring test comment ("2 s of 22.05 kHz mono lands exactly on 44 100") beside the assert over all
   three fixtures. Inferred: all three are 22.05 kHz mono. **Supported** by
   `real_mp3_flac_and_ogg_files_decode` (asserts 1 ch, 22 050 Hz) and measured 44 100 each.
   Residual decomposed: `TakeDuration` uses `floor(1e9 / 22 050)` = 45 351 ns per sample (0.4739 ns
   short) and yields while `remaining > per-sample`, so the count is `floor(D / 45 351)`: 2 s ->
   44 100.46 -> 44 100 (0 over); 5 s -> 110 251.15 -> 110 251 (measured 110 251, 1 over: 110 250 x
   0.4739 ns = 52 250 ns, more than one 45 351 ns step). Exactness holds up to about 4.3 s at this
   rate; the comment is right.
7. Task 0: "`docs/superpowers/reviews/2026-10-28-*` (the five red-team reviews)". Inferred: the
   glob commits five files. **No longer true**: five were untracked when the plan was written;
   with this review the glob matches six. The glob does the right thing; the count is stale.
8. Global Constraints: sound-or-silence assertions "never through a hand-built model of `Player`"
   beside the tripwire's hand-built `UniformSourceIterator::new(src, MONO, rate)`. Inferred: the
   tripwire is a model. **Supported as equivalent**: `Mixer::add` is exactly
   `UniformSourceIterator::new(source, channels, sample_rate)` (`mixer.rs`), so it is the real
   direct path; `mixer.add` on a `rodio::mixer::mixer` would say so without a reader checking.

## Smaller findings

- Task 0 creates `silent-loop-fix` in the main checkout and commits on it "before creating any
  worktree". `git worktree add` refuses a branch already checked out elsewhere, so the main
  checkout must switch back to `main` first (carrying the modified `NEXT.md` and
  `.beads/issues.jsonl` with it). One line in Task 0 would save the executor a detour.
- Task 2 Step 2 asks to "share the open-and-map code with `source`". Then `source` stops using
  `Decoder::try_from(File)` and builds via the builder too; same behaviour (`try_from` is that
  builder chain, `decoder/mod.rs:280-295`), but `every_decoder_error_reads_as_it_did_with_rodio_0_16`
  is the only guard on the wording, and it does not reach the `metadata()` branch.
- `LoopedDecoder` on a file that decodes to no samples returns `None` from every `next()` and
  re-probes the file each call; the queue drops it on the first `None`, so no hang, same outcome
  as `repeat_infinite` (ends at once). Noted because it is the one input where the two differ in
  work done.
