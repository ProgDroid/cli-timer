# Red team 2: surround output rewire design (2026-10-28)

Reviewed: `docs/superpowers/specs/2026-10-28-hardening-batch-design.md` (the rewritten "Surround
output rewire" design). Context only: the first red team,
`docs/superpowers/reviews/2026-10-28-hardening-batch-design-redteam.md`.
Read against: `src/sound.rs` (`source`, `bound`, `fit_channels`, `FillChannels`, `open_output`,
`play` and its tests), `src/app.rs` (`start_sound`), `src/alert.rs`, `src/ding.rs`, `src/remind.rs`,
`NEXT.md`, `docs/parking-lot.md`, `tests/fixtures/README.md`, and rodio 0.23.0's source in the
cargo registry: `mixer.rs`, `player.rs`, `queue.rs`, `stream.rs`, `source/uniform.rs`,
`source/buffered.rs`, `source/repeat.rs`, `source/take.rs`, `source/empty.rs`, `source/amplify.rs`,
`conversions/sample_rate.rs`, `decoder/symphonia.rs`, `fixed_source.rs`, `lib.rs`.

Stance: hostile. The question is whether this is the right thing to build, not whether the code
in the spec would compile.

## Evidence: a second probe, through real decoders and the real device

The first red team's probe fed the mixer a synthetic stereo buffer that reports its whole length
as one span. Real `-s` files come through rodio's symphonia decoder, which reports spans
differently, so I measured with decoded files. A scratch crate (outside the repo, rodio `=0.23.0`
with cli-timer's exact feature list, the repo's `Cargo.lock`) copies `FillChannels` verbatim with
a switch for the proposed cap, adds the design's end-signal wrapper (drops a `Sender` on `None`
and on drop), and runs three wirings:
`today` (`Player::connect_new` then `append`), `direct-nocap` (`mixer.add(fit_channels(..))`, today's
formula) and `direct-cap` (the design). Sources: the repo's own fixtures (`tests/fixtures/tone.*`,
mono 22 050 Hz) and files made with ffmpeg at 44.1 kHz stereo: constant L = 0.1 / R = 0.9 as WAV,
default FLAC and FLAC with 16 384-frame blocks; and "sine on the left, silence on the right" as
WAV, MP3 and Ogg Vorbis. Without a device it pulls 3.5 s from `rodio::mixer::mixer(ch, rate)`'s
`MixerSource`. With a device it opens this machine's default output exactly as rodio does
(`DeviceSinkBuilder::open_default_sink`, `log_on_drop(false)`), runs today's `play` and the
design's `play` at zero volume (`.amplify(0.0)`, which forwards `current_span_len` unchanged,
`source/amplify.rs:80`) and times the wait.

Controls run first: the FLAC fixture plays its full length on every wiring (rows below), and the
tail probe's "last non-silent frame" agrees with the end signal to within one frame, so the
silence metric can see sound and the timing can see an end.

### Table 1: what a decoder reports as its span (no device)

| File | First `current_span_len`, before any `next()` | Largest span |
|---|---|---|
| `tests/fixtures/tone.ogg` (repo) | **Some(0)** | 512 samples |
| `tests/fixtures/tone.mp3` (repo) | **Some(0)** | 47 |
| `tests/fixtures/tone.flac` (repo) | Some(2304) | 2 304 |
| `sineL44.ogg` (ffmpeg libvorbis) | **Some(0)** | 2 048 samples = 1 024 frames |
| `sineL44.mp3` (ffmpeg libmp3lame) | Some(94) | 2 304 = 1 152 frames |
| `const44.wav`, `sineL44.wav` | Some(2304) | 2 304 = 1 152 frames |
| `const44.flac` (ffmpeg default) | Some(9216) | 9 216 = 4 608 frames |
| `const44_big.flac` (`-frame_size 16384`) | Some(32768) | 32 768 = 16 384 frames |
| synthetic buffer (the first probe's source) | Some(264 600) | whole sound |

Why `Some(0)`: `SymphoniaDecoder::init` keeps whatever the first packet decodes to
(`decoder/symphonia.rs:114-140`), while `next()` loops past packets with no frames
(`:300-326`, "Loop until we get a packet with audio frames"). The first audio packet of a Vorbis
stream yields no samples by the codec's design (overlap-add needs two blocks), so this should hold
for every Vorbis file (2 of 2 measured); for MP3 it depends on the encoder (1 of 2 measured).

### Table 2: frames of sound that reach the output (no device, 3.5 s pulled)

Repo fixtures, 0.5 s tone. "Loop" is `repeat_infinite()`, as `remind` (`bound`) and the
full-screen alarm (`start_sound`) use it. 2- and 6-channel outputs, 48 kHz and 44.1 kHz, gave the
same pattern; one cell per case:

| File | today, once | today, loop | design, once | design, loop |
|---|---|---|---|---|
| `tone.flac` | 23 377 | 143 326 | 23 988 | 143 928 |
| `tone.ogg` | 23 892 | **0** | **0** | **0** |
| `tone.mp3` | 23 370 | **0** | **0** | **0** |

(48 kHz, stereo output. The ffmpeg-made stereo `.ogg` behaves like `tone.ogg` on every wiring.)

### Table 3: the same, on this machine's real output device (2 channels, 48 kHz), zero volume

| File | Playback | today's `play` waited | design's `play` waited |
|---|---|---|---|
| `tone.flac` | `Once { 5 s }` (as `ding`) | 456 ms | 469 ms |
| `tone.flac` | `Loop { 2 s }` (as `remind --ring 2s`) | 1 949 ms | 1 967 ms |
| `tone.ogg` | `Once { 5 s }` | 466 ms | **7 ms** |
| `tone.ogg` | `Loop { 2 s }` | **8 ms** | **8 ms** |
| `tone.mp3` | `Once { 5 s }` | 456 ms | **9 ms** |
| `tone.mp3` | `Loop { 2 s }` | **9 ms** | **0 ms** |

Not run through the cli-timer binary itself (it would raise a desktop pop-up); the probe makes
the same rodio calls `play` makes, in the same order. A by-hand confirmation is cheap:
`cli-timer remind 1s -s tests/fixtures/tone.ogg` should ring for 30 s and, per this table, is
silent and returns at once today.

### Table 4: channel rotation, 6-channel output (wrong frames / non-silent frames)

| Source | Device rate | today | direct, no cap | direct, cap |
|---|---|---|---|---|
| synthetic buffer | 48 kHz (resampled) | 96 775 / 144 405 | 95 096 / 143 985 (first at 5 944) | 0 / 144 002 |
| `const44_big.flac` | 48 kHz | 96 775 / 144 405 | 95 096 / 143 985 (first at 5 944) | 0 / 144 002 |
| `const44.wav`, `const44.flac` | 48 kHz | 144 348 / 144 434 | **0** / 144 014 | 0 / 144 014 |
| `sineL44.wav`, `sineL44.mp3` | 48 kHz | ~100 % | **0** | 0 |
| every file above | 44.1 kHz (equal) | ~100 % | 0 | 0 |

Same results with `repeat_infinite` in front (rows not repeated). This confirms the first red
team's central finding (today's wiring is rotated, the direct wiring is not) on real decoded files,
and adds one thing: **for every file whose packets are at most 4 608 frames, the direct wiring is
already 0 wrong without the cap.** The cap only changes the outcome for spans over 5 461 frames: the
synthetic buffer and a FLAC encoded with non-default 16 384-frame blocks.

### Table 5: the end signal and the tail (no device, `sineL44.wav`)

| Output | today: signal at frame / last non-silent frame | design: signal / last non-silent |
|---|---|---|
| 2 ch, 48 kHz | 144 248 / 144 248 | 144 014 / 144 014 |
| 6 ch, 48 kHz | 144 433 / 144 433 | 144 014 / 144 014 |
| 2 ch, 44.1 kHz | 132 987 / 132 986 | 132 301 / 132 300 |
| 6 ch, 44.1 kHz | 133 157 / 133 156 | 132 301 / 132 300 |

The design's frame counts are the ideal ones: 132 300 = 3 s x 44 100, and 144 014 is 3 s at 48 kHz
plus 14 frames of resampler restarts. Today's are longer by the start-up head.

### Answers to the specific checks

- **Does `Mixer::add` drop a source once it returns `None`?** Yes. `MixerSource::sum_current_sources`
  uses `retain_mut` and drops the box on the first `None` (`mixer.rs:148-161`), so both of the
  wrapper's paths (on `None` and on drop) fire inside the same audio callback.
- **Does `play` return before the last samples reach the device?** At the rodio layer, no
  regression: the design's signal fires on the same pull as the last sample, exactly as the
  queue's `signal_after_end` does today (`queue.rs:278-281`; Table 5, within one frame). At the
  device layer both versions return when the last sample is *pulled into* the cpal callback
  buffer, then drop the `MixerDeviceSink`, which drops the `cpal::Stream` (`stream.rs:58-63`); what
  is still in that buffer is lost either way. Unchanged, so not an objection, but the spec's
  "a signal when the sound has ended" means "pulled", not "heard".
- **Does dropping the `MixerDeviceSink` stop sound as promptly as `player.stop()`?** Yes, and it is
  what stops it today too. `Player::stop` only sets an atomic that `periodic_access` reads every
  5 ms of audio inside the callback (`player.rs:130-133`, `:301-303`); `start_sound`'s thread
  returns right after it, dropping `player` and then `output`. The spec's sentence is supported.
- **Is anything else `Player` provided relied on?** Volume, pause, speed, seek, position and the
  5 ms control loop: no, nothing in `src/` calls them (only `connect_new`, `append`,
  `sleep_until_end`, `stop`). **But the queue behind `Player` provided something the spec does not
  list: it sanitises the span length.** `SourcesQueueOutput::current_span_len` treats a source
  reporting `Some(0)` as exhausted and reports a frame-aligned threshold instead
  (`queue.rs:150-168`). That guard is why `ding -s x.ogg` plays today. See objection 1.
- **Does the cap make resampled output frame-aligned?** Yes, when it engages. `bootstrap` cuts the
  source with `Take` at the reported length (`uniform.rs:55-63`); `SampleRateConverter` reads whole
  frames into `current_span`/`next_frame`, and when `next_frame` comes back empty at the end of a
  chunk it drains the last whole frame and stops (`sample_rate.rs:110-122`, `:172-199`), so a chunk
  that ends on a frame boundary produces whole output frames, and the next chunk starts on channel
  0. Table 4: 0 wrong frames with the cap in every case. Each chunk restarts the interpolation
  phase (`next_output_span_pos_in_chunk = 0`), which adds a frame or so per chunk (the +14 above)
  on every wiring; that is rodio's, not this design's.

## Objection 1 (b, assumed rather than established): "an end signal is all `Player` gave us" is false; the rewire makes `ding -s` silent for every Ogg file and some MP3s

The spec's change 2 says the new wrapper "replaces what `Player` gave us: a signal when the sound
has ended". It also gave the queue's span guard. Without it:

- `Mixer::add` bootstraps straight from the decoder. A decoder that reports `Some(0)` before its
  first `next()` (Table 1: both of the repo's lossy fixtures) gets `Take { n: Some(0) }`;
  `UniformSourceIterator::next` finds it empty, re-bootstraps once, gets `Some(0)` again (nothing
  advanced the decoder) and returns `None` (`uniform.rs:76-89`); the mixer drops the source. No
  sample is played, the end signal fires at frame 1, `play` returns `Ok(())`, and `ding` exits 0.
  `bound`'s `take_duration` does not help: `TakeDuration::current_span_len` passes a smaller inner
  value through, `Some(0)` included (`take.rs:136-147`). Neither does `fit_channels`:
  `0 / from * to = 0`, capped or not, and on a stereo output it is not even in the path.
- Measured on this machine's real output (Table 3): today's `ding`-style playback of `tone.ogg`
  waits 466 ms (the tone is 0.5 s); the design's waits 7 ms. Same for `tone.mp3` (456 ms to 9 ms).
  This is a regression on the stereo device the developer actually has, to fix a rotation on a
  surround device nobody has.
- **The loop paths are already silent today**, on every wiring and every output (Tables 2 and 3):
  `repeat_infinite()` wraps the decoder in `Buffered`, whose `extract` turns a first span of
  `Some(0)` into `Span::End` (`buffered.rs:102-105`), so `remind -s x.ogg` returns at once and the
  full-screen alarm with `-s x.ogg` plays nothing. Nothing in `NEXT.md` or `docs/parking-lot.md`
  mentions it (grep for `silent`, `Some(0)`, `span_len`, `ogg` there finds only unrelated lines).
  The v0.2.2 fixture tests cannot see it: `real_mp3_flac_and_ogg_files_decode` collects the
  decoder with `next()`, which skips the empty packet; the loop test uses the built-in alarm.
- The spec's oracle cannot see either: its frame tests use synthetic constant sources (never
  `Some(0)`), and "the end signal fires when a finite source ends, and the wait returns" passes
  when the sound never plays. The control ("each frame test must fail on today's wiring") proves
  the tests can see rotation, not that sound comes out.

What to do: before any rewire, make `sound::source` never hand rodio a `Some(0)` span while samples
remain (for example a small adapter that reports `None` in that case; `None` is safe for a file,
whose channels and rate do not change), with a test that pulls a decoded `tone.ogg` and `tone.mp3`
through `bound(.., Loop)` and through a `rodio::mixer::mixer` and asserts non-silence. That fixes a
bug every user with an Ogg alarm has today, and it is a precondition for the direct wiring, not an
option.

## Objection 2 (a, a simpler design with most of the value): the value is bounded by a surround listener who does not exist; keep `Player` and spend the batch elsewhere

- Who hears the bug: only people on a 3+-channel output playing a stereo (or 3+-channel) `-s`
  file. The built-in sounds are mono and unaffected (the spec says so). `NEXT.md:11-12`: "not
  heard by anyone yet: no surround output here"; this machine's default output is 2 channels at
  48 kHz (Table 3); the last recorded reach is 0 stars and 0 forks. On a stereo output the spec's
  own words put today's whole effect at the first 256 frames, about 5 ms.
- The "correct" output the oracle pins, `[L, R, R, R, R, R]` on 5.1 (FL FR FC LFE BL BR), is itself
  a lopsided image: centre, LFE and both rears carry only the right channel. The fix moves the
  left channel from back-left to front-left and leaves four of six speakers right-only.
- The 80 % design keeps `Player`, `sleep_until_end` and `stop`: on outputs of 3+ channels, have
  `fit_channels` average a file's frame to mono before widening. A frame whose channels are all
  equal cannot be rotated audibly. Measured through today's wiring, sine at 0.25 peak: the largest
  inter-channel difference after start-up is 0.0157 at both 44.1 and 48 kHz, which is exactly one
  input sample of skew (0.25 x 2 pi x 440 / 44 100 = 0.01567), i.e. a 23 microsecond offset between
  speakers. The only larger frame (0.134) is the single last frame, where `take_duration` ends
  mid-frame and the queue pads two zeros. No wrapper, no cap, no mirrored rodio constant, no
  change to the alarm thread. It costs the stereo image on surround outputs, an image the design
  would render as above.
- The span cap, as specified, protects almost nothing real: decoded spans measured 1 024 to 4 608
  frames (Table 1), under the 5 461 the cap needs to engage, and Table 4 shows the direct wiring at
  0 wrong frames on all of them without it. The parking-lot entry carried that qualifier; the
  spec's "without the cap a resampled widened span is cut mid-frame" dropped it.
- Cheapest of all: file the rotation as a bead with Tables 4 and 5 and the first red team's
  table, and do objection 1's fix instead, which helps the hardware that exists.

## Objection 3 (c, the collision in six months): this moves off rodio's playback API just before rodio replaces it

- `docs/parking-lot.md` already expects rodio's "bigger release" to force another pass over
  `src/sound.rs` and the alarm thread, and lists three copies of rodio internals to re-check then
  (`open_output`'s fallback, `fit_channels`' mapping, `log_on_drop`). rodio 0.23 itself says the
  `speakers` API "will eventually replace" the device sink, and ships an `experimental`
  `FixedSource` trait that "never change[s] sample rate or channel count" and has no
  `current_span_len` at all (`fixed_source.rs:1-19`; gated in `lib.rs:194-218`).
- The design adds, on top of those, a dependency on three more rodio internals: that `Mixer::add`
  bootstraps a `UniformSourceIterator` immediately, that it cuts at a private `32768`
  (`uniform.rs:55`, mirrored as `32768 / to` with no test that fails when rodio changes it), and
  that the mixer drops a source on its first `None`. It also replaces `Player`, the one part of
  rodio's API whose semantics (`sleep_until_end`, `stop`) are documented, with a hand-rolled signal.
- In a `FixedSource` world the bug class (a converter bootstrapped from the wrong span) does not
  exist, so the cap and the wrapper become dead code to delete, and the new frame tests, written
  against `rodio::mixer::mixer` and span behaviour, have to be rewritten rather than reused as the
  migration oracle. The work buys a fix for a listener who does not exist yet and expires at the
  migration the project already knows is coming.
- The housekeeping makes it worse: `NEXT.md`'s check becomes "a stereo `-s` file check on
  surround", which nobody here can run, so the hands-on check before the next release either
  stays open or gets ticked unrun.

## Fixed check: facts set side by side, and what a reader infers

| # | Sentence (paraphrased) | Facts paired | Reader infers | Verdict |
|---|---|---|---|---|
| 1 | Header: "a small change (two call sites, one new wrapper, tests)" | A count of touched places | Nothing else about playback changes | **Unsupported.** The rewire also drops the queue's span guard: `ding -s x.ogg` goes silent (Table 3). Objection 1. |
| 2 | Intro: the red team "showed two items were not worth building and the third aimed at the wrong cause. The developer picked this rewire instead." | Red team's findings + the developer's pick | The pick weighed the red team's other option ("drop item 1 and file a bead") and found surround worth it | **Unsupported by the files.** `NEXT.md`'s feedback log records only the original batch pick; `NEXT.md:12` says there is no surround output here. If the developer did weigh it, write that down. Objection 2. |
| 3 | The bug: "every later frame is rotated by two channels, and at equal sample rates nothing re-aligns it" | Rotation by 2 + nothing re-aligns at equal rates | At unequal rates something does (partly) re-align it | **Unsupported for real files.** The first red team's 70 % figure came from a synthetic source cut every 32 768 samples; with decoded files (spans under 5 461 frames) today's wiring is ~100 % wrong at 48 kHz too (Table 4). Minor. |
| 4 | The bug: built-ins are invisible to the rotation + "stereo outputs are not rotated ... first 256 frames play as mono" | Unaffected built-ins + a 5 ms head on stereo | The fix's audible value is confined to surround outputs | **Supported**, and it is the conclusion the spec does not draw. With fact 2 (no surround output), the value to known users is about 5 ms. Objection 2. |
| 5 | "With the sound handed to the mixer directly, its measurement was 0 wrong frames at equal rates, and 0 at 44.1 kHz into 48 kHz only with the span cap" | Measured through rodio's real path + 0 wrong frames | The design has been measured to play `-s` files correctly, and real files need the cap | **Unsupported, twice.** The measurement used a synthetic buffer, never a decoder; with decoders the direct wiring plays nothing for Ogg and `tone.mp3` (Tables 2, 3), and is 0 wrong without the cap for every normal file (Table 4). Objections 1 and 2. |
| 6 | Change 3: bootstrap "cuts spans at 32 768 samples, which is not whole 6-channel frames; without the cap a resampled widened span is cut mid-frame" | The 32 768 cut + 32 768 mod 6 = 2 | Real `-s` files get mid-frame cuts today without the cap | **Unsupported.** Only spans over 5 461 frames reach the cut (Table 1); the parking-lot entry said so and the spec dropped it. Objection 2. |
| 7 | Approaches, A: "Fixes equal-rate and resampled cases (red team: 0 wrong frames with the cap)" | Fixes + measured 0 | Picking A changes nothing else | **Unsupported.** Same as 1 and 5. |
| 8 | Change 2: "`start_sound` needs no stop flag: its thread already ends on the app's stop or on the sender being dropped, and returning drops the output device sink, which stops the sound, as `player.stop()` plus that drop does today" | Thread ends on stop + the drop stops sound | Stopping is as prompt as today | **Supported** by `player.rs:130-133`, `:301-303` and the drop order in `app.rs:225-234`. |
| 9 | Change 2: the wrapper drops the sender "when the inner source returns `None` (and on drop), so the waiting side wakes in both cases and never hangs" | Two wake paths | `play` cannot hang | **Overstated.** If the device stops pulling (a cpal stream error, a device removed), the mixer neither yields `None` nor drops the source: the stream, and the closure owning `MixerSource`, live in `output`, which `play` holds. Today's `sleep_until_end` hangs the same way, so it is not a regression; drop "never hangs". |
| 10 | Tests: "Control first: each frame test must fail on today's wiring" + "The end signal fires when a finite source ends, and the wait returns" | Tests can see rotation + the wait returns | The oracle shows sound reaches the output | **Unsupported.** The end-signal test passes on a sound that never played (signal at frame 1, Table 2). Objection 1. |
| 11 | Housekeeping: the old check "cannot see the rotation (mono sound); replace it with a stereo `-s` file check on surround" | The old check is blind + a new check | Someone can run the new check | **Unsupported.** `NEXT.md:12`: no surround output here. Objection 3. |

Displacement: flags 1, 5, 7 and 10 are objection 1; flags 2, 4 and 6 are objection 2; flag 11 is in
objection 3. Flags 3 and 9 are minor and left as notes. None is left over to displace an
objection.

Residuals decomposed:
- Table 4, direct wiring at 44.1 kHz: 132 300 non-silent frames = 3 s x 44 100. Sums.
- Table 2, `tone.flac` once, design: 23 988 frames at 48 kHz = 0.5 s x 48 000 (24 000) less 12.
  Today's 23 377 is 611 frames fewer; that difference is **not decomposed** (it lies in today's
  start-up head) and nothing here rests on it. The objection rests on the 0-frame rows, which need
  no decomposition.
- Mono downmix skew: 0.0157 measured = 0.01567 predicted (one input sample of a 440 Hz sine at
  0.25 peak, 44.1 kHz); the one frame above it is the padded last frame (0.134 in channels 0-3,
  zeros in 4-5). Sums.
- Table 3: the playing cases wait 456 to 469 ms for a 0.5 s tone, 31 to 44 ms short. That is
  consistent with the signal firing when the last sample is pulled, about one callback buffer
  before it is heard, but the buffer size was not measured, so the residual is **not decomposed**.
  The silent cases (0 to 9 ms) need no decomposition: they are shorter than one 0.5 s tone by
  two orders of magnitude.

## Smaller notes

- The start-up head is not always "converted as mono". `Empty` reports 48 kHz (`empty.rs:49-51`);
  on a device at another rate, `SampleRateConverter::new` pre-pulls two samples from the queue at
  `Mixer::add` time (`sample_rate.rs:58-71`), before anything is appended, which swaps in the 512
  keep-alive `Zero` samples. So at 44.1 kHz the head is about 470 frames of silence (first
  non-silent frame 472 in the probe), and the rotation by 2 still follows. The spec's "the first
  256 frames of a stereo file play as mono" is the 48 kHz-device case.
- `NEXT.md` "Known rough edges" and the parking lot should gain the Ogg/MP3 loop silence whatever is
  decided here.
- Probe crate: `C:\Users\Nando Ferreira\AppData\Local\Temp\claude\G--rustDev-cli-timer\4c191b0e-5535-4bb1-a701-2153c69a6c98\scratchpad\probe2`
  (modes `spans`, `rot`, `mono`, `tail`, `ogg`, `device`, `seam`; logs beside it), media in
  `...\scratchpad\media`. Scratch, not kept. While setting up I overwrote the first red team's
  `scratchpad\probe\Cargo.toml` and `Cargo.lock`; the manifest was restored with
  `clap = "=4.6.7"` (`derive`) as its `main.rs` needs, but its exact original lock file is gone.
