# Silent looped Ogg/MP3 fix: design

Date: 2026-10-28. Size: a small change. Bead: `clitimer-4xc` (P1 bug). Ships as v0.2.3.

Found by the red team of the surround rewire design
(`docs/superpowers/reviews/2026-10-28-hardening-batch-design-redteam-2.md`), confirmed by the agent
with its own probe. That design is parked (its spec, `2026-10-28-hardening-batch-design.md`, and
`docs/parking-lot.md`). This design's own red team
(`docs/superpowers/reviews/2026-10-28-silent-loop-fix-design-redteam.md`) replaced the first
approach (a span-correcting wrapper) with rodio's looping decoder; the developer agreed.

## The bug

rodio 0.23's symphonia decoder keeps whatever its first packet decodes to
(`decoder/symphonia.rs:114-140`). A Vorbis stream's first packet decodes to no samples by design.
An MP3 at 24 kHz or below loses its whole first frame to gapless trimming (LAME's 1 105-sample
delay against a 576-sample MPEG-2/2.5 frame; a 44.1/48 kHz MPEG-1 frame of 1 152 keeps 47). Either
way the decoder reports `current_span_len() == Some(0)` before any sample has been read. rodio's `Buffered` (`source/buffered.rs`, `extract`), which
`repeat_infinite()` uses, reads `Some(0)` as the end of the stream. Looping such a file therefore
yields no samples at all:

- `remind -s x.ogg` skips the ring: when the time is up the sound "plays" for about 70 ms of
  nothing (measured: a 3 s ring returned `Ok` after 68 ms), then it prints "fired", exit 0.
- The full-screen timer with `-s x.ogg` stays silent until a key is pressed.
- `ding -s x.ogg` plays: it does not loop, and rodio's `Player` queue falls back to the size hint
  or a 512-sample threshold when a source reports `Some(0)` (`queue.rs:137-168`).

Measured 2026-10-28 (the agent's scratch probe on the repo's `Cargo.lock`, rodio's real decoders):

| Fixture | First span | Played once | `repeat_infinite().take(100 000)` |
|---|---|---|---|
| `tests/fixtures/tone.flac` | Some(2304) | 11 025 samples | 100 000 |
| `tests/fixtures/tone.ogg` | Some(0) | 11 264 samples | 0 |
| `tests/fixtures/tone.mp3` | Some(0) | 11 025 samples | 0 |

Sample rate decides for MP3, not the encoder: with one encoder (ffmpeg 4.4.1 libmp3lame), 3 of 6
red-team mp3s were hit, exactly the three at 22.05 kHz or below (`tone.mp3` is 22.05 kHz). The
developer's passing 2026-10-27 check most likely used a 44.1 kHz mp3. Every Vorbis file is expected
to be hit. The existing fixture tests decode each file once, so they could not see it.

A smaller cost of today's loop: `Buffered` keeps what has played so far, up to the whole decoded
file, in memory: min(ring, file length) x rate x channels x 4 bytes. For a typical 2-5 s alarm
sound that is under 2 MiB; it only grows large for long files with long rings (a 3-minute 48 kHz
stereo file costs 66 MiB once it has played through).

## The change

**Loop `-s` files with rodio's own looping decoder** (`Decoder::builder()…build_looped()`,
`decoder/mod.rs:86-106`, `606-676`), which seeks the file back to the start and decodes again
when it reaches the end, instead of `repeat_infinite()`. It never goes through `Buffered`, and it
holds no decoded copy of the file (0.7 MiB measured for a 3-minute 48 kHz stereo Ogg).

- A new function in `src/sound.rs` builds the looping source for a `Sound`: a file gets the
  looping decoder; the built-in alarm and chime (`SamplesBuffer`) keep `repeat_infinite()`, which
  is correct for them.
- Both loop sites call it: `bound` for `Playback::Loop` (`remind`) and `start_sound` in
  `src/app.rs` (the full-screen alarm, which adds its 500 ms fade-in on top).
- **Order of work, so the tests can go red on today's code:** first add the function as a pure
  refactor (`source(sound)?.repeat_infinite()` for every sound), point both loop sites at it, and
  write the tests against it: the Ogg and MP3 tests fail. Then switch the file branch to the
  looping decoder: they pass. The red and green tests are the same code.
- Build it the way `Decoder::try_from(File)` builds a plain decoder (`decoder/mod.rs:280-295`):
  `BufReader` over the file, `with_byte_len(len)`, `with_seekable(true)`. Without those, seeking
  may be slower or unavailable.
- One-shot playback (`ding`, `Playback::Once`) is unchanged.
- The `-s` check before the timer starts (`check`) is unchanged; a file that decodes once also
  loops.

Known limits, accepted:
- If seeking fails mid-ring (an I/O error), the looping decoder stops, so the ring ends early in
  silence. Local files seek.
- The looping decoder still reports `Some(0)` before its first sample. On the `Player` path
  cli-timer uses, the queue covers it (`queue.rs:150-168`; measured: 287 854 non-zero samples for
  looped `tone.ogg`). Handing sounds to the mixer directly, as the parked surround design wanted,
  makes it silent (measured: 0). A tripwire test pins this (below), and the parking-lot entry says
  so.
- Each loop decodes the file again. The red team found the seam sample-identical to today's at
  the device's own rate; at a resampled rate the output differs from today's (not worse, no new
  click).

## Approaches

| Approach | Verdict |
|---|---|
| **rodio's looping decoder for files** | Picked (after the red team): no custom `Source`, no span lie, no decoded copy in memory. |
| A span-correcting wrapper with `repeat_infinite()` | First pick, replaced: works, but keeps the `Buffered` cache of everything played, and its `None` span makes `Buffered` cut a 32 768-sample first chunk (mid-frame on 5.1, which matters if the surround design is unparked). |
| Decode the whole file into memory at start | Rejected: seconds of decoding before the timer starts for a long file, and the same memory cost. |

## Tests (the oracle)

Unit tests in `src/sound.rs` (end-to-end tests run with `CLI_TIMER_QUIET=1` and never play sound).
Sound-or-silence assertions read the sound **through rodio's real playback path with no device**
(`rodio::mixer::mixer`, `Player::connect_new`, `fit_channels`, then the mixer's output), because
`Player`'s queue handling of the first span is what decides between sound and silence. Reading the
source raw would pass on a silent build. (Revised after the plan red team: an earlier draft used a
hand-built `queue(false)` model of `Player`, which a rodio upgrade could leave green while the app
went silent.) The full-screen alarm's sound, fade-in included, is built in `sound.rs` so it is
tested too; its assertions skip the fade. No fade-in in the compared samples: the fixtures are 0.5 s, the fade is 500 ms.

**Red first** (fail after the refactor step, pass after the switch):
- For `tone.ogg` and `tone.mp3`, the new looping function's output, through the player, has sound
  in each of at least three file-length windows (thresholds measured on the fixtures and pinned), and play 3 equals play 2 sample for sample at the
  file's own rate. (Play 1 may differ from later plays, for example an mp3's priming.)

**Controls** (pass before and after; they prove the tests can see sound and a seam):
- `tone.flac` (first span `Some(2304)`): the same assertions.
- One play of each fixture is unchanged (the existing tests).

**Tripwire** (passes now, meant to fail the day rodio changes):
- Looped and one-shot `tone.ogg` and `tone.mp3`, fed straight into a `UniformSourceIterator`
  without `Player`, yield no samples. Its comment says why, and that the day it fails, direct mixer wiring (the parked surround
  design) has become possible.

No new fixture: the chosen design has no 32 768-sample chunking, and `tone.ogg` already spans about
22 packets and a seam.

The builder runs the red tests after the refactor step and records that they failed.

## Non-goals

- No change to how sounds reach the output (`Player` stays; the surround rewire is parked).
- No new formats; no change to which files `-s` accepts.
- No change to one-shot playback.

## Left to the builder

- The new function's name and signature, and how `bound` gets at it (it takes a `Sound` today only
  through its caller).
- How the tests define "has sound" in a window (a peak or energy threshold), measured on the
  fixtures and pinned.

## Release

v0.2.3: bump `version` in `Cargo.toml` and `plugin/.claude-plugin/plugin.json`, release notes that
name the fix: in 0.2.2 a looped `-s` sound (`remind`, the full-screen alarm) was silent for Ogg
Vorbis files and for MP3s at 24 kHz or below (`remind` skipped its ring and reported "fired" at
once; the full-screen alarm made no sound); `ding` was not
affected. A secondary line may say looped files are no longer kept in memory as they play (it
matters only for long files with long rings). Tagging follows the usual dry-run-first routine in `NEXT.md`, after the developer says
to release.

## Housekeeping

- `docs/parking-lot.md`: a "Surround output rewire" entry pointing at the parked spec and its two
  red-team reviews: the `Player` rotation bug, that direct mixer wiring would silence the looping
  decoder and one-shot Ogg (both report `Some(0)` first), and that rodio's next release may remove
  span lengths. Fold the existing "Surround channel-fill span length" entry into it. In the
  "Leading `-`" entry, record that `-l=-x` and `-l-x` already work and why the change was declined.
  In the "Release archive names" entry, record the PATH and stable-URL costs.
- `NEXT.md`: the "alarm plays from every speaker" check cannot see the rotation (mono sound); say
  so, and that stereo `-s` files on surround are known wrong (parked).
