# Surround output rewire: design

> **Parked 2026-10-28, not built.** Its second red team
> (`docs/superpowers/reviews/2026-10-28-hardening-batch-design-redteam-2.md`) found that handing
> sounds to the mixer directly would make one-shot Ogg and some MP3 files silent, and that looped
> ones are already silent in v0.2.2. That bug was fixed first
> (`2026-10-28-silent-loop-fix-design.md`). Nobody here has a surround output, and rodio's next
> release may remove span lengths altogether. See `docs/parking-lot.md`.

Date: 2026-10-28. Size: a small change (two call sites, one new wrapper, tests). Started as a
three-item "hardening batch" from `docs/parking-lot.md`; the red team
(`docs/superpowers/reviews/2026-10-28-hardening-batch-design-redteam.md`) showed two items were not
worth building and the third aimed at the wrong cause. The developer picked this rewire instead.

## The bug

cli-timer plays every sound as `Player::connect_new(output.mixer())` then
`player.append(fit_channels(source, channels))` (`src/sound.rs` `play`, `src/app.rs`
`start_sound`). `connect_new` adds the player's queue to the mixer **before** anything is
appended. `Mixer::add` wraps it in a `UniformSourceIterator`, whose constructor bootstraps at once
(`rodio-0.23.0/src/source/uniform.rs:38`) from the queue's current source: `Empty`, which reports
1 channel (`source/empty.rs`), with a keep-alive span of `threshold(1) = 512` samples
(`queue.rs:129-133`). So the first 512 samples of the real sound are converted as mono. On a
6-channel output 512 = 85 x 6 + 2, so every later frame is rotated by two channels, and at equal
sample rates nothing re-aligns it.

Effect on a 5.1 output: a stereo `-s` file plays as `[R, R, R, R, L, R]` (left only from
back-left); a native 6-channel file is rotated too. The mono built-in alarm and chime carry the
same sample on every channel, so the rotation is invisible for them. Stereo outputs are not
rotated (512 is whole stereo frames) but the first 256 frames of a stereo file play as mono.

The mechanism was read in rodio's source by the agent; the red team measured it through rodio's
real `Player`/queue/mixer path with no device (its review has the table and the controls). With
the sound handed to the mixer directly, its measurement was 0 wrong frames at equal rates, and 0
at 44.1 kHz into 48 kHz **only** with the span cap below.

## The change

1. **Hand the sound to the mixer directly**: `output.mixer().add(...)`, no `Player`. The mixer's
   converter then bootstraps from the real source's channels and span at sample 0.
2. **A small wrapper replaces what `Player` gave us**: a signal when the sound has ended. It holds
   the sending side of a channel and drops it when the inner source returns `None` (and on drop),
   so the waiting side wakes in both cases and never hangs.
   - `sound::play` waits on that signal instead of `sleep_until_end`.
   - `start_sound` (the looping full-screen alarm) needs no stop flag: its thread already ends on
     the app's stop or on the sender being dropped, and returning drops the output device sink,
     which stops the sound, as `player.stop()` plus that drop does today.
3. **Span cap in `FillChannels::current_span_len`**: report `(len / from).min(32768 / to) * to`
   instead of `len / from * to`. `UniformSourceIterator::bootstrap` (`uniform.rs:55`) cuts spans at
   32 768 samples, which is not whole 6-channel frames; without the cap a resampled widened span is
   cut mid-frame. (The first design blamed `Buffered`; it sits upstream of `FillChannels`, on 1- or
   2-channel audio, and never sees this number.)
4. **Testable without a device**: split the core of `play` so it takes a `&Mixer`; `play` opens
   the device and calls it. Tests build `rodio::mixer::mixer(channels, rate)` and pull samples
   from the `MixerSource`, which is what the device would get.

## Tests (the oracle)

Through the real mixer, no device:
- Stereo source (L = 0.1, R = 0.9 everywhere) into a 6-channel mixer at the same rate: every output
  frame is `[0.1, 0.9, 0.9, 0.9, 0.9, 0.9]`.
- Same at a different rate (44.1 kHz source, 48 kHz mixer): no frame whose channel 0 is not 0.1,
  past the resampler's first frames if it needs a ramp (builder measures, then pins the bound).
- A native 6-channel source into a 6-channel mixer comes out unrotated.
- A stereo source into a 2-channel mixer is not mono at the start.
- The end signal fires when a finite source ends, and the wait returns.
- Control first: each frame test must fail on today's wiring (`Player::connect_new` then
  `append`), so the test is proved able to see the bug before the fix makes it pass.

Plus a unit test of the span cap: a stereo span over 5 461 frames widened to 6 channels reports a
multiple of 6 that is at most 32 768. Existing span and fixture tests keep passing.

## Approaches

| Approach | Verdict |
|---|---|
| **A. Direct to the mixer, own end-signal wrapper** | Picked. Fixes equal-rate and resampled cases (red team: 0 wrong frames with the cap). |
| B. Keep `Player`, append before adding its queue to the mixer | Rejected: red team measured 60-100 % rotated frames once the file's rate differs from the device's. |
| C. Report upstream and wait | Rejected: fixes nothing here. The developer also said: post nothing upstream. |

## Non-goals

- No hyphen-value change to `-l`/`-s`: `-l=-standup` and `-l-standup` already work, and the change
  would turn `-t 1m -l --help` into a timer labelled "--help".
- No version in release archive names: the folder inside would change every release, so a PATH
  entry keeps running the old binary, and fixed `releases/latest/download/<name>` links would end.
- No rodio issue or PR.
- No version bump or tag in this change.

## Left to the builder

- The wrapper's name and whether it lives in `sound.rs`; whether `fit_channels` returns it.
- Exact frame-test bounds at different rates (measure, then pin).

## Housekeeping

- `docs/parking-lot.md`: remove the "Surround channel-fill span length" entry (fixed here); in the
  "Leading `-`" entry, record that `-l=-x`/`-l-x` already work and why the change was declined; in
  the "Release archive names" entry, record the PATH and stable-URL costs; in the "rodio's next big
  release" entry, note that `play` no longer uses `Player`, so re-check the wiring when moving.
- `NEXT.md`: its "the alarm plays from every speaker" check cannot see the rotation (mono sound);
  replace it with a stereo `-s` file check on surround.
