# Silent-loop fix: final review

Date: 2026-10-28. Range: `a641fee..90f8629` (branch `silent-loop-fix`, not pushed). Reviewer: final
code review, read-only on the checkout. Plan: `docs/superpowers/plans/2026-10-28-silent-loop-fix.md`.
Spec: `docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md`. Ledger:
`.superpowers/sdd/2026-10-28-silent-loop-fix/progress.md`.

## What was run

- `cargo fmt --all -- --check`: clean. `cargo clippy --all-targets -- -D warnings`: clean (exit 0).
- `cargo test` at `90f8629`: 171 unit + 33 end-to-end passed, 0 failed, `REAL_EXIT=0`. That is
  the plan's predicted 204 for Windows.
- **Mutation run** (a `git archive` copy in the scratchpad, separate target dir): with `looped`'s
  file branch reverted to `source(sound)?.repeat_infinite()`, five tests fail on `tone.ogg`
  (`a_looped_file_rings_for_the_requested_time` 0 vs 44 100, `looped_files_repeat_exactly` 0 vs
  33 792, `the_full_screen_alarm_sounds_after_its_fade_in` 0 loud,
  `looped_files_sound_every_time_round_through_the_player` play 1 has 0 loud,
  `looped_files_keep_sounding_when_resampled` 0 loud after seam 1). The tripwire and the controls
  pass. The tests see the bug, and the red state in `da09545`'s message is reproduced.
- **Scratch probes** (same copy, not in the repo):
  - Error wording, `source` against `looped`: identical for a missing file (`cannot open sound
    file …`), a text file named `.mp3` and the ADPCM fixture (`cannot decode sound file …:
    Unrecognized format`), and a directory.
  - A WAV with zero samples: `check` is `Ok`; `looped(..).take(1000)` yields 0 samples in 0.35 ms,
    `bound(Loop 2 s)` yields 0, the player path gives silence in 2.8 ms. No spin, no hang.
  - Every play of a looped fixture (`tone.ogg`, `tone.mp3`, `tone.flac`), play 1 and play 2,
    matches a one-shot play sample for sample.
  - Re-opening the decoder (the work `LoopedDecoder` does at every seam, on the audio thread),
    3-minute 48 kHz stereo files: Ogg 1.2 ms, MP3 (with or without a Xing header), FLAC and WAV
    0.25-0.3 ms in a release build. In a debug build: Ogg 16 ms, the others 0.5-2 ms.
  - Fade-in: through the player, the first 100 ms of `alarm_loop` has 0 loud samples for every
    fixture, against about 1 450 for `looped`.

## Strengths

- The fix is the smallest one that removes the cause. `Buffered` is gone from the file loop path,
  replaced by rodio's own `LoopedDecoder`. There is no custom `Source` and no span correction. As a
  side effect, a long file is no longer cached in memory while it rings.
- `file_decoder` copies `Decoder::try_from(File)` exactly (`rodio-0.23.0/src/decoder/mod.rs:280-295`):
  `BufReader`, `with_byte_len`, `with_seekable(true)`, and a failed `metadata()` mapped to
  `DecoderError::IoError(e.to_string())`. One-shot playback through `.build()` is therefore
  byte-for-byte the old decoder. `build()` and `build_looped()` share `build_impl`
  (`builder.rs:314-331`), so `check` predicts exactly whether `looped` will construct.
- Every loop site now goes through one function. A grep for `repeat_infinite` in `src` finds only
  the built-ins' arm of `looped` (`src/sound.rs:215`). `start_sound` stops building its own source,
  and the fade-in is now in `sound.rs`, where tests can reach it.
- The oracle is the right one: `through_player` runs rodio's real `Player` and mixer with no
  device, the layer that decides between sound and silence. The plan red team's hand-built
  `queue(false)` model was correctly dropped.
- Red, then green, with the same test code, recorded in `da09545`'s body. The mutation run above
  reproduces it.
- The tripwire is well made. It checks every case before failing, it explains itself, and it is
  linked from the parking lot. The day it trips, the reason the surround rewire was parked is gone.
- Parking-lot housekeeping matches the spec's Housekeeping section, and the commit split matches
  the plan task for task (`5206280` docs only, `da09545` red, `fa0d9ec` green with `clitimer-4xc`
  on its own line, `90f8629` version and parking lot).

## Issues

### Critical (Must Fix)

None.

### Important (Should Fix)

None.

### Minor (Nice to Have)

1. **The fade-in is not asserted anywhere.** `src/sound.rs:878`
   (`the_full_screen_alarm_sounds_after_its_fade_in`) checks only that there is sound after the
   fade. If `alarm_loop` dropped `.fade_in(ALARM_FADE_IN)`, every test would still pass. This is
   not a regression, since the fade was never tested. But the spec moved `alarm_loop` into
   `sound.rs` "so it is tested too", and the fade is the only thing `alarm_loop` adds over `looped`.
   Fix: in that test, also assert `loud(&out[..2_205]) == 0` (the first 100 ms at 22.05 kHz; the
   fade's peak there is at most 0.025, below `PEAK`). Probe: 0 loud samples with the fade and
   about 1 450 without it, for all three fixtures. This is one line and fits the "fix minors
   before merge" habit.

2. **The WAV control's signal repeats every 1 000 samples.** `src/sound.rs:456` (`write_wav`
   writes `i % 1000 * 10`), used by `a_looped_wav_repeats_exactly` (`src/sound.rs:974`) with a
   4 000-sample file. A seam offset by any multiple of 1 000 samples (125 ms) would still compare
   equal. It is only a control, and the sine fixtures in `looped_files_repeat_exactly` would catch
   such a shift, so the real exposure is small. Fix (optional): use `write_wav("loop.wav", 3_999)`,
   or any length that is not a multiple of 1 000. Do not change `write_wav`, because other tests
   pin its values.

3. **Ledger "tests:" lines quote the doc-test summary.** In
   `.superpowers/sdd/2026-10-28-silent-loop-fix/progress.md`, the Task 2 and Task 3 lines read
   `test result: ok. 0 passed`. This is the last summary line of the run, not the suite. The
   executor's notes correct it, and I measured 171 + 33 myself, but anyone who reads only the
   ledger sees "0 passed". This is a tooling issue (the `task-done` helper picks the last
   `test result:` line), not something to fix on this branch. Fix the helper so it sums the
   suites or quotes all of them.

## Declined to judge

- **A file that decodes to zero samples** (headers only): `check` passes, and `remind` then
  "fires" with a silent, instant ring. This behaves the same as before the change (an empty
  `Buffered` also ends at once) and the same as one-shot playback. The probe shows no spin. It is
  pre-existing and outside this fix.
- **Vorbis end padding**: `tone.ogg` decodes to 11 264 samples against 11 025 for the other
  fixtures, with about 170 trailing near-silent samples (8 ms), so each Ogg loop carries a small
  gap. The cause is symphonia/rodio not trimming the last Ogg page's granule position. Before
  this fix, Ogg loops made no sound at all, so this is an improvement, and the spec accepts seam
  quality "not worse". It is not cli-timer's to fix.
- **Seek failure mid-ring** (a removable or network drive disappearing, a FIFO given as `-s` on
  Unix): `LoopedDecoder` stops and the ring ends early in silence. Before, `Buffered` replayed
  from memory. The spec lists this as an accepted known limit.
- **The decoder is rebuilt on the audio thread at each seam.** Measured at 0.25-1.2 ms in
  release, so I do not expect an audible glitch. A debug build of a long Ogg takes 16 ms, which
  could click on a small device buffer, but users run release builds. Not a finding. The
  hands-on check below can include a long Ogg if wanted.
- **The surround channel rotation** (`Player::connect_new` before `append`): this was parked
  deliberately and is a spec non-goal.
- **Release notes, the `NEXT.md` "Try this" entry (Review Focus 5) and `bd close clitimer-4xc`**:
  by plan, these happen at hand-off or release, after the merge, not on the branch. I did not
  check them as missing.
- **Review Focus 5 (real device, `--json` stdout purity)**: cannot be tested without a device.
  Nothing in the change writes to stdout or stderr (`LoopedDecoder` prints nothing), so the
  `--json` contract is not touched by this code.
- **TOCTOU between `check` and play** (the file changes or is deleted between the timer starting
  and the alarm): pre-existing and unchanged.

## Executor rulings

- **Branch in the main checkout, not a worktree** (to keep the uncommitted `NEXT.md` line out of
  a linked worktree): sound. The branch isolates `main` equally, and nothing uncommitted leaked
  into the commits (each commit's file list checked).
- **Task 1 ledgered by hand** (`task-done` records only passing runs): sound. `da09545`'s message
  carries the failure lines, and the mutation run reproduces them.
- **`&Path` instead of `&PathBuf`** in `file_decoder` and `decode_error` (clippy `ptr_arg`):
  correct, and idiomatic.

## Recommendations

- Take Minor 1 (the fade-in assertion) on the branch before merging. Minor 2 is optional.
- At hand-off, `NEXT.md`'s "Try this" should include `remind 3s -s tests/fixtures/tone.ogg --ring 5s`,
  `cli-timer -t 3s -s tests/fixtures/tone.ogg` and a `--json` run (Review Focus 5). Optionally
  add a long (minutes-long) Ogg looped past one seam, to hear the seam on real hardware. Then run
  `bd close clitimer-4xc` after the merge, as the plan says.
- Release notes for v0.2.3 should follow the spec's Release section: the silent loop for
  Ogg/low-rate MP3 in 0.2.2, `ding` unaffected, and optionally the memory line.

## Assessment

**Ready to merge?** Yes, ideally after the one-line fade-in assertion (Minor 1).

**Reasoning:** The change matches the plan and spec task for task. It fixes the cause with rodio's
own looping decoder and keeps one-shot playback, error wording and `check`'s prediction exactly as
they were. A mutation run proves the new tests see the bug through the real `Player` path.
Nothing found blocks the merge. The minors are a missing fade assertion and a weak control signal.
