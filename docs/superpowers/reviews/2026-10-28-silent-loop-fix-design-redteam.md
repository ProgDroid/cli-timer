# Red team: silent looped Ogg/MP3 fix design

Date: 2026-10-28. Target: `docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md`.
Stance: hostile staff engineer looking for why this is the wrong thing to build.

## Evidence base

Read: the spec; `src/sound.rs` (`source`, `bound`, `fit_channels`, `FillChannels`, `play`, the
fixture tests); `src/app.rs` `start_sound`; `tests/fixtures/README.md`; rodio 0.23.0
`decoder/symphonia.rs`, `decoder/mod.rs` (`LoopedDecoder`), `decoder/builder.rs`,
`source/buffered.rs`, `source/repeat.rs`, `source/take.rs`, `source/fadein.rs`,
`source/uniform.rs`, `conversions/channels.rs`, `queue.rs`, `player.rs`, `mixer.rs`.

Probe: a throwaway crate outside the repo
(`C:/Users/NANDOF~1/AppData/Local/Temp/claude/G--rustDev-cli-timer/4c191b0e-5535-4bb1-a701-2153c69a6c98/scratchpad/redteam3`),
rodio `=0.23.0` with the repo's exact feature list, resolved offline from the same registry
(rodio 0.23.0 and symphonia-core / -codec-vorbis / -bundle-mp3 0.5.5, the same versions as the repo's
`Cargo.lock`). It implements the spec's wrapper literally ("until the first sample has been read,
`Some(0)` becomes `None`"), copies `FillChannels`/`fit_channels`, and drives sources through
`Player::new()`'s queue plus a `UniformSourceIterator` (what `Mixer::add` wraps every source in).
Media: the three repo fixtures, plus files made with the same ffmpeg 4.4.1 as the fixtures: 3 s mono
MP3 at 16, 22.05, 44.1 and 48 kHz; 3 s stereo 44.1 kHz MP3, Ogg and FLAC; a 3 s 5.1 Ogg and FLAC
(channel k at amplitude 0.1·(k+1)); a 180 s 48 kHz stereo Ogg; an empty PCM wav (data chunk of 0
bytes).

### Raw results (E1: first span, three plays of the file)

| File | ch | rate | first span | first span, gapless off | today loop ×3 | wrapper loop ×3 (exact) | `LoopedDecoder` ×3 (exact) |
|---|---|---|---|---|---|---|---|
| tone.flac | 1 | 22 050 | Some(2304) | Some(2304) | 33 075 | 33 075 (yes) | 33 075 (yes) |
| tone.ogg | 1 | 22 050 | Some(0) | Some(0) | **0** | 33 792 (yes) | 33 792 (yes) |
| tone.mp3 | 1 | 22 050 | Some(0) | Some(576) | **0** | 33 075 (yes) | 33 075 (yes) |
| mono16000.mp3 | 1 | 16 000 | Some(0) | Some(576) | **0** | 144 000 (yes) | 144 000 (yes) |
| mono22050.mp3 | 1 | 22 050 | Some(0) | Some(576) | **0** | 198 450 (yes) | 198 450 (yes) |
| mono44100.mp3 | 1 | 44 100 | Some(47) | Some(1152) | 396 900 | 396 900 (yes) | 396 900 (yes) |
| mono48000.mp3 | 1 | 48 000 | Some(47) | Some(1152) | 432 000 | 432 000 (yes) | 432 000 (yes) |
| st44100.mp3 | 2 | 44 100 | Some(94) | Some(2304) | 793 800 | 793 800 (yes) | 793 800 (yes) |
| st44100.ogg | 2 | 44 100 | Some(0) | Some(0) | **0** | 793 800 (yes) | 793 800 (yes) |
| st44100.flac | 2 | 44 100 | Some(9216) | Some(9216) | 793 800 | 793 800 (yes) | 793 800 (yes) |
| six.ogg | 6 | 44 100 | Some(0) | Some(0) | **0** | 2 381 400 (yes) | 2 381 400 (yes) |
| six.flac | 6 | 44 100 | Some(27648) | Some(27648) | 2 381 400 | 2 381 400 (yes) | 2 381 400 (yes) |

"Exact" = the three-play output equals the single decode concatenated three times, sample for
sample. Every file was also read to the end, checking `current_span_len()` after every sample:
**0 occurrences of `Some(0)` after the first sample, in 12 of 12 files**, and after the end every
decoder still reports its last packet's length (non-zero), never `Some(0)`.

## Answers to the specific questions

- **Buffered's second extract after the wrapper's `None`.** With `None`, `extract` takes
  `min(None→32768, 32768)` samples. The second extract asks the wrapper (now passing through), which
  asks the decoder, whose `current_span_len()` is `Some(self.buffer.len())`: the **whole** current
  packet, not what is left of it (`symphonia.rs:163-165`). The first chunk ended mid-packet, so every
  later Buffered span is a packet-length run that straddles two packets. The data is still correct
  (exact equality above), but span boundaries are no longer packet boundaries (st44100.ogg:
  `[32768, 2048, 2048, …]`), and for 3/5/6/7 channels they are no longer frame boundaries either
  (six.ogg: **124 of 125 Buffered span starts are off-frame**, because 32 768 mod 6 = 2). For the
  0.5 s fixtures none of this happens: each fits in one Buffered span (`[11264]`, `[11025]`).
- **Second repeat: cached or re-queried?** Cached. `Repeat` holds a clone of the `Buffered` at its
  head (`repeat.rs:14-18`) and the chain of `Arc<Span>` is never re-extracted; the wrapper is not
  consulted again. Measured: exact equality across three plays.
- **One-shot ding path.** Unchanged and the end is still detected. tone.ogg, tone.mp3 and
  st44100.ogg through `take_duration(5 s)` → `Player` queue → `UniformSourceIterator(2 ch, 48 kHz)`
  give the same non-zero count, first and last non-zero index with and without the wrapper, and
  `player.empty()` is true afterwards (the `Done` wrapper fired, which is what `sleep_until_end`
  waits on). The queue only asks the sound's span at a `UniformSourceIterator` re-bootstrap, and the
  sound's `None` just means one long `Take`. One real difference: `TakeDuration` caches the span it
  saw at construction (`take.rs:14`) and only re-reads it when that count hits zero (`take.rs:102-109`);
  given `None` it never re-reads, so its `duration_per_sample` is frozen at the first span's
  rate/channels for the whole ding. Harmless for a constant-format file, but it falsifies the
  table's "every later span reaches rodio as before" on the `Once` path.
- **Is "until the first sample" the right trigger; can symphonia report `Some(0)` mid-stream?**
  No mid-stream `Some(0)`: `next()` only replaces the buffer with a packet that has
  `decoded.frames() > 0` (`symphonia.rs:315-322`), and at the end it returns `None` without touching
  the buffer. The only `Some(0)` is the one `init` keeps (`symphonia.rs:114-140`). Measured: 0 of 12.
  Consequence: the trigger is correct but **redundant**. A wrapper that maps every `Some(0)` to
  `None` (the rejected approach B) behaves identically on this decoder; the `started` flag is dead
  state.
- **Do the proposed tests fail on today's code?** Only the ogg and mp3 cases of the first bullet.
  The flac case passes today (33 075 samples looped). The empty-file case passes today: the 0-byte
  wav opens, reports `Some(0)`, and loops to 0 samples both today and with the wrapper. The
  "passes `Some(2304)` through" test cannot run on today's code (no wrapper to call).

## Objection 1 (a): a simpler design gets the value without the span lie

rodio 0.23 ships a looping decoder: `Decoder::builder()…build_looped()` (`decoder/mod.rs:86-106`,
`606-676`). It loops by seeking the file to 0 and rebuilding the symphonia decoder, then reading one
sample, so it never goes near `Buffered`. Measured on every file above: exact three-play output,
including tone.ogg, tone.mp3 and 6-channel Ogg; through the shipped `Player` path it plays (E2,
identical channel levels to the wrapper). And it does not cache the decoded file:

| 180 s, 48 kHz stereo Ogg, two full loops (34.56 M samples) | live heap with the source alive |
|---|---|
| wrapper + `repeat_infinite()` (the spec) | **67.0 MiB** |
| `LoopedDecoder` | **0.7 MiB** |

(Counting global allocator in the probe. Decomposition: 180 × 48 000 × 2 = 17.28 M samples × 4 B
= 65.9 MiB of cached samples, plus the 0.7 MiB both share, plus about 0.4 MiB of `SpanData` headers
and `Vec` slack for ~8 400 spans = 67.0 MiB.)

Cost of the alternative: `source()` would have to know whether it is building a loop (built-in
sounds keep `repeat_infinite()`, which is fine for `SamplesBuffer`), and `LoopedDecoder` still reports
`Some(0)` before its first sample, so it is **silent under direct mixer wiring** (measured: 0 samples
through a bare `UniformSourceIterator`). On the shipped `Player` path that does not matter: the
queue's fallback covers it, as the spec says for `ding`.

And if a wrapper is kept anyway, drop the state machine: since the decoder never reports `Some(0)`
after the first sample, "map `Some(0)` to `None`" is the whole wrapper; the A-versus-B row in the
approaches table distinguishes two behaviourally identical designs.

## Objection 2 (b): the test oracle is assumed satisfiable and covers the wrong path

"Each must fail on today's code first, then pass with the fix" is stated as the oracle, but:

- the flac case of the first bullet passes today (it is a control, not a red test);
- "a file that decodes to nothing still ends" passes today (measured, 0 = 0), so it cannot go red
  first and a builder following the rule literally will either distort it or report a red that never
  happened;
- "passes `Some(2304)` through" does not exist on today's code.

Worse, every proposed test uses 0.5 s fixtures (11 025 to 11 264 samples) that fit inside one
32 768-sample Buffered chunk. The path every real alarm file takes (any mono file over 0.74 s at
44.1 kHz, any stereo file over 0.37 s) is: a 32 768-sample first chunk forced by the wrapper's `None`,
then pass-through packet-length spans that straddle packets. No proposed test reaches it. A longer
fixture (or a generated multi-chunk source) and an exact-equality assertion across two plays, as the
probe does, would; so would a channel-count case that is not a power of two.

## Objection 3 (c): it collides with the parked surround rewire it is meant to unblock

The spec's housekeeping says direct mixer wiring "needs this fix first". Under direct wiring a
`UniformSourceIterator` re-bootstraps its channel and rate converters at every span boundary the
source reports. The wrapper's `None` makes `Buffered` cut a first chunk of 32 768 samples, which is
not a multiple of 3, 5, 6 or 7 channels; every later boundary inherits the offset. Measured, 5.1 Ogg,
looped, straight into `UniformSourceIterator` with 6 output channels, per-channel RMS 0.5-0.9 s in:

| Output rate | wrapped one-shot (reference) | wrapper loop (the spec) |
|---|---|---|
| 44 100 (no resampling) | 0.071, 0.142, 0.214, 0.000, 0.357, 0.428 | identical |
| 48 000 (resampled) | 0.071, 0.142, 0.213, 0.000, 0.356, 0.427 | **0.356, 0.427, 0.071, 0.142, 0.213, 0.000** |

That is a rotation by two channels from 0.12 s on, introduced by this fix, on exactly the wiring the
parked design moves to. (The reference is the wrapped one-shot because an unwrapped Ogg through a
bare `UniformSourceIterator` yields 0 samples: its `Some(0)` makes the first `Take` empty and the
re-bootstrap sees `Some(0)` again.) On today's `Player` path the effect is not visible: through the
queue the wrapper, `LoopedDecoder` and the one-shot all give the same levels (all equally rotated by
the already-parked `Player` rotation bug), so this is not a regression in v0.2.3, it is a trap for
the next piece of work.

The same housekeeping notes "rodio's next release may remove span lengths". The wrapper's only lever
is `current_span_len`; when that goes, the fix is either moot or a compile break, while the
user-visible failure it guards ("rings silently, prints fired, exit 0") is undetected by anything in
the app.

Cheap repairs, either of which keeps the design: report a frame-aligned length instead of `None`
before the first sample (`Some(32_768 / ch * ch)`; `extract` takes fewer if the file is shorter, and
an empty file still ends because `data.is_empty()` returns `End`), or prime the decoder at open (read
and hold one sample) so the decoder's own packet-length span is reported and Buffered spans stay
packet- and frame-aligned like flac's today. Independently, a check-time assertion that the source
as it will be played yields a first sample would catch the next decoder quirk too.

## Fixed check: paired facts and the claim a reader infers

1. **"The developer's 2026-10-27 hands-on check with an mp3 `-s` file passed; the red team found one
   of two ffmpeg-made mp3s reports a non-zero first span, so it depends on the encoder."**
   Pair: a passing mp3 + 1 of 2 ffmpeg mp3s non-zero. Inferred: the encoder decides.
   **Unsupported implicature**: both mp3s came from the same encoder, so the pair cannot separate
   encoders. Measured with one encoder (ffmpeg 4.4.1 libmp3lame) varying only the rate: 16 and
   22.05 kHz give `Some(0)`, 44.1 and 48 kHz give `Some(47)` (stereo `Some(94)`). The residual sums:
   LAME's delay is 576 + 529 = 1 105 samples and rodio enables gapless trimming by default; at 44.1
   and 48 kHz (MPEG-1, 1 152-sample frames) the first frame keeps 1 152 − 1 105 = 47 samples; at
   24 kHz and below (MPEG-2/2.5, 576-sample frames) the whole first frame is trimmed, hence 0. With
   gapless off every mp3's first span is the full frame (576 or 1 152); Vorbis stays `Some(0)` either
   way (Vorbis's first packet only primes the overlap). So the MP3 half of the bug is "MP3 at
   24 kHz or below", not "some encoders"; the release note's "some MP3 `-s` files" should say so,
   and the repo's 22.05 kHz fixture is the minority case, which is why the developer's real mp3
   passed.
2. **Approach C "rejected: … hundreds of MB" beside approach A "picked"**. Inferred: A avoids the
   hundreds of MB. **Unsupported implicature**: `Repeat` keeps the head of the `Buffered` chain, so
   A ends up caching the whole decoded file too (67.0 MiB for 3 minutes at 48 kHz stereo, decomposed
   above), lazily instead of up front; for `remind`'s 10-minute cap that is up to ~230 MB. The only
   real difference from C is start-up latency. (This is not new in v0.2.3: flac, wav and 44.1 kHz
   mp3 loops already do it; but it is not a reason to prefer A.) `LoopedDecoder` is the design that
   actually avoids it (0.7 MiB).
3. **"`Buffered::extract` takes up to 32 768 samples and returns its end marker when it gets none,
   so a truly empty file still ends"** under "Why `None` is safe". Inferred: `None` is otherwise
   neutral. **Partly unsupported**: true for the end-marker, but 32 768 is not frame-aligned for
   3/5/6/7 channels (124 of 125 off-frame starts for 5.1 Ogg; rotation under direct wiring,
   objection 3).
4. **Approach A "every later span reaches rodio as before"**. Inferred: later spans are as correct
   as before. **Unsupported**: the value passes through, but after a mid-packet first chunk it no
   longer marks a packet boundary (it is the full packet length, not the remainder), and on the
   `Once` path `TakeDuration` never re-reads it.
5. **"Every Vorbis file is expected to be hit (2 of 2 measured)."** A count used as a universal.
   Supported here by the codec (the first Vorbis packet produces no output) and by this probe's
   3 of 3 further Vorbis files; acceptable.
6. **Fixture table: "Played once 11 264 / 11 025" beside "`repeat_infinite().take(100 000)` 0"**.
   Inferred: looping, not decoding, is broken. Supported (reproduced exactly).
7. **"`ding -s x.ogg` plays: it does not loop, and … the `Player` queue falls back…"**. Inferred:
   the one-shot path needs no fix. Supported (E3: identical output with and without the wrapper,
   end detected). It also means the wrapper, placed in `sound::source` as specified, changes the
   ding path for no benefit; scoping it to the loop builders (`bound`'s `Loop` arm, `start_sound`)
   would leave `ding` byte-for-byte as shipped, though the parked direct wiring would then need its own
   first-span fix for `ding` (an unwrapped one-shot Ogg is silent there, measured).

Flags 1 and 2 are folded into the review rather than displacing an objection: flag 2 is part of
objection 1, and flag 1 changes the scope wording and release notes, not what to build.

## What I could not establish

- No audible check on real hardware; all levels are computed from samples pulled through rodio's
  own queue and converters.
- six.ogg has one near-silent channel (the encoder's channel mapping or LFE handling); the 5.1
  comparisons are therefore made between paths on the same file, never against the nominal levels.
