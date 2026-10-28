# Silent Looped Ogg/MP3 Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A looped `-s` sound (`remind`, the full-screen alarm) plays for Ogg Vorbis files and MP3s at 24 kHz or below, instead of ringing silently.

**Architecture:** Two new functions in `src/sound.rs` build every looping sound: `looped` (endless, used by `remind` through `bound`) and `alarm_loop` (`looped` plus the full-screen 500 ms fade-in, used by `start_sound`). First they are a pure refactor (`repeat_infinite()` for every sound) so the new tests are seen failing on today's behaviour; then `looped`'s file branch switches to rodio's own looping decoder (`build_looped`), which seeks the file back to the start instead of going through `Buffered`. Built-in sounds keep `repeat_infinite()`. Playback still goes through `Player`.

**Tech Stack:** Rust (toolchain pinned in `rust-toolchain.toml`), rodio 0.23.0 (symphonia decoders), ratatui 0.30.

**Spec:** `docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md` (bead `clitimer-4xc`). Plan red team: `docs/superpowers/reviews/2026-10-28-silent-loop-fix-plan-redteam.md`.

## Global Constraints

- `Player` stays on every playback path; no direct `mixer.add`.
- One-shot playback (`Playback::Once`, `ding`) is unchanged.
- `check` is unchanged: a file that decodes once also loops.
- Build the looping decoder the way `Decoder::try_from(File)` does (`rodio-0.23.0/src/decoder/mod.rs:280-295`): `BufReader` over the file, `with_byte_len(len)`, `with_seekable(true)`.
- Errors keep their wording: a file that cannot be opened is `SoundError::Open`; one that cannot be decoded is `SoundError::Decode`, both naming the path. A failed `metadata()` stays a `Decode` error carrying `DecoderError::IoError(e.to_string())`, as `Decoder::try_from` reports it today.
- No new fixture files.
- Sound-or-silence assertions read the sound **through rodio's real playback path with no device**: `rodio::mixer::mixer(ch, rate)`, `Player::connect_new(&mixer)`, `player.append(fit_channels(src, ch))`, then samples from the mixer's output. Never through a hand-built model of `Player`.
- Gates after every task: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Exception: Task 1 ends with its new tests failing by design; fmt and clippy must still be clean. Do not push the branch until Task 2 is green (CI runs on every push).
- The commit that finishes the bead carries `clitimer-4xc` alone on a line.

## Review Focus

1. A looped file at a rate other than the output's (a 22.05 kHz file on a 48 kHz device): sound after every seam. Test: Task 2, `looped_files_keep_sounding_when_resampled`.
2. A looped WAV (a format the bug does not hit): loops exactly as before. Test: Task 2, `a_looped_wav_repeats_exactly` (control).
3. A looped file shorter than the ring (`remind --ring`): the ring lasts the requested time. Test: Task 1, `a_looped_file_rings_for_the_requested_time`.
4. One-shot playback of the bug's files (`ding -s x.ogg`): still plays. Test: Task 1, `one_play_of_each_fixture_through_the_player_has_sound` (control).
5. A real device: `remind 3s -s tests/fixtures/tone.ogg --ring 5s` and `cli-timer -t 3s -s tests/fixtures/tone.ogg` ring audibly, and `--json` prints only JSON lines. Not testable without a device; it goes in `NEXT.md`'s "Try this" at hand-off.

---

### Task 0: commit the design documents

On a new branch `silent-loop-fix` from `main`, commit (docs only, nothing else):
`docs/superpowers/specs/2026-10-28-hardening-batch-design.md`, `docs/superpowers/specs/2026-10-28-silent-loop-fix-design.md`, this plan, and `docs/superpowers/reviews/2026-10-28-*` (the five red-team reviews), as `docs: silent-loop fix design, plan and red-team reviews; surround rewire parked`. Do it before creating any worktree, so a worktree sees them.

---

### Task 1: `looped` and `alarm_loop` as a refactor, with the tests (red)

**Files:**
- Modify: `src/sound.rs` (`bound`, `play`, new `looped` and `alarm_loop`, tests module)
- Modify: `src/app.rs:208-210` (`start_sound`)

**Interfaces:**
- Produces: `pub fn looped(sound: &Sound) -> Result<Box<dyn Source + Send>, SoundError>`: the endless looping source of `sound` (no fade, no time limit).
- Produces: `pub fn alarm_loop(sound: &Sound) -> Result<Box<dyn Source + Send>, SoundError>`: `looped(sound)?` with `fade_in(ALARM_FADE_IN)`, `const ALARM_FADE_IN: Duration = Duration::from_millis(500)` in `sound.rs`. `start_sound` uses it; nothing else builds the full-screen sound.
- Produces: `fn bound(sound: &Sound, playback: Playback) -> Result<Box<dyn Source + Send>, SoundError>` (was `bound(source, playback)`): `Once { max }` is `source(sound)?.take_duration(max)`, `Loop { for_ }` is `looped(sound)?.take_duration(for_)`. `play` calls `bound(sound, playback)?`.
- Test helpers (tests module):
  - `fn through_player(src: Box<dyn Source + Send>, rate: u32, n: usize) -> Vec<f32>`: a 1-channel `rodio::mixer::mixer` at `rate` (the fixtures are mono), `Player::connect_new(&mixer)`, `player.append(fit_channels(src, MONO))`, then `n` samples from the mixer output. Keep `player` alive until collected. The output never ends (the player keeps it alive), so tests assert on sound, not length.
  - `fn loud(samples: &[f32]) -> usize`: count of samples with `abs() > PEAK`, `const PEAK: f32 = 0.05` (the fixtures are a lavfi sine at amplitude 0.125).
  - `fn one_play(name: &str) -> usize`: `source(&Sound::File(fixture(name))).unwrap().count()`.

- [ ] **Step 1: Write the tests**

Update `once_stops_at_max_or_at_the_end_of_the_sound` and `loop_repeats_the_cycle_for_the_requested_time_then_stops` to call `bound(&Sound::Alarm, ..).unwrap()`; assertions unchanged. Add:

```rust
#[test]
fn looped_files_sound_every_time_round_through_the_player() {
    for name in ["tone.ogg", "tone.mp3", "tone.flac"] { // flac is the control
        let p = one_play(name);
        let out = through_player(looped(&Sound::File(fixture(name))).unwrap(), 22_050, 3 * p);
        for w in 0..3 {
            // measured: about 0.72 p loud samples per play (plan red team 2)
            assert!(loud(&out[w * p..(w + 1) * p]) > p * 6 / 10, "{name}: play {} quiet", w + 1);
        }
    }
}

#[test]
fn looped_files_repeat_exactly() {
    for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
        let p = one_play(name);
        let out: Vec<f32> = looped(&Sound::File(fixture(name))).unwrap().take(3 * p).collect();
        assert_eq!(out.len(), 3 * p, "{name}");
        assert_eq!(out[p..2 * p], out[2 * p..], "{name}: play 3 differs from play 2");
    }
}

#[test]
fn the_full_screen_alarm_sounds_after_its_fade_in() {
    for name in ["tone.ogg", "tone.mp3"] {
        let p = one_play(name);
        let out = through_player(alarm_loop(&Sound::File(fixture(name))).unwrap(), 22_050, 3 * p);
        assert!(loud(&out[p..]) > p * 12 / 10, "{name}: quiet after the fade-in"); // 2 plays at ~0.72 p each
    }
}

#[test]
fn a_looped_file_rings_for_the_requested_time() {
    for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
        let ring = bound(&Sound::File(fixture(name)), Playback::Loop { for_: Duration::from_secs(2) }).unwrap();
        // TakeDuration truncates the per-sample nanoseconds, so its error grows with ring time,
        // rate and channels; 2 s of 22.05 kHz mono lands exactly on 44 100.
        assert_eq!(ring.count(), 44_100, "{name}");
    }
}

#[test]
fn one_play_of_each_fixture_through_the_player_has_sound() {
    // control for `ding`: one-shot plays already work through the player
    for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
        let p = one_play(name);
        let out = through_player(source(&Sound::File(fixture(name))).unwrap(), 22_050, p);
        assert!(loud(&out) > p * 6 / 10, "{name}");
    }
}

#[test]
fn ogg_and_low_rate_mp3_need_rodios_player_to_be_heard() {
    // Tripwire. rodio 0.23's decoders report an empty first span for Vorbis and for MP3 at
    // 24 kHz or below, and only `Player`'s queue reads past it: fed straight to rodio's
    // converter (what direct mixer wiring does) these yield no samples at all. While this
    // holds, sounds must go through `Player`. The day it fails, the parked surround rewire
    // (docs/parking-lot.md) may be possible.
    let rate = SampleRate::new(22_050).unwrap();
    let cases: [(&str, Box<dyn Source + Send>); 4] = [
        ("looped tone.ogg", looped(&Sound::File(fixture("tone.ogg"))).unwrap()),
        ("looped tone.mp3", looped(&Sound::File(fixture("tone.mp3"))).unwrap()),
        ("one tone.ogg", source(&Sound::File(fixture("tone.ogg"))).unwrap()),
        ("one tone.mp3", source(&Sound::File(fixture("tone.mp3"))).unwrap()),
    ];
    // No samples at all, not merely quiet ones: the converter ends at once. Every case is
    // checked before failing, so one change does not hide the others.
    let produced: Vec<(&str, usize)> = cases
        .into_iter()
        .map(|(what, src)| (what, UniformSourceIterator::new(src, MONO, rate).take(30_000).count()))
        .filter(|&(_, n)| n > 0)
        .collect();
    assert!(produced.is_empty(), "now produce samples: {produced:?}");
}
```

- [ ] **Step 2: Implement `looped` as `Ok(Box::new(source(sound)?.repeat_infinite()))`, `alarm_loop`, the new `bound`, and point `start_sound` at `sound::alarm_loop(&self.sound)?`**

Doc comments say what each returns (`looped`: endless, no fade, no limit; `alarm_loop`: the full-screen alarm's sound). In `src/app.rs`, `Source` in `use rodio::{Player, Source}` (line 9) becomes unused once the fade-in moves; drop it (clippy `-D warnings` fails otherwise). `StdDuration` stays: it is still used at `src/app.rs:85`.

- [ ] **Step 3: Run the tests; record the red**

Run: `cargo test --lib sound::tests`
Expected FAIL, on `tone.ogg` (the first case) in each: `looped_files_sound_every_time_round_through_the_player`, `looped_files_repeat_exactly` (`out.len()` 0), `the_full_screen_alarm_sounds_after_its_fade_in`, `a_looped_file_rings_for_the_requested_time` (count 0). Then, with `tone.ogg` temporarily removed from each list, the same four fail on `tone.mp3`; restore the lists. Everything else passes, including the tripwire and the one-play control. Put the failure lines in the commit body and the task report.

- [ ] **Step 4: Commit the red state**

`cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean. Commit `src/sound.rs src/app.rs` as `refactor(sound): build every looped sound in sound.rs; tests show Ogg/MP3 loops silent`. Do not push.

---

### Task 2: loop files with rodio's looping decoder (green)

**Files:**
- Modify: `src/sound.rs` (`looped`'s file branch; tests module)

**Interfaces:**
- Consumes: `looped`, `bound`, `through_player`, `loud`, `one_play`, `fixture`, `write_wav` from Task 1 and the existing tests.
- Produces: `looped(&Sound::File(path))` returns rodio's `LoopedDecoder` built per Global Constraints; built-ins unchanged.

- [ ] **Step 1: Write the Review Focus tests**

```rust
#[test]
fn looped_files_keep_sounding_when_resampled() {
    for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
        let p = one_play(name) * 48_000 / 22_050; // one play at the output rate, a few samples over
        let out = through_player(looped(&Sound::File(fixture(name))).unwrap(), 48_000, 3 * p);
        for seam in 1..3 {
            // 100 ms just after each seam has sound
            // measured: about 3 500 loud samples per window
            assert!(loud(&out[seam * p..seam * p + 4_800]) > 2_500, "{name}: quiet after seam {seam}");
        }
    }
}

#[test]
fn a_looped_wav_repeats_exactly() {
    let path = write_wav("loop.wav", 4_000); // 0.5 s at 8 kHz
    let out: Vec<f32> = looped(&Sound::File(path.clone())).unwrap().take(12_000).collect();
    let once: Vec<f32> = source(&Sound::File(path.clone())).unwrap().collect();
    std::fs::remove_file(path).unwrap();
    assert_eq!(out[..4_000], once[..]);
    assert_eq!(out[4_000..8_000], once[..]);
    assert_eq!(out[8_000..], once[..]);
}
```

Run: `cargo test --lib sound::tests`. Expected: `looped_files_keep_sounding_when_resampled` fails on `tone.ogg`; `a_looped_wav_repeats_exactly` passes (control).

- [ ] **Step 2: Switch `looped`'s file branch to `Decoder::builder()…build_looped()`**

Open the file as `source` does (`SoundError::Open`), read its length (a failed `metadata()` is `SoundError::Decode` with `DecoderError::IoError`, per Global Constraints), then `Decoder::builder().with_data(BufReader::new(file)).with_byte_len(len).with_seekable(true).build_looped()`, mapping its error to `SoundError::Decode`. Share the open-and-map code with `source` rather than copying it. Built-ins keep `repeat_infinite()`. `looped`'s doc comment says why files use the looping decoder (an empty first span ends `repeat_infinite()`'s `Buffered`; it also keeps no decoded copy of the file).

- [ ] **Step 3: Run the full gates**

Run: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
Expected: all pass, the tripwire included. Count: 196 today + 6 (Task 1) + 2 (Task 2) = 204 on Windows (171 unit + 33 end-to-end); 205 on Linux/macOS (one `cfg(unix)` test).

- [ ] **Step 4: Commit**

`fix(sound): loop sound files with rodio's looping decoder`, body naming the cause in one line and `clitimer-4xc` alone on the last line.

---

### Task 3: version, parking lot, bead

**Files:**
- Modify: `Cargo.toml`, `plugin/.claude-plugin/plugin.json` (`version` 0.2.2 → 0.2.3), `Cargo.lock` (via `cargo build`)
- Modify: `docs/parking-lot.md`

- [ ] **Step 1: Bump the version to 0.2.3 in both files; run `cargo test`** (`plugin_version_matches_crate` must pass).

- [ ] **Step 2: Parking lot**, per the spec's Housekeeping:
  - New entry "Surround output rewire" (2026-10-28, hardening-batch red teams): `Player::connect_new` before `append` makes rodio convert the first 512 samples as mono, so on 5.1/7.1 every later frame is rotated two channels (a stereo `-s` file plays left only from back-left; mono built-ins look fine). A fix needs sounds handed to the mixer directly, which today silences Ogg and low-rate MP3, looped and one-shot alike (pinned by `ogg_and_low_rate_mp3_need_rodios_player_to_be_heard`). rodio's next release may remove span lengths. Pointers: `superpowers/specs/2026-10-28-hardening-batch-design.md` and both `superpowers/reviews/2026-10-28-hardening-batch-design-redteam*.md`. Fold the "Surround channel-fill span length" entry into it.
  - "Leading `-`" entry: `-l=-x` and `-l-x` already work for the full-screen `-l`; declined 2026-10-28 because `allow_hyphen_values` would turn `-t 1m -l --help` into a timer labelled "--help"; `-s ./-x.mp3` covers `-s`.
  - "Release archive names" entry: declined 2026-10-28: a versioned folder inside the archive leaves an old PATH entry running the old binary, and ends fixed `releases/latest/download/<name>` links.
  - "rodio's next big release" entry: looped files now use `LoopedDecoder`; the tripwire test says when direct mixer wiring becomes safe; the player-path test helper (`through_player`) is the oracle for the move.

- [ ] **Step 3: Commit** `Cargo.toml Cargo.lock plugin/.claude-plugin/plugin.json docs/parking-lot.md` as `chore: v0.2.3; park the surround rewire and the declined batch items`.

- [ ] **Step 4: Close the bead** after the branch is merged: `bd close clitimer-4xc --reason="looped -s files use rodio's LoopedDecoder (v0.2.3)"`, in the main checkout.

`NEXT.md` is not touched on the branch: it is updated in the main checkout at hand-off, after the merge (Review Focus item 5, release notes still to write, the surround note). Release notes are written at release time from the spec's Release section.
