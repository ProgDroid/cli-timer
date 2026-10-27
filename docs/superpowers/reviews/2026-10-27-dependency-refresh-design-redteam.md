# Red team: v0.2.2 dependency refresh design

2026-10-27. Hostile review of `docs/superpowers/specs/2026-10-27-dependency-refresh-design.md`.
Evidence comes from the repo at `cec62cc`, `Cargo.lock`, crates.io API queries made today, and crate
sources: crossterm 0.25.0 and 0.28.1, rodio 0.16.0, cpal 0.18.1 and tui 0.19.0 from
`~/.cargo/registry`, plus rodio 0.23.0, ratatui 0.30.2, ratatui-core 0.1.1, ratatui-crossterm 0.1.1
and crossterm 0.29.0 downloaded into the session scratchpad. **Verified** means I read it in a
source, the lock or the API. **Suspected** means inferred and not run.

## Objection 1 (b, assumed requirement): the crossterm bump is treated as API churn, but it changes key handling on Windows, and nothing in the plan would see it

The table describes crossterm 0.25 -> 0.29 as "follows ratatui". The Goals promise the same "keys".
The plan assumes that a change which compiles, passes clippy and passes tests behaves the same.
For this crate that assumption is false.

**Verified:**
- crossterm 0.25.0 `src/event/sys/windows/parse.rs:238`: `if !key_event.key_down { return None; }`.
  Key-up events are dropped on Windows, so each key press reaches the app once.
- crossterm 0.26.0 CHANGELOG, Breaking: "Add key release event for windows. (#745)". In crossterm
  0.29.0, `src/event/sys/windows/parse.rs:286-289` emits a second `KeyEvent` with
  `KeyEventKind::Release` for every key-up.
- `src/event.rs` forwards every `CrosstermEvent::Key(e)` whatever its `kind`. `src/handler.rs`
  matches only on `key_event.code` and never reads `kind`.
- ratatui 0.30.2's own examples filter on `key.kind == KeyEventKind::Press` (`src/lib.rs:280`,
  `src/init.rs:97,149`). That filter exists because of this behaviour.

**What follows on Windows** (worked out from `handler.rs`, not run). Each key now arrives as Press
and then Release:
- `r` while running: Press goes to `Restart`, then Release calls `restart()`. The "Press again to
  confirm" prompt is skipped and the timer restarts.
- Space while running: Press pauses, then Release resumes. Pause stops working.
- Esc or `q` on the restart prompt: Press goes back to `Running`, then Release hits `Esc` in
  `Running` and sets `running = false`. Cancelling the prompt quits the timer.

Windows is the developer's platform, and the only one where sound and pop-ups get checked by hand
(README lines 91-92).

**Why the plan misses it:**
- `src/app.rs`, `src/event.rs`, `src/handler.rs`, `src/tui.rs` and `src/main.rs` contain 0
  `#[test]`. The repo has 164 `#[test]` attributes; NEXT.md reports 163 on Windows.
- "Green after each" is therefore a compile check for the ratatui + crossterm commit.
- The hands-on check opens the full-screen timer but never presses Space, `r` or Esc.
- The tests-first oracle discipline goes to `sound.rs`, which already has 16 tests. The screen and
  keys get nothing, yet that half contains the one verified behaviour change.

**Cheaper and stronger:** apply the same "oracle that predates it" rule to the screen.
- Before the swap, while still on tui 0.19 (it has `backend::TestBackend` and `assert_buffer`),
  write `TestBackend` buffer tests for each `State`.
- Write `handle_key_events` tests that feed a Press then a Release and assert a single transition.
- Filter to `KeyEventKind::Press` in `event.rs`.
- Add "press Space, `r`, `r`, Esc" to the hands-on check.

A related **suspected** layout risk: ratatui-core 0.1.1 `layout/flex.rs:97` makes `Flex::Start` the
default. tui 0.19 behaved like what ratatui now calls `Flex::Legacy` ("excess space into the
last"). Rounding of `Percentage(49)` may move the time row. No test would catch that either; the
`TestBackend` snapshot would.

## Objection 2 (c, the 6-month collision; also a): rodio 0.23.0 is a stopgap its maintainers have already announced they will replace

**Verified:** the rodio 0.23.0 CHANGELOG (released 2026-10-11, 16 days ago) says:

> "A minimal release for users blocked on old cpal version while we are working on a bigger release."

Its only change is breaking: cpal error types became `cpal::Error`, and `rodio::DevicesError` was
removed. Before it:
- 0.21 replaced `OutputStream::try_default`/`OutputStreamHandle` with a builder, moved sources to
  `f32`, and renamed `Sink::try_new` to `connect_new`.
- 0.22 renamed `OutputStream` to `MixerDeviceSink`, `Sink` to `Player` and `StreamError` to
  `OsSinkError`, and made `SamplesBuffer::new` take `NonZero` channel and rate values.

The spec targets "current" for rodio, which means migrating `sound.rs` and the alarm thread in
`app.rs` across three breaking redesigns to land on a release its authors call interim. The "bigger
release" will make the developer do the `play` and `start_sound` rewrite again, probably within the
6-month window. When it does, the anti-goals (stderr silence, loudness, fade) need re-checking by
hand.

Nothing in the spec says rodio 0.16 is broken today:
- The parking-lot entry gives no defect: "Upgrade `rodio` (and re-check sound decoding of real
  mp3/flac/ogg files, which no test covers)".
- The Intent's case is "seven releases behind" (see Fixed check F3).

**The 80 percent design:**
- Do the parts that are cheap or fix a real problem: `tui` -> `ratatui` with the Press filter,
  crossterm, the `clap`/`chrono` lock updates.
- Land the real-format fixture tests now, on rodio 0.16. They are the valuable half of the rodio
  parking-lot entry, and they become the oracle for whichever rodio comes next.
- Re-park "Upgrade rodio" with a trigger, "when rodio's post-0.23 release lands", so the sound path
  is migrated once rather than twice. This is the spec's rejected Approach C, minus the second
  release round, which only happens when rodio gives a reason.

The spec rejects C as "two hands-on checks". Upgrading to 0.23 now produces the same second check
later, plus a second migration.

A second, smaller simplification (**verified**): `rand` is used once, `random_color` in
`src/app.rs`, which calls `thread_rng().gen_range(0..15)`. rand 0.8 is still maintained (0.8.8
released 2026-08-25), so the 0.10 upgrade buys nothing. Choosing one of 15 colours does not need a
CSPRNG crate: `std::hash::RandomState` + `BuildHasher::hash_one`, or the clock's sub-second nanos,
removes the dependency and every future `rand` rename.

## Objection 3 (b, assumed requirement): "nothing changes" assumes today's decodable formats are mp3/flac/ogg/wav; today's set is wider, and the spec treats the rodio features as a naming problem

**Verified:**
- rodio 0.16.0 depends on `symphonia = "0.5"` **with default features** (its Cargo.toml has no
  `default-features = false`).
- `cargo tree --offline -e features -i symphonia-format-mkv` (and the same for
  `symphonia-codec-adpcm`) shows both activated today through `symphonia feature "default"` <- rodio
  0.16.0.
- rodio 0.16's decoder probes with `symphonia::default::get_probe()` / `get_codecs()`
  (`src/decoder/symphonia.rs:13,65,72`), so every compiled-in format is accepted. Today `-s` takes
  WAV (PCM and ADPCM), FLAC, MP3, Ogg Vorbis and MKV/WebM.
- rodio 0.17.1 CHANGELOG: "Disable `symphonia`'s default features". rodio 0.23.0's Cargo.toml has
  `[dependencies.symphonia] default-features = false`, and maps each feature narrowly:
  - `symphonia-vorbis = ["symphonia/vorbis"]`: the codec only. The Ogg container is
    `symphonia-ogg`.
  - `symphonia-wav = ["symphonia/wav"]`: no PCM codec. The `wav` feature adds `symphonia-pcm`.
  - MKV and ADPCM are separate opt-ins, `symphonia-mkv` and `symphonia-adpcm` (rodio 0.21
    CHANGELOG: "Previously, the ADPCM decoder was enabled when `symphonia-wav` was").
  - `playback` (cpal) is optional too, and is needed for any output.

So keeping today's four feature names under rodio 0.23 removes PCM WAV and Ogg; the existing WAV
tests and the planned ogg fixture would catch those. It also removes MKV/WebM and ADPCM WAV, which
**no planned test covers**: the fixtures are mp3, flac and ogg only. A user's `-s alarm.webm` would
go from ringing to "cannot decode", which the spec's own anti-goals count as a visible difference.
The anti-goal frames this as "whatever the 0.23 feature names are". The real decision is which
formats are supported, and the spec never states it.

The fixture assertions are also weaker than "an oracle that predates it" suggests. Channel count,
sample rate and approximate duration are set by the `ffmpeg` command, not by rodio 0.16. Passing on
0.16 proves the files decode, which is all an oracle can prove here; it does not cover loudness or
timbre. Either name the supported set (and add mkv and ADPCM fixtures, or accept dropping them and
say so in the release notes), or turn on `symphonia-all` and test a representative subset.

A side note (**verified**): in rodio 0.23 symphonia is the default mp3 path and minimp3 is an
opt-in feature (`minimp3 = ["dep:minimp3_fixed"]`). The anti-goal about "going back to a decoder
that aborts" now holds unless someone turns that feature on, so the `Cargo.toml` comment needs
rewriting rather than preserving.

## Lesser points (not in the top three)

- **Untested 500 ms fade-in** (verified): `start_sound` in `src/app.rs` applies
  `.fade_in(500 ms)`, and no test covers it. rodio 0.20 "refactored `fade_in` to use the
  `linear_gain_ramp`". The anti-goal "no different fade" has no oracle.
- **Stderr on drop** (verified): rodio 0.23 `src/stream.rs:85-89` prints with `eprintln!` unless
  `log_on_drop(false)` is set or the `tracing` feature is on. The spec already handles this; the
  call is needed both in `sound::play` and in the alarm thread in `app.rs`.
- **Deprecation as an error** (verified): `Frame::size` is `#[deprecated = "use area() instead"]`
  (ratatui-core 0.1.1 `terminal/frame.rs:79`), so `-D warnings` forces the rename. That is fine, but
  it is a source change in `app.rs` that the table does not list.

## Fixed check: paired facts and the claims a reader draws from them

**F1. Table + "Already current: serde, serde_json, notify-rust, and the toolchain."**
- Inferred claim: the two lists together cover every dependency, and after the change nothing is
  behind.
- The 10 direct dependencies in Cargo.toml break down as:
  - 6 in the table: tui, crossterm, rodio, rand, clap, chrono;
  - 3 listed as current: serde, serde_json, notify-rust;
  - 1 unlisted: the direct `thiserror` 2.0.21, which is current (crates.io max_stable 2.0.21).
  - 6 + 3 + 1 = 10.
- The table's 7th row, thiserror 1.x, is transitive. The lock holds 286 packages, and the spec
  speaks for one of them.
- **Unsupported implicature** at the transitive level: rodio 0.23.0 requires `symphonia ^0.5.5`
  while symphonia is at 0.6.1. "Bring every dependency up to its current release" (Intent) is true
  for direct dependencies only.

**F2. "Every target's `rust-version` is 1.88 or lower, so the pinned toolchain stays."**
- Both facts check out: ratatui 0.30.2 1.88.0, rodio 0.23.0 1.87, rand 0.10.3 1.85, clap 4.6.7
  1.85, chrono 0.4.45 1.62.0, crossterm 0.29.0 1.63.0, and toolchain 1.99. The conclusion follows.
- Inferred claim: the build needs only Rust 1.88. That is unsupported. notify-rust 4.18.2, named
  in the sentence before, declares 1.89.0, and the README says 1.91 or later. Harmless with 1.99
  pinned, but the number is a floor for the targets only.

**F3. "a sound crate seven releases behind (`rodio` 0.16)", next to "`tui`, last release 2022".**
- The count: 14 non-yanked releases after 0.16.0, of which 7 are minor lines (0.17 through 0.23)
  and 7 are patches (0.17.1-0.17.3, 0.18.1, 0.20.1, 0.21.1, 0.22.1-0.22.2). 0.20.0 is yanked.
  7 + 7 = 14.
- Inferred claim: rodio 0.16 is stale in the same way tui is, so it needs replacing.
- **Unsupported implicature.** Neither fact shows a defect in 0.16, and the newest release
  describes itself as interim. This supports Objection 2.

**F4. thiserror 1.x row: "in the lock | gone | an old transitive copy; should drop out".**
- Inferred claim: upgrading removes thiserror 1.x from the lock.
- **Unsupported, and the evidence points the other way.** Today thiserror 1.0.38 is pulled by 5
  packages: cpal 0.14.2, jni 0.19.0, ndk 0.6.0, ndk 0.7.0, and proc-macro-crate 1.2.1 (via
  num_enum_derive 0.5.7).
- cpal 0.18.2 depends on `ndk ^0.9` for Android. ndk 0.9.0 is the latest release and requires
  `thiserror ^1.0.23`, non-optional. Cargo.lock resolves all targets: today's lock already carries
  the Android-only `ndk` and `jni`.
- thiserror 1.x will stay in the lock (**suspected**, not run). If a builder or reviewer treats
  "gone" as an expected outcome, they will chase a non-goal.

**F5. "Already current: ... the toolchain (`1.99`, stable is 1.99.1)."**
- Inferred claim: the build runs on 1.99.0.
- Consistent: the `channel = "1.99"` pin floats to the newest 1.99.x patch under rustup. Not
  flagged.
