# Red team 2: silent looped Ogg/MP3 fix design (revised: rodio's looping decoder)

Date: 2026-10-28. Target: `docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md` (the
revision that loops `-s` files with `Decoder::builder()…build_looped()`). Context read: the first
red team (`2026-10-28-silent-loop-fix-design-redteam.md`). Stance: hostile staff engineer looking
for why this is the wrong thing to build.

## Evidence base

Read: the spec; `src/sound.rs` (`source`, `check`, `bound`, `play`, `fit_channels`, the tests);
`src/app.rs` `start_sound`; `src/remind.rs`; `tests/fixtures/` and its README; rodio 0.23.0
`decoder/mod.rs` (`LoopedDecoder` and its `Source` impl), `decoder/builder.rs`,
`decoder/symphonia.rs`, `decoder/read_seek_source.rs`, `source/take.rs`, `source/fadein.rs`,
`source/uniform.rs`, `source/mod.rs` (`is_exhausted`), `queue.rs`, `player.rs`, `stream.rs`.

Probe: a throwaway crate outside the repo
(`C:/Users/NANDOF~1/AppData/Local/Temp/claude/G--rustDev-cli-timer/4c191b0e-5535-4bb1-a701-2153c69a6c98/scratchpad/redteam4`),
rodio `=0.23.0` with the repo's feature list, the repo's `Cargo.lock` and `rust-toolchain.toml`,
built `--release --offline`. The looped source is built exactly as the spec says (`BufReader`,
`with_byte_len(len)`, `with_seekable(true)`, `build_looped()`); "today" is `Decoder::try_from(File)`
plus `repeat_infinite()`, as `bound` and `start_sound` do now. Chains through `Player::new()`'s
queue (all of `append`'s wrappers) and a `UniformSourceIterator`, which is what `Mixer::add` wraps
every source in. Media: the three repo fixtures; the first red team's ffmpeg 4.4.1 files (mono MP3
at 16/44.1 kHz, stereo 44.1 kHz MP3/Ogg/FLAC, 5.1 Ogg/FLAC, a 180 s 48 kHz stereo Ogg, a 0-sample
wav); new: a 2 s stereo MP3, and 2 s 44.1 kHz MP3 and FLAC files with a 2.7 MB and a 10.9 MB
embedded cover picture (`art*.mp3`, `art*.flac`).

### Raw results

**E1. The wrap itself** (every `next()` timed over five plays; "wrap" is the call that crosses the
end; `Some(0)` checked after every sample):

| File | wrap call (µs, 4 wraps, two runs) | slowest other call (µs) | span right after wrap | `Some(0)` after sample 1 |
|---|---|---|---|---|
| tone.flac | 42-55 | 30 | Some(2304) | 0 |
| tone.ogg | 578-878 | 8-11 | Some(512) | 0 |
| tone.mp3 | 26-35 | 8-11 | Some(47) | 0 |
| st44100.ogg | 824-1 030 | 47-52 | Some(1152) | 0 |
| six.ogg (5.1) | 2 120-2 568 | 119-200 | Some(3456) | 0 |
| long180.ogg | 988-1 130 | 82-159 | Some(1152) | 0 |
| art.mp3 (2.7 MB picture) | 1 712-3 200 | 34-36 | Some(47) | 0 |
| art.flac (2.7 MB picture) | 1 250-2 458 | 53-59 | Some(4608) | 0 |
| art10.mp3 (10.9 MB picture) | 6 887-7 772 | 49 | Some(47) | 0 |
| art10.flac (10.9 MB picture) | 9 537-10 096 | 51 | Some(4608) | 0 |

(All 16 files: 0 occurrences of `Some(0)` after the first sample, across five plays each.)

**E2. Today versus the spec through `Player` queue + `UniformSourceIterator`, 7 s**, on files that
loop today:

| File | output | identical to today | first difference | max \|diff\| | max frame-to-frame step today / spec |
|---|---|---|---|---|---|
| tone.flac | 1 ch / 22 050 (native) | yes | none | 0 | 0.01569 / 0.01569 |
| tone.flac | 2 ch / 48 000 | no | sample 10 026 | 0.0101 | 0.01562 / 0.01215 |
| mono44100.mp3 | 1 ch / 44 100 (native) | yes | none | 0 | 0.00817 / 0.00817 |
| mono44100.mp3 | 2 ch / 48 000 | no | sample 2 712 (28 ms) | 0.0263 | 0.00817 / 0.00750 |
| st44100.mp3 | 2 ch / 44 100 (native) | yes | none | 0 | 0.00578 / 0.00578 |
| st44100.mp3 | 2 ch / 48 000 | no | sample 2 712 | 0.0203 | 0.00526 / 0.00528 |
| st44100.flac | 2 ch / 44 100 (native) | yes | none | 0 | 0.00555 / 0.00555 |
| st44100.flac | 2 ch / 48 000 | no | sample 10 030 | 0.0023 | 0.00555 / 0.00552 |
| art.mp3, st2s.mp3 | native / 48 000 | yes / no | none / 2 712 | 0 / ≤0.023 | equal / equal or lower |

Exact-zero output frames at 48 kHz (a gap detector): tone.flac today 41 frames, at and 62 ms and
100 ms after every seam; spec 1 (the first sample). st44100.flac today 7, at every seam; spec 1.
Files today cannot loop (tone.ogg, tone.mp3, st44100.ogg): today 0 non-zero samples, spec 672 000
of 672 000.

**E3. The full-screen composition literally** (`looped.fade_in(500 ms)`, three plays):

| File | one play | play 2 == play 1 | play 3 == play 2 | today, samples |
|---|---|---|---|---|
| tone.flac (control) | 11 025 | **false** | true | 33 075 |
| tone.ogg | 11 264 | **false** | true | 0 |
| tone.mp3 | 11 025 | **false** | **false** | 0 |
| st44100.ogg | 264 600 | **false** | true | 0 |

**E4. The `remind` composition** (`take_duration(3 s)`, raw): tone.flac today 66 150 samples and
plays 2+ equal play 1, spec the same; tone.ogg and tone.mp3 today 0, spec 66 150 and equal.
empty.wav today 0, spec 0 (ends in 0.26 ms; no spin).

**E5. Where the remaining `Some(0)` is handled**, 3 s, 2 ch / 48 kHz, non-zero samples:

| File | looped, straight into `UniformSourceIterator` | looped, via `rodio::queue::queue(false)` | today, via the queue |
|---|---|---|---|
| tone.ogg | **0** | 287 854 | 0 |
| tone.mp3 | **0** | 287 800 | 0 |
| tone.flac | 287 960 | 287 962 | 287 930 |

**E6. Span reports, mono44100.mp3**: `repeat_infinite()` (via `Buffered`) counts its span down
(47 → 1 → next); the looping decoder reports the whole current packet (`symphonia.rs:163-165`
returns `buffer.len()`, not what is left): `Some(47)` for samples 0-47, then `Some(1152)` from
sample 48 on, constant within the packet.

## Answers to the specific questions

- **What span does `LoopedDecoder` report at a wrap?** Not `Some(0)`. The wrap
  (`decoder/mod.rs:662-669`) seeks the byte reader to 0, builds a new `SymphoniaDecoder` (a full
  re-probe), and reads one sample before returning. For Vorbis that read moves past the empty first
  packet, so the span after the wrap is the first audio packet (Some(512) mono, Some(1152) stereo);
  for MP3 it is the first non-trimmed frame (Some(47) at 22.05 and 44.1 kHz, Some(94) stereo).
  Just before the wrap, the old decoder still reports its last packet's length. Measured: 0
  mid-stream `Some(0)` in 16 of 16 files (E1).
- **Queue, `UniformSourceIterator`, `TakeDuration`, `FadeIn` at the seam: silence, gap,
  misalignment?** None found. At the device's own rate, mono and stereo, output is sample-identical
  to today's for every file that loops today (E2). At a different rate it is not identical, from the
  second mixer span on, long before any seam (28 ms in): the looping decoder reports the full packet
  length where `Buffered` reported the remaining count (E6), so the mixer re-bootstraps its
  resampler at different points. No new click: the largest frame-to-frame step is equal or lower,
  and today's path has exact-zero frames at every seam that the spec's path does not (E2). Every
  span the decoder reports is a whole number of frames, so no channel rotation at constant channel
  count. `TakeDuration` re-reads the span only when its countdown hits 0 (`take.rs:102-109`) and
  uses it only for the per-sample duration, which is constant for one file. `FadeIn` uses a time
  ramp and never reads the span or the total duration.
- **MP3 encoder delay and padding at each seam?** No gap the old replay did not have. The wrap
  re-probes, so gapless trimming (on by default, `builder.rs:87`) is applied again from the LAME
  header: three plays equal one decode concatenated three times (E4, and the first red team's
  12-file table). Content of the seam is unchanged; only the work done at it is new (next point).
- **What does a wrap cost?** A re-probe on the thread that pulls the audio, which for `Player` is
  the output callback: 0.03 ms (MP3) to 2.6 ms (5.1 Ogg) on this machine, and roughly 1 ms per MB
  of embedded cover picture (7-10 ms at 10.9 MB) because the probe reads the metadata again. rodio
  asks for about 50 ms buffers (`stream.rs:207-216`: 2 048 frames at 48 kHz, 43 ms), so this fits;
  whether WASAPI honours a fixed buffer size was not checked (UNKNOWN). Not an objection, but the
  spec's "seeks the file back to the start" understates it: it is a byte seek plus a full re-probe,
  and the `with_byte_len`/`with_seekable` settings matter to the probe, not to the wrap's seek.
- **`total_duration`.** `LoopedDecoder::total_duration()` is always `None`
  (`decoder/mod.rs:734-736`). Nothing on either loop path reads it: `TakeDuration` counts its own
  time, `FadeIn` passes it through, the queue reports `None` anyway (`queue.rs:209-211`). No
  misbehaviour.
- **Can the red tests go red today, and do the controls pass on both?** The `remind` half: yes
  (E4: 0 today, 66 150 with the fix; the flac control passes on both). The full-screen half: no, and
  it cannot pass after the fix either (objection 1).

## Objection 1 (b): the full-screen half of the oracle cannot be satisfied, and cannot run red

The spec's red test: "Looping `tone.ogg` and `tone.mp3`, built … as the full-screen alarm builds
it, yields more samples than one play, and the second play's samples equal the first's", with the
flac loop as a control making "the same assertions". The full-screen alarm builds
`source.repeat_infinite().fade_in(500 ms)` (`app.rs:208-210`). Every fixture is 0.5 s, so the fade
covers the whole first play: with the fix, play 2 differs from play 1 for **every** file, the flac
control included (E3). tone.mp3 is exactly 0.5 s, so the ramp's last step lands on play 2's first
sample and even play 3 differs from play 2.

It also has nowhere to run red: today the full-screen composition exists only inline in
`start_sound`, inside a thread that opens the output device. A test of "the alarm as the full-screen
alarm builds it" on today's code must copy the composition into the test (a re-derivation, not the
code that ships), and the green version must call the new function, which does not compile against
today's code. So "the builder runs the red tests against today's code first and records that they
failed" will be met by a test that differs from the one kept.

What to do: the spec should say what the full-screen test compares (play 2 against play 3, or the
fade window excluded, or the fade checked separately), and phrase the red step against an entry
point that exists on both sides (`bound(source(..), Playback::Loop{..})` works; E4). If both sites
call one function, test that function once and keep `fade_in` out of the equality.

## Objection 2 (c): the bug that remains lives in a layer no proposed test reaches, and the next planned change removes that layer

The spec accepts that the looping decoder still reports `Some(0)` before its first sample and that
"the queue covers it". Measured: the looping decoder straight into a `UniformSourceIterator`, which
is what direct mixer wiring does, gives **0** non-zero samples for tone.ogg and tone.mp3; the same
source through `rodio::queue::queue(false)` gives 287 854 (E5). The cover is
`SourcesQueueOutput::current_span_len` treating `Some(0)` as exhausted (`source/mod.rs:187-189`)
and falling back to the size hint or a 512-sample threshold (`queue.rs:150-168`).

Every proposed test iterates the composed source raw. Raw, the looping decoder yields samples
whether or not anything downstream trusts its first span, so these tests pass on a build where the
alarm is silent through direct wiring. The parked surround rewire is the next piece of work the
spec itself names, and it removes `Player`; a rodio bump that changes the queue fallback, or the
span removal the spec's housekeeping mentions, would too. The failure that comes back is this bug's
own ("rings silently, prints fired, exit 0") with every test green. The parking-lot sentence is the
only guard.

Cheap repairs, any of which keeps the design: run the loop tests through
`rodio::queue::queue(false)` plus a `UniformSourceIterator` (both public; `sound.rs` already uses
the queue in `a_mono_frame_is_finished_before_a_stereo_span_starts`) and add one test that feeds
the looped source straight into a `UniformSourceIterator` and asserts the known `Some(0)` result,
so the day it changes is visible; or have `check` pull one sample out of the composition that will
actually play, which also catches the next decoder quirk.

This also retires the "longer fixture": it was asked for to reach `Buffered`'s 32 768-sample first
chunk and packet-straddling spans, which the chosen design does not have. tone.ogg already spans
about 22 packets (first audio packet 512 samples, 11 264 in all) and a wrap; the stereo wrap behaves
like the mono one (E1, E4). A test at the queue layer buys more than a new binary in the repo.

## Objection 3 (flagged implicature, displaces the weakest objection): the release note will ship two claims the evidence does not support

1. **"Whether an mp3 is hit depends on its encoder (the red team found one of two ffmpeg-made
   mp3s reports a non-zero first span)"**, and "some encoders' first MP3 packet", and the release
   note's "some MP3 `-s` files". The first red team corrected this with measurements and the revision
   kept it (the correction did not propagate). With one encoder (ffmpeg 4.4.1 libmp3lame), 3 of 6
   mp3s are hit, all at 22.05 kHz or below; the 44.1 and 48 kHz ones are not. Sample rate decides,
   not encoder. The residual sums: LAME's delay is 1 105 samples and gapless trimming is on; an MPEG-1
   frame (44.1/48 kHz) is 1 152, so the first frame keeps 1 152 − 1 105 = 47; an MPEG-2/2.5 frame
   (24 kHz and below) is 576, so the whole first frame is trimmed (span 0) and the second keeps
   2 × 576 − 1 105 = 47, which is exactly the span this probe reads after a wrap for tone.mp3 (E1).
   The developer's passing hands-on mp3 was most likely 44.1 kHz. The note should say "MP3 at 24 kHz
   or below", which tells a user whether they were affected.
2. **"`repeat_infinite()` keeps every decoded sample of the file in memory (67.0 MiB measured for a
   3-minute 48 kHz stereo Ogg)" beside "0.7 MiB measured for the same 3-minute Ogg"**, and the
   release note's "looped files no longer held in memory". The reader infers today's users pay about
   67 MiB. Neither fact establishes it: today's loop of that Ogg yields 0 samples and caches nothing
   (E4: Ogg loops are the bug); 67.0 MiB was the rejected wrapper's path, run for two full loops. And
   `Buffered` caches lazily, as far as the file has played: the cost is
   min(ring, file length) × rate × channels × 4 B. Decomposed: 67.0 = 65.9 (180 s × 48 000 × 2 × 4 B)
   + 0.7 (shared by both paths) + 0.4 (span headers and `Vec` slack). For `remind`'s default 30 s ring
   the same file costs at most 11.0 MiB (30 × 48 000 × 2 × 4 B); for a typical 2-5 s alarm sound, under
   2 MiB. The saving is real only for long files with long rings, in flac, wav or 44.1/48 kHz MP3.

Neither changes what to build: the looping decoder is still the right mechanism. They change what
the release promises.

## Fixed check: paired facts and the claim a reader infers

| # | Sentence (spec) | Pair | Inferred third claim | Verdict |
|---|---|---|---|---|
| 1 | "A Vorbis stream's first packet decodes to no samples by design, and so does some encoders' first MP3 packet" | Vorbis empty first packet + some MP3s | encoder decides for MP3 | **Unsupported** (objection 3.1) |
| 2 | Bug table: first span Some(0), played once 11 264 / 11 025, looped 0 | one play works + loop 0 | looping, not decoding, is broken | Supported (E4 reproduces it) |
| 3 | "`ding -s x.ogg` plays: it does not loop, and … `Player` queue falls back …" | one-shot plays + queue fallback | one-shot needs no fix | Supported on the `Player` path; false under direct wiring (E5), which the spec says elsewhere |
| 4 | "hands-on check with an mp3 passed; … depends on its encoder (one of two ffmpeg-made mp3s …)" | a passing mp3 + "1 of 2" | encoder decides; the dev's mp3 was a lucky encoder | **Unsupported**: the count is 3 of 6, one encoder, split by rate (objection 3.1) |
| 5 | "67.0 MiB measured for a 3-minute 48 kHz stereo Ogg … for every format" | 67 MiB + every format | today's loops cost ~67 MiB | **Unsupported**: wrapper path, Ogg cannot loop today, lazy cache (objection 3.2) |
| 6 | "holds no decoded copy (0.7 MiB …)" beside the 67 MiB | 0.7 vs 67 MiB | the fix saves ~66 MiB for users | **Unsupported** for the default ring and typical alarm sounds (objection 3.2) |
| 7 | "If seeking fails mid-ring (an I/O error), the looping decoder stops … Local files seek." | stops on seek error + local files seek | the ring never ends early in practice | Mostly supported; the wrap also ends if the re-probe fails, not only the seek (`decoder/mod.rs:664-666`) |
| 8 | "still reports `Some(0)` before its first sample. On the `Player` path … the queue covers it (measured: plays)" | Some(0) + plays | the remaining hazard is handled | Supported for today; untested at the layer that handles it (objection 2) |
| 9 | "Each loop decodes the file again, so the seam is whatever the decoder produces from the start" | re-decode + seam | the seam may differ from today's | Seam content identical at sample level (E4); output differs at a resampled rate from 28 ms on for another reason (E6), no new click (E2) |
| 10 | "every existing fixture is 0.5 s mono, which fits in one 32 768-sample chunk and one short decode" | 0.5 s + one chunk | a longer fixture is needed | **Unsupported** for the chosen design: the 32 768 chunk is `Buffered`'s, which the design removes (objection 2) |

## Considered and not raised

- Real-time cost of the re-probe at each wrap (E1): at most about 10 ms with a 10.9 MB embedded
  picture, against rodio's ~43 ms buffer. Would matter on a much slower machine, with a cold cache,
  or if the backend ignores rodio's fixed buffer size (UNKNOWN).
- Output change at a resampled rate for files that loop fine today (E2, E6): different, not worse.
  The unit tests run at the file's own rate and will not see it; nobody will hear it.
- `check` builds with `Decoder::try_from`, the loop with the builder: today they are the same
  settings (`decoder/mod.rs:280-295`), so "a file that decodes once also loops" holds; it would drift
  only if one of the two copies changes.

## What I could not establish

- No audible check on real hardware; all levels are computed from samples pulled through rodio's
  own queue and converters.
- Whether WASAPI honours rodio's fixed buffer size; wrap timings are from one machine, warm cache.
- Pipes and other non-seekable `-s` paths (process substitution on Linux/macOS): not tested. With
  the looping decoder the first wrap's byte seek would fail and the ring would end after one play;
  whether such a path opens today at all is UNKNOWN.
