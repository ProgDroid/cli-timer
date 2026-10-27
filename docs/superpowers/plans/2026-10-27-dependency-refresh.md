# v0.2.2 Dependency Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every direct dependency to its current release (ratatui replaces tui, crossterm 0.29, rodio 0.23, rand 0.10, clap/chrono lock updates) with no behaviour change except `-s` narrowed to four formats, then release v0.2.2.

**Architecture:** Tests that pin today's behaviour (keys, screen, real sound files) land first, on the old crates; then one upgrade per commit, smallest first, rodio last. Each commit is green on fmt, clippy and tests, so a later regression points at one upgrade.

**Tech Stack:** Rust 1.99 (pinned), ratatui 0.30.2 (crossterm 0.29 through its re-export), rodio 0.23.0 (symphonia decoders), rand 0.10.3, clap 4.6, chrono 0.4.45; `ffmpeg` (local, on PATH) to make fixtures.

**Spec:** `docs/superpowers/specs/2026-10-27-dependency-refresh-design.md` (read it; its Goals and Anti-goals bind every task). Red team: `docs/superpowers/reviews/2026-10-27-dependency-refresh-design-redteam.md`.

## Global Constraints

- Work on branch `deps-refresh` in the main checkout `G:\rustDev\cli-timer`. Do not edit `NEXT.md`.
- After every task: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all clean. Capture exit codes (`cmd > log 2>&1; echo "REAL_EXIT=$?" >> log`), never through a pipe.
- No visible or audible change: layout, black background, random colour, `-` prefix when triggered, paused/restart/label lines, alarm, chime, 500 ms fade-in, ring and cut-off times, human lines, JSON, exit codes 0/1/3.
- No new stdout/stderr text anywhere, including when an audio output stream is dropped.
- Tests never need an audio device and never play sound; `sound::check` and `sound::source` stay device-free.
- `-s` formats after Task 7: mp3, flac, ogg vorbis, PCM wav only (rodio features `playback`, `symphonia-mp3`, `symphonia-flac`, `symphonia-ogg`, `symphonia-vorbis`, `symphonia-wav`, `symphonia-pcm`; `default-features = false`).
- Keys act on `KeyEventKind::Press` only.
- Lint policy lives in `Cargo.toml` `[lints.clippy]`; fix new lints, do not add `allow`s without a one-line reason comment, matching the existing ones.
- Commits from the Bash tool, message via `git commit -F <file>`, ending with the session's `Co-Authored-By` trailer. Put the task's bead id alone on a line in the body.
- When a crate's API is in doubt, read the installed source under `~/.cargo/registry/src/*/<crate>-<version>/`, not docs.

## Review Focus

1. The time's row on screen. tui 0.19 floors `Percentage(49)` (row 11 at 80x24) and ratatui 0.30.2 rounds it (row 12); 39 of the 79 heights from 2 to 80 differ (plan red team, measured). Test: `time_row_is_49_percent_down_rounded_down` (Task 2) pins the absolute row; Task 6 keeps it with `Constraint::Length(area.height * 49 / 100)`.
2. The error text for a bad `-s` file, shown on stderr and in `--json`'s `error`. rodio 0.23 rewords `DecoderError` ("Unrecognized format" becomes "The format of the data has not been recognized."). Test: `a_text_file_reports_unrecognized_format` (Task 3) pins today's full line; Task 7 keeps it by writing that detail ourselves.
3. Dropping rodio's output sink logs to stderr by default since 0.21; under `remind --json` that corrupts the one-object-per-line contract. No test can open a device, so Task 7 opens the device in one function only (`sound::open_output`), which switches the logging off; the hands-on check (Task 8) runs `remind 5s --json` with sound.
4. A small terminal and long timers: 10x2 and 1x1 render without panicking, and 100 hours shows `100:00:00`. Tests: `renders_in_a_tiny_terminal`, `three_digit_hours_render_in_full` (Task 2).
5. A sound file with no extension or an upper-case one is accepted by content, not name. Test: `a_fixture_without_an_extension_still_decodes` (Task 3), kept passing through Task 7.

(Not on the list: key repeats. crossterm 0.29 on Windows sends only Press or Release, so a held key repeats as Press, as today; `repeat_events_change_nothing` in Task 1 is defensive, not a Windows proof.)

---

### Task 1: Key-handler tests and the Press-only filter (on crossterm 0.25)

crossterm 0.25 already has `KeyEvent::kind` (`KeyEventKind::{Press, Repeat, Release}`) and `KeyEvent::new_with_kind`, so the fix lands before the upgrade that makes it necessary.

**Files:**
- Modify: `src/handler.rs` (filter + `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `App::default()`, `App.state`, `App.running`, `State` (`src/app.rs`).
- Produces: `handle_key_events(key_event: KeyEvent, app: &mut App) -> Result<()>`, unchanged signature; ignores any event whose `kind` is not `Press`.

- [ ] **Step 1: Write the tests** in `src/handler.rs`. Helpers `press(code) -> KeyEvent` (`KeyEvent::new(code, KeyModifiers::NONE)`, which is `Press`) and `with_kind(code, kind)`. One test per row, asserting `app.state` and `app.running` after:

| Test | Start state | Key(s) | Expect |
|---|---|---|---|
| `space_pauses_a_running_timer` | Running | Space | Paused, running |
| `space_resumes_a_paused_timer` | Paused (pre_pause Running) | Space | Running |
| `r_asks_before_restarting` | Running | `r` | Restart |
| `r_twice_restarts` | Running | `r`, `r` | Running |
| `esc_on_the_restart_prompt_cancels_without_quitting` | Restart | Esc | Running, `running == true` |
| `q_quits` | Running | `q` | `running == false` |
| `ctrl_c_quits_and_plain_c_does_not` | Running | `c` then Ctrl+`c` | true after `c`, false after Ctrl+`c` |
| `release_events_change_nothing` | each of Running, Paused, Restart | Space, `r`, Esc, `q` as `Release` | state and `running` unchanged |
| `repeat_events_change_nothing` | Running | Space, `r`, `q` as `Repeat` | unchanged |

`r_twice_restarts` calls `App::restart`, which sends on `app.sender` only if set; `App::default()` has none, so no sound thread starts.

- [ ] **Step 2: Run** `cargo test --lib handler`. Expected: `release_events_change_nothing` and `repeat_events_change_nothing` FAIL; the rest pass.
- [ ] **Step 3: Implement** an early `return Ok(())` in `handle_key_events` when `key_event.kind != KeyEventKind::Press`, with a one-line comment saying why (Windows reports releases from crossterm 0.26 on).
- [ ] **Step 4: Run** `cargo test --lib handler`. Expected: all pass. Then the global fmt/clippy/test gate.
- [ ] **Step 5: Commit** `test(handler): pin key behaviour; act on key presses only`.

### Task 2: Screen tests through `TestBackend` (on tui 0.19)

**Files:**
- Modify: `src/app.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `App::render(&self, frame)`, public fields `state`, `time_left`, `colour`, `message`.
- Produces: test helper `fn screen(app: &App, w: u16, h: u16) -> Vec<String>` (one string per row, from the `TestBackend` buffer) and `fn cell_at(app, w, h, x, y)` returning the cell's `(fg, bg)`. Task 6 changes only these two helpers' bodies (tui's `Cell.symbol` field becomes ratatui's `symbol()` method).

- [ ] **Step 1: Write the tests.** Build `App::default()`, set `time_left` and `state` directly (no clock), `colour = Color::Cyan`. Terminal 80x24 unless stated, where the time is on **row 11** (tui floors `0.49 * 24`). Assert rows by index, never by searching:
  - `time_row_is_49_percent_down_rounded_down`: at width 80, for every height 2..=80, the time is on row `h * 49 / 100` (integer division) and the line under it on the next row (Paused state, so that line has text). Spot values: 24 -> 11, 25 -> 12, 40 -> 19.
  - `running_shows_the_time_centred`: `time_left = 1h 2m 3s`, Running → a row whose trimmed text is `01:02:03`, centred (left and right padding differ by at most 1); the row below is blank.
  - `triggered_shows_minus_and_the_label`: `time_left = -65 s`, Triggered, `message = Some("stretch")` → `-00:01:05` and, on the next row, `stretch`.
  - `paused_shows_paused`: Paused → the row under the time reads `Paused`.
  - `restart_prompt_text`: Restart → the row under the time starts `Are you sure you want to restart the timer?` (80 columns cut the rest).
  - `time_uses_the_colour_on_black`: the first digit cell of the time row has `fg == Color::Cyan`, `bg == Color::Black`; a cell in the top area has `bg == Color::Black`.
  - `three_digit_hours_render_in_full`: `time_left = 100 h` → `100:00:00`.
  - `renders_in_a_tiny_terminal`: 10x2 and 1x1 render without panicking.
  - The other tests use row 11 for the time and row 12 for the line under it.
- [ ] **Step 2: Run** `cargo test --lib app`. Expected: all pass (they describe today's screen). If one fails, the test is wrong, not the app: fix the test.
- [ ] **Step 3: Gate and commit** `test(app): pin the full-screen layout with TestBackend`.

### Task 3: Real sound-file fixtures and tests (on rodio 0.16)

**Files:**
- Create: `tests/fixtures/tone.mp3`, `tone.flac`, `tone.ogg`, `tone-adpcm.wav`, `tests/fixtures/README.md`
- Modify: `.gitattributes` (`tests/fixtures/** -text`), `src/sound.rs` (tests)

**Interfaces:**
- Consumes: `sound::check`, `sound::source`, `Sound::File`.
- Produces: fixture paths `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/<name>")`, used again in Task 7.

- [ ] **Step 1: Make the fixtures** (0.5 s, 440 Hz, mono, 22 050 Hz) with the local ffmpeg:
  ```
  ffmpeg -f lavfi -i "sine=frequency=440:duration=0.5:sample_rate=22050" -ac 1 <codec args> tests/fixtures/<file>
  ```
  codec args: `-c:a libmp3lame -b:a 32k` (mp3), `-c:a flac` (flac), `-c:a libvorbis -q:a 0` (ogg), `-c:a adpcm_ms` (adpcm wav). Add `-map_metadata -1 -fflags +bitexact` so reruns are stable. Each file must be under 16 KB; record the exact commands, `ffmpeg -version` first line, and why the ADPCM file exists (Task 7 asserts it is rejected) in the README.
- [ ] **Step 2: Write the tests** in `src/sound.rs`:
  - `real_mp3_flac_and_ogg_files_decode`: for each of the three, `check` is Ok, and `source` reports `channels() == 1`, `sample_rate() == 22_050`, and a decoded sample count within ±0.1 s of 0.5 s (11 025 ± 2 205).
  - `a_fixture_without_an_extension_still_decodes`: copy `tone.ogg` and `tone.mp3` to temp files named `cli-timer-<pid>-noext` and `...-UPPER.MP3`; both pass `check`; remove them.
  - `a_text_file_reports_unrecognized_format`: a temp text file's `check` error displays exactly `cannot decode sound file <path>: Unrecognized format` (today's rodio 0.16 text; Task 7 must keep it).
  - Measured by the plan red team: mp3 decodes to 12 672 samples on 0.16 and 11 025 on 0.23, ogg 11 264 on both; the ±2 205 tolerance covers both.
- [ ] **Step 3: Run** `cargo test --lib sound`. Expected: pass on rodio 0.16. (ADPCM is not asserted yet: 0.16 decodes it.)
- [ ] **Step 4: Check** `git check-attr text -- tests/fixtures/tone.mp3` says `unset`. Gate and commit `test(sound): decode real mp3, flac and ogg fixtures`.

### Task 4: `clap` and `chrono` to current

**Files:** Modify `Cargo.toml` (`clap = { version = "4.6", ... }`, `chrono = { version = "0.4.45", ... }`), `Cargo.lock`.

- [ ] **Step 1:** `cargo update -p clap -p chrono`; confirm `Cargo.lock` shows clap 4.6.7 and chrono 0.4.45 (or newer patch).
- [ ] **Step 2:** Gate. Fix any new deprecation warnings (clippy runs with `-D warnings`). `tests/cli.rs` covers clap's help and errors; any changed help text there is a behaviour change: stop and report rather than update the expectation.
- [ ] **Step 3:** Commit `chore(deps): clap 4.6 and chrono 0.4.45`.

### Task 5: `rand` 0.10

**Files:** Modify `Cargo.toml` (`rand = "0.10"`), `src/app.rs` (`random_color`).

**Interfaces:** Produces `fn random_color() -> Color`, unchanged.

- [ ] **Step 1: Write** `random_color_is_one_of_the_fifteen` in `src/app.rs`: 500 draws, each in the 15-colour list `random_color` matches on (no `Black`, no `Reset`). Run: pass on rand 0.8.
- [ ] **Step 2: Upgrade:** `rand = "0.10"`; `thread_rng()` becomes `rand::rng()`, `gen_range` becomes `random_range` (check the 0.10.3 source for the trait to import).
- [ ] **Step 3:** Gate; the new test still passes. Commit `chore(deps): rand 0.10`.

### Task 6: `tui` 0.19 to `ratatui` 0.30.2, crossterm 0.29

**Files:** Modify `Cargo.toml` (remove `tui` and `crossterm`; add `ratatui = "0.30.2"`), `src/app.rs`, `src/tui.rs`, `src/main.rs`, `src/event.rs`, `src/handler.rs`, test helpers from Task 2; `CLAUDE.md` (first line: `tui` 0.19 becomes `ratatui` 0.30).

**Interfaces:**
- Produces: `App::render(&self, frame: &mut ratatui::Frame)` (no backend generic); every crossterm path becomes `ratatui::crossterm::...`.
- Consumes: Task 1's filter (`KeyEventKind` now from `ratatui::crossterm::event`), Task 2's helpers.

- [ ] **Step 1: Swap the crates** and fix compile errors. Known renames: `tui::terminal::Frame<B>` to `ratatui::Frame`; `frame.size()` to `frame.area()`. Replace `Constraint::Percentage(49)` with `Constraint::Length(area.height * 49 / 100)` and a one-line comment (ratatui rounds percentages where tui floored them; this keeps tui's row, measured at every height 2..=80); `time_row_is_49_percent_down_rounded_down` must go red without it. Confirm `ratatui::crossterm` is re-exported in the installed 0.30.2 source; if not, add `crossterm = "<the version ratatui's Cargo.toml asks for>"` directly. If `Tui<B: Backend>`'s `?` no longer converts the backend's error into `Box<dyn Error>`, add the bound the compiler asks for (`B::Error: 'static` or similar), rather than making `Tui` concrete.
- [ ] **Step 2:** Update only the bodies of Task 2's `screen` and `cell_at` helpers.
- [ ] **Step 3: Run** `cargo tree -i crossterm` and confirm exactly one crossterm, 0.29.x. Gate: all Task 1 and Task 2 tests pass unchanged apart from the helpers.
- [ ] **Step 4:** Remove the "Migrate `tui` 0.19 to `ratatui`" entry from `docs/parking-lot.md` and the `tui` unmaintained mention under its "Known rough edges" pointer if any. Commit `chore(deps): replace tui with ratatui 0.30, crossterm 0.29`.

### Task 7: `rodio` 0.16 to 0.23.0

**Files:** Modify `Cargo.toml` (rodio line and its comment), `src/sound.rs`, `src/app.rs` (`start_sound`), `src/alert.rs` only if `SoundError` variants change; `docs/parking-lot.md`.

**Interfaces:**
- Produces: `sound::source(&Sound) -> Result<Box<dyn Source + Send>, SoundError>` (the `Item = f32` bound goes: rodio is f32-only since 0.21), `check`, `play`, `bound`, constants unchanged; `SoundError::{Open, Decode, Output}` kept with the same messages; `Playback` dropped only if no rodio call can fail that way any more.
- Consumes: Task 3's fixtures.

Upgrade map (from rodio's `UPGRADE.md`; confirm each against the installed 0.23.0 source):
`OutputStream::try_default()` to `let mut sink = DeviceSinkBuilder::open_default_sink()?; sink.log_on_drop(false);` (`log_on_drop` takes `&mut self` and returns `()`, so it cannot be chained); `Sink::try_new(&handle)` to `Player::connect_new(sink.mixer())`; `SamplesBuffer::new(1, SAMPLE_RATE, v)` to `SamplesBuffer::new(nz!(1), nz!(48_000), v)` or `NonZero` constructors; `Decoder::new(BufReader::new(file))` to `Decoder::try_from(file)`; `.convert_samples()` removed; opening the device fails with `rodio::DeviceSinkError` (`StreamError`/`OsSinkError` are gone).

- [ ] **Step 1: Cargo.toml:** `rodio = { version = "0.23.0", default-features = false, features = [the seven in Global Constraints] }`; rewrite the comment above it: why symphonia only, the four formats chosen 2026-10-27, and that MKV/WebM and ADPCM are deliberately off.
- [ ] **Step 2: Write** `an_adpcm_wav_is_rejected_as_undecodable` in `src/sound.rs`: `check(tone-adpcm.wav)` is `Err(SoundError::Decode { .. })`. It cannot pass on 0.16; it is part of this commit.
- [ ] **Step 3: Port** `src/sound.rs` and `App::start_sound` per the map. Add `pub fn open_output() -> Result<MixerDeviceSink, SoundError>` in `src/sound.rs`, the only code that opens the audio device: it opens the default sink and calls `log_on_drop(false)`, with a doc comment saying why (the drop message would land on stderr, inside `--json` output and over the full screen). `play` and `App::start_sound` both call it. `SoundError::Decode` keeps today's text: map `DecoderError::UnrecognizedFormat` to the detail `Unrecognized format` ourselves (other variants may show rodio's text), so `a_text_file_reports_unrecognized_format` passes unchanged. Tests compare `sample_rate()`/`channels()` through `.get()` where the types are now `NonZero`; the sample-exact alarm/chime/fade/loop tests must pass with no change to their numbers. Update the `SAMPLE_RATE` doc comment (drop the rodio 0.16 `SineWave` reference).
- [ ] **Step 4: Run** `cargo test`. Expected: all pass, including Task 3's fixture tests unchanged and the new ADPCM test. Then `grep -rn "open_default_sink\|DeviceSinkBuilder" src/` finds them only inside `open_output`. `cargo tree -e features -i symphonia` lists no `mkv`, `adpcm` or `isomp4`.
- [ ] **Step 5:** Remove the "Upgrade `rodio`" parking-lot entry. Extend the "rodio's next big release" entry: rodio's source says its `speakers` API "will eventually replace" the device sink, and that API has no `log_on_drop`, so the stderr guarantee must be re-proved by hand then (the fixture tests cannot see it). Gate and commit `chore(deps): rodio 0.23; -s takes mp3, flac, ogg vorbis and wav`.

### Task 8: Version, merge, release

**Files:** Modify `Cargo.toml` and `plugin/.claude-plugin/plugin.json` (`version` to `0.2.2`), `Cargo.lock`.

- [ ] **Step 1:** Bump both versions; `cargo test` (the `plugin_version_matches_crate` test checks they match). Commit `chore: v0.2.2`.
- [ ] **Step 2:** Whole-branch review by a fresh reviewer against the spec; fix its small findings on the branch before merging (developer's standing preference).
- [ ] **Step 3:** In the main checkout on `main`: `git merge --squash deps-refresh`, update `NEXT.md` in the same commit (what changed, the narrowed formats, the hands-on list), subject `handoff: v0.2.2 dependency refresh merged ...`. Ask the developer before `git push origin main`.
- [ ] **Step 4:** After the push, CI (`check.yml`) green on Linux, Windows and macOS: `gh run watch <id> --exit-status`. (The spec's Windows-GNU clippy check is the substitute for cloud sessions without CI; on this Windows machine the native build plus CI's Windows job cover it.)
- [ ] **Step 5:** Developer's hands-on check from the spec (install from the checkout; keys on Windows; alarm; mp3 `-s`; `remind 5s -l test`; `ding "done"`; `remind 5s --json` prints JSON lines only).
- [ ] **Step 6:** `gh workflow run release.yml --ref main`; wait; four artifacts listed. Ask the developer, then push the lightweight tag `v0.2.2` on that commit and check the release has four archives plus `.sha256` files. Release notes mention the `-s` format change.
- [ ] **Step 7:** Close the beads; hand off (`NEXT.md` alone, `handoff:` subject).
