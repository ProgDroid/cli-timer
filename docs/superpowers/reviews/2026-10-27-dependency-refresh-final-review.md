# Final whole-branch review: v0.2.2 dependency refresh (cec62cc..dc75451)

Reviewer: the final-review seat. Read-only on the checkout. I did the work in passes: (1) the source diff and
the task reviews, (2) one focused check for each named cross-task risk, (3) the docs and metadata, (4) triage of
the deferred minors, (5) the verdict. I skimmed Cargo.lock and the binary fixtures and did not read them line by
line.

## Evidence base

- Gates re-run at dc75451 on this checkout (logs in the session scratchpad):
  - `cargo fmt --all -- --check`: REAL_EXIT=0.
  - `cargo clippy --all-targets -- -D warnings`: REAL_EXIT=0.
  - `cargo test`: REAL_EXIT=0, lib 152 + cli 33 = 185 passed.
- Old-vs-new comparisons ran against two scratch worktrees:
  - a build of the base cec62cc (rodio 0.16, tui 0.19, clap 4.1.1);
  - the Task 2 commit 86cd050 (tui 0.19 with the TestBackend helpers), plus a head copy, each with a
    throw-away screen-dump test.

  All three worktrees were removed afterwards. The checkout stayed at dc75451 on deps-refresh, clean.
- Not run:
  - Linux and macOS: Docker's daemon is not running here, and the branch has never been pushed, so
    `gh run list --branch deps-refresh` is empty.
  - Real sound: I played nothing.

## Named cross-task risks: one focused check each

### 1. User-visible strings

Method: I ran the base binary and the head binary (`CLI_TIMER_QUIET=1`) on the same inputs and diffed
their output.

- **remind/ding human lines and JSON**: byte-identical, with the same exit codes. Inputs: `remind 0s`,
  `remind 0s --json -l hi`, `ding`, `ding --json done`, a bad duration, a too-long duration (human and
  JSON), a missing `-s` file and a text `-s` file.
- **`-s` errors** (stderr and the `--json` `error` value), measured:

  | input | 0.2.1 (rodio 0.16) | head (rodio 0.23) |
  |---|---|---|
  | text file | `Unrecognized format` | `Unrecognized format` (pinned by a test) |
  | empty .mp3 | `Unrecognized format` | `Unrecognized format` |
  | missing file | `cannot open ...: The system cannot find the file specified. (os error 2)` | same |
  | ADPCM wav | plays (exit 0) | `cannot decode ...: Unrecognized format` (the deliberate change) |
  | truncated flac (first 300 B) | `cannot decode ...: end of stream` | `cannot decode ...: An IO error occurred while reading, writing, or seeking the stream.` |
  | header-only wav (44 B) | `cannot decode ...: end of stream` | the same generic IO line |

  The last two rows are a changed user-visible line that no test sees: **Important 1**.
- **Other decoder variants**: rodio 0.23 also rewords them (rodio-0.23.0/src/decoder/mod.rs:765-797, against
  rodio-0.16.0/src/decoder/mod.rs:552-569).
  - `DecodeError(msg)` and `LimitError(msg)` gain a long prefix.
  - `ResetRequired` was "Reset required"; it is now "The demuxer or decoder needs to be reset before continuing."
  - `NoStreams` was "No streams"; it is now "No streams were found by the decoder."

  I did not find a file that triggers these.
- **No-device error** (the exit-3 path and the `error` value in the fired event): UNKNOWN at runtime, because
  this machine has a device.
  - `SoundError::Output` now wraps `DeviceSinkError`, whose Display is a fixed sentence that does not include
    the cpal cause (rodio-0.23.0/src/stream.rs:456-476: `#[source]`, no `{0}`).
  - 0.16's `StreamError` printed the cpal error itself, and `NoDevice` when there was no device
    (rodio-0.16.0/src/stream.rs:154-163).

  This is **Minor 4**.
- **clap 4.1.1 -> 4.6.7**, measured:
  - (a) Every `--help` page's option column is one space narrower: `-t <TIME>       Timer...` is now
    `-t <TIME>      Timer...`, and the same holds for remind's and ding's pages.
  - (b) `cli-timer -t 5s remind 1s` said `error: unexpected argument 'remind' found` and now says
    `error: the subcommand 'remind' cannot be used with '-t <TIME>'`.

  Exit codes are identical across all 13 invocations I diffed. No test sees either change, and both come from
  clap upstream. This is **Minor 3**.
- **Full-screen no-device line**: it has a doubled prefix (Task 7's deferred minor 4 and Ruling R5). This is
  **Minor 2**.
- **Exit codes**: they do not depend on the `SoundError` variant (alert.rs:22 wraps any `SoundError`, and
  output.rs:108 gives 3 for any partial failure). Removing `SoundError::Playback` cannot change an exit code.

### 2. `-s` check before the device, and `check` stays device-free

- src/main.rs:50 `sound::check(&app.sound)?` still runs before `Terminal::new` and `tui.init()`. src/remind.rs:65
  and src/ding.rs:28 still call `sound::check` before anything fires.
- `check` is `source(..).map(drop)` (src/sound.rs:168-170). `source` builds either `SamplesBuffer`s or
  `Decoder::try_from(File)` (rodio-0.23.0/src/decoder/mod.rs:280-294: metadata, `BufReader` and the builder,
  with no device).
- `open_output` is called only from `play` (src/sound.rs:228, after `source`) and `App::start_sound`
  (src/app.rs:216, after `source`).

**Holds.**

### 3. Stdout/stderr silence

I listed every print site in the compiled crates, rather than looking only for the one already known.

- **rodio 0.23** (grep for `eprintln!`/`println!` over src):
  - stream.rs:89 is the drop message. `log_on_drop(false)` turns it off (src/sound.rs:214) on the only sink we
    open.
  - stream.rs:243 ("error getting list of output devices") is only inside `open_default_sink`, which we no longer
    call. `open_output` reimplements its fallback without the print (src/sound.rs:195-213).
  - stream.rs:168 is `default_error_callback`, "audio stream error: {err}". It replaces 0.16's "an error
    occurred on output stream: {}" (rodio-0.16.0/src/stream.rs:198): an existing line reworded, not a new
    one. This is Minor 7.
  - decoder/wav.rs (hound) is not compiled. The rodio features are
    `playback,symphonia-{flac,mp3,ogg,pcm,vorbis,wav}`, and `cargo tree -i hound` matches no package.
  - The microphone and speakers builders are unused, and channels.rs:151 is test-only.
  - rodio's `tracing` feature is off. The `tracing` crate is in the graph only through zbus and notify-rust.
- **cpal 0.18.2**: its only print sites are under `#[cfg(test)]` (host/coreaudio/macos/mod.rs:457).
- **ratatui/crossterm**: they write only to the backend (stderr, src/main.rs:52), as tui did.
- The `quiet notify:` and `quiet play:` lines in my runs come from the quiet alerter, as before.

**Holds**: there is no new print path in remind, ding, `--json` or the full-screen timer. Checking it at runtime
with a real device (`remind 5s --json` with sound) is still the developer's hands-on item.

### 4. Oracle tests not weakened

- `git diff 2d61d4a dc75451 -- src/handler.rs`: only the import path changed.
- `git diff 86cd050 dc75451 -- src/app.rs` (the test module): the helpers' two buffer-indexing expressions and the
  import changed. No test body, number or case changed. One test was added (`random_color_is_one_of_the_fifteen`).
- `git diff 79205d8 dc75451` (sound.rs tests): `.get()` was added to six `channels()`/`sample_rate()`
  comparisons (Ruling R1). Every expected number is byte-identical, as are the `11_025 ± 2_205` range and the
  exact "Unrecognized format" line. One test was added (ADPCM).
- tests/cli.rs is unchanged across the whole branch.

**Holds.**

Beyond the oracle, I rendered 882 screens on tui 0.19 and on ratatui 0.30.2 and compared every cell's symbol, fg
and bg:
- **Inputs**: 7 states (including a long label, 100 h and the restart prompt), widths from 1 to 200, and
  heights from 1 to 120.
- **Height 2 and over**: all 784 screens are identical. This includes the truncated restart prompt at 20 to
  87 columns, and heights of 81 and 120 rows.
- **Height 1**: the only difference. tui drew a blank row with the Reset background. ratatui draws a black
  row, and in the Restart state it draws the prompt line. This is Minor 6.

### 5. Docs and metadata: lines that are now false or stale

- **Cargo.toml:13-14**, "rodio's other mp3 decoder (minimp3 -> slice-deque) aborts debug builds": this
  describes 0.16. In 0.23 the alternative is the opt-in `minimp3_fixed` (rodio-0.23.0/Cargo.toml:65, 365).
  Reword it as history (Task 7's deferred minor 3). Minor 1a.
- **tests/fixtures/README.md**:
  - Line 3, "(src/sound.rs tests; Task 7 of the dependency refresh reuses them)", is a plan-internal pointer
    (Task 3's deferred minor 3).
  - Lines 15-16 ("rodio 0.16 decodes it; after the move to rodio 0.23 ... it must be rejected") are still
    true, but are written in the future tense.

  Point at the test `an_adpcm_wav_is_rejected_as_undecodable` instead. Minor 1b.
- **docs/parking-lot.md:57-62** (the rodio's-next-release entry): true, but it names only the drop message.
  `open_output` now also copies rodio's device fallback (src/sound.rs:183-187). The next upgrade must check that
  copy against rodio's own `open_default_sink`, and drop it if the print is gone. Minor 1c.
- **docs/parking-lot.md has no entry for the surround upmix.** It is Task 7's deferred minor 2 (ledger line 47:
  "for hands-on + parking lot"). CLAUDE.md requires deferred items to live in the parking lot. This is
  Important 3.
- **README.md**: says nothing about which `-s` formats work, and this release narrows them. One line next to
  README:26-27 (remind) or :50 (ding) would document the deliberate change. Minor 1d.
- **The plan** (docs/superpowers/plans/2026-10-27-dependency-refresh.md):
  - Line 130 (`thread_rng()` becomes `rand::rng()`): the code uses the free function `rand::random_range`
    (src/app.rs:36).
  - Lines 155 and 159 (`open_output` opens the default sink with `DeviceSinkBuilder::open_default_sink()`):
    superseded by Task 7's fix round.

  The plan is a record of what was planned, so I would leave it unchanged.
- **The spec**: it does **not** name `rng()`/`random_range`. A grep for `rng\(\)|random_range` over the spec
  found nothing, and the same grep matched spec lines 70 and 124 for its other terms, so it can match. The rand
  wording appears only in the plan. I found no false line in the spec:
  - Line 69 (rejected at start with the usual "cannot decode" error) is confirmed by measurement: the ADPCM
    file gives `cannot decode ...: Unrecognized format`.
  - Lines 26-30 are confirmed:
    - thiserror 1.0.38 and 2.0.21 are both in Cargo.lock.
    - rust-toolchain.toml says 1.99.
    - The highest `rust-version` in `cargo metadata` is 1.89.0 (notify-rust and uuid); ratatui's is 1.88.
- **CLAUDE.md:3** is correct.
  - "Linux builds need ALSA headers" is still true: cpal 0.18.2 depends on `alsa` 0.11 on Linux.
  - README:54 ("Rust 1.91 or later") is unaffected.
  - plugin.json and Cargo.toml both say 0.2.2, and SKILL.md's ">= 0.2.0" is unaffected.

**What NEXT.md must say at the merge** (listed as asked, not flagged):
- **What merged**: v0.2.2, not yet released. ratatui 0.30.2 (with crossterm 0.29 through its re-export),
  rodio 0.23.0, rand 0.10, clap 4.6.7 and chrono 0.4.45. Keys act on Press only (the Windows fix for
  release events).
- **The deliberate change**: `-s` takes mp3, flac, ogg vorbis and PCM wav only. MKV/WebM and ADPCM wav played in
  0.2.1 and are now rejected with `cannot decode sound file <path>: Unrecognized format`. The release notes
  should say the same.
- **Known rough edges** (line 68):
  - Drop "rodio 0.16" and "`tui` unmaintained".
  - Add that rodio 0.23 is an upstream stopgap (see the parking lot).
  - If not fixed: on outputs with more than 2 channels, the sound plays only from front left and right.
  - clap's help column is one space narrower, and one usage error is reworded.
  - If not fixed: the no-device error wording now comes from rodio 0.23.
- **Still to do**:
  1. The spec's Windows hands-on check:
     - the full screen with the built-in alarm, and with an mp3 `-s`;
     - Space, Space, r, Esc, r r, q;
     - `remind 5s -l test` and `ding "done"`;
     - `remind 5s --json`, which must print only JSON.
  2. The release dry run (four archives).
  3. Tag v0.2.2.

  macOS and Linux sound and pop-ups are still unchecked by a person.
- **Housekeeping**: the feedback-log entry the handoff routine asks for, and the status of beads epic
  clitimer-p7x.

### 6. Surround upmix (outputs with more than 2 channels)

I confirmed it in the source, and **it is wider than the ledger says**.
- **0.16**: `ChannelCountConverter` copied the **last** input channel into every extra output channel
  (rodio-0.16.0/src/conversions/channels.rs:52-75). Mono went to every channel, and a stereo file played as
  L, R, R, R, R, R on a 5.1 output.
- **0.23**: the converter (rodio-0.23.0/src/conversions/channels.rs:57-66) copies mono into channel 1 only, and
  fills every channel from index 2 up with zeros.
- **Effect**: on a 5.1 or 7.1 output, the mono built-in alarm and chime, **and stereo `-s` files**, play nothing
  from the centre, LFE and surround speakers. The mixer applies this conversion to every source it is given
  (rodio-0.23.0/src/mixer.rs:62, then source/uniform.rs:53-66).

Is there a small fix in rodio's public API?
- **`rodio::source::ChannelVolume::new(src, vec![1.0; n])`** copies a mono signal into n channels, and the
  device's channel count is public (`MixerDeviceSink::config().channel_count()`, stream.rs:72/124). It is
  **not a safe drop-in**:
  - (a) Its `current_span_len()` passes the input's span length through, while it emits n samples for each
    input frame. The mixer's `UniformSourceIterator` cuts spans by that length (uniform.rs:55-62), so it would
    re-bootstrap mid-span, and for a span length that is not a multiple of n it would misalign the channels.
  - (b) For a stereo input it adds the channels without averaging them: the result of the averaging line is
    discarded, and upstream marks the code "TODO Need a test for this". That doubles the amplitude.
- **A correct fix is our own wrapper `Source`**, about 40 lines:
  - It copies the last input channel into the extra channels (0.16's rule, which covers both mono and stereo).
  - It reports `channels() = n` and `current_span_len() = inner * n / from`.
  - It is applied in `play` and `start_sound` after `open_output`, when the device has more than 2 channels.
  - It can be unit-tested without a device (wrap a `SamplesBuffer`, pass n explicitly, assert the frames), but
    no one here can listen to it without a surround output.
- **Size**: small to medium, about 40 lines and 2-3 tests touching the two playback sites. rodio's "bigger
  release" may change this area again.
- **Recommendation**: do not hold the merge for the code. The spec's anti-goal ("any audible difference") is
  broken on such outputs, so the developer must rule on it explicitly:
  - either accept it, with a parking-lot entry and a release-note line (Important 3);
  - or have the wrapper written now, if he or his users have surround outputs.

---

## Strengths

- **The test-first order worked as designed.** The key, screen and real-format oracles were written on the old
  crates, and none was weakened later (risk 4). The two version behaviours that differ upstream were each caught
  by a test that went red and then green: tui floors `Percentage` where ratatui rounds it, and crossterm 0.26+
  sends Release events. My 882-screen cell-by-cell comparison found the ratatui port pixel-identical at every
  height of 2 or more, including colours and truncation.
- **`open_output` is a good single choke point.** It is documented, it is the only code that opens a device, and
  it is silent on every path, including the rodio fallback print that the Task 7 fix round removed without
  adding a dependency. The print-site sweep (risk 3) found no other path.
- **`decode_detail` keeps the pinned "Unrecognized format" line**, and the ADPCM fixture pins the narrowed format
  set. Both were measured end to end against the 0.2.1 binary.
- **The dependency graph is tidy**: one crossterm through ratatui's re-export, the rodio features exactly as the
  spec lists them, no `tracing` feature, no hound, no mkv/adpcm/isomp4 symphonia crates.
- **The process evidence is honest**: pre-registered mutations, exit codes captured, a withdrawn ruling (R4)
  instead of a quietly accepted breach.

## Issues

### Critical (Must Fix)

None.

### Important (Should Fix)

1. **src/sound.rs:83-88: `decode_detail` lets rodio 0.23's rewording through for every variant except
   `UnrecognizedFormat`.**
   - What happens: a truncated or damaged file says `An IO error occurred while reading, writing, or seeking
     the stream.` where 0.2.1 said `end of stream`. This was measured on Windows with a 300-byte flac and a
     44-byte wav, on stderr and in `--json` `error`. The cause is that rodio 0.23's `IoError(String)` Display
     drops its payload (rodio-0.23.0/src/decoder/mod.rs:771-772). The payload is still
     `e.to_string()` of symphonia's io error (decoder/symphonia.rs:37), so it can be recovered.
   - Why it matters: a bad `-s` file is the most likely error a user meets. The spec keeps these lines
     unchanged, and the new text drops the cause (on Linux and macOS, also the cause for `-s some_dir`).
   - Fix: restore 0.16's text for every variant. `DecoderError` is not `#[non_exhaustive]`, and all these
     variants exist under the symphonia features we enable:
     ```rust
     DecoderError::UnrecognizedFormat => "Unrecognized format".to_owned(),
     DecoderError::IoError(msg) => msg.clone(),
     DecoderError::DecodeError(msg) | DecoderError::LimitError(msg) => (*msg).to_owned(),
     DecoderError::ResetRequired => "Reset required".to_owned(),
     DecoderError::NoStreams => "No streams".to_owned(),
     ```
   - Test: copy the first 300 bytes of `tests/fixtures/tone.flac` to a temp file and assert the full line
     `cannot decode sound file <path>: end of stream`. Pre-register it as 1 failing test before the fix.
   - This supersedes Task 7's deferred minor 1.
2. **CI has never run on this branch, and the audio stack under it was replaced.** cpal moved from 0.14 to 0.18
   (alsa 0.11 on Linux, coreaudio on macOS) and ratatui/crossterm moved too. Only Windows MSVC was built and
   tested, here.
   - Why it matters: the repo's routine squash-merges into `main` locally and then pushes `main`. A Linux or macOS
     compile or clippy failure would first show up as a red `main`, just before the release.
   - Fix: push `deps-refresh` (check.yml runs on every push) and wait for Linux, Windows and macOS to go green
     before the squash merge. This needs the developer's go-ahead to push. Alternative: run the Linux clippy and
     tests in Docker once the daemon is up.
   - This is a gate, not a code change, and it costs about 10 minutes.
3. **The surround upmix has no ruling and no parking-lot entry** (risk 6).
   - What it is: rodio 0.23 zero-fills channels 3 and up, for mono **and** stereo sources. Task 7's deferred minor
     covered only mono.
   - Why it matters: it breaks the spec's anti-goal on 5.1/7.1 outputs. CLAUDE.md also requires anything deferred
     to live in `docs/parking-lot.md`, and the ledger's "for hands-on + parking lot" has not been carried out.
   - Fix before merge: the developer rules on it. If he accepts it, add a dated parking-lot entry (with the
     wrapper-`Source` sketch from risk 6 and the `ChannelVolume` caveats) and a release-note line. If he does
     not, write the wrapper.

### Minor (Nice to Have)

1. **Four stale doc lines** (risk 5):
   - (a) Cargo.toml:13-14: say that 0.16's minimp3 decoder aborted debug builds, as history.
   - (b) tests/fixtures/README.md:3 and :15-16: name the test, not "Task 7", and use the past tense.
   - (c) docs/parking-lot.md:57-62: add that `open_output` copies rodio's `open_default_sink` fallback, so the
     next rodio upgrade must re-check or drop that copy.
   - (d) README.md: one line saying `-s` takes mp3, flac, ogg vorbis and wav (PCM).

   Each is a one-liner. **Fix before merge.**
2. **src/app.rs:216-221: the no-device line in the full-screen timer reads `Could not open output stream: no audio
   output available: <rodio text>`.** This is Task 7's deferred minor 4 and Ruling R5. Fix: make `open_output`
   return `Result<MixerDeviceSink, DeviceSinkError>`. `play`'s `?` still converts it through
   `#[from] DeviceSinkError` into `SoundError::Output`, so remind and ding keep their text, and app.rs then prints
   the bare rodio error as 0.16 did. Three lines. **Fix before merge.**
3. **clap 4.6 help and usage text** (risk 1):
   - The option column is one space narrower.
   - `-t 5s remind 1s` now says `the subcommand 'remind' cannot be used with '-t <TIME>'`.

   Exit codes are unchanged, and the new error reads better. **Accept.** Mention it in the release notes and
   NEXT.md; there is nothing to fix.
4. **src/sound.rs:75-76: the no-device text (`SoundError::Output`) loses the cpal cause.** `DeviceSinkError`'s
   Display omits its `#[source]`, so the exit-3 `error` value is now a fixed sentence such as
   `no audio output available: Error opening the stream with the OS`. 0.16 showed the cpal message itself.
   - Fix: display `e` and, if there is one, `e.source()` after `: `. It is about 5 lines, and unit-testable
     only for `NoDevice`, which has no source.
   - **Can wait**: it needs an audio-less machine to see. Park it, or do it together with Important 1 if
     that is cheap.
5. **src/app.rs:133-134: `area.height * 49 / 100` overflows u16 above 1337 rows.** In a debug build that panics;
   in a release build the first chunk wraps to a small value. The comment's "at every height" is therefore
   stronger than the code. This is parity with tui 0.19 (layout.rs:275), not a regression (Task 6's minor 1).
   - Fix, exact at every u16 with no casts: `area.height / 100 * 49 + area.height % 100 * 49 / 100`.
     `render` already allows `modulo_arithmetic`.
   - **Fix before merge**: one line, and it makes the comment true.
6. **Height-1 terminal**: tui drew nothing, while ratatui draws a black row (or the restart prompt). A one-row
   terminal is not a real use case. **Accept** with no action.
7. **The mid-playback stream-error line is reworded upstream** (`an error occurred on output stream:` became
   `audio stream error:`). It is an existing line, reworded by rodio's `default_error_callback`.
   `DeviceSinkBuilder::with_error_callback` could restore the old wording, but that needs a builder-type
   change in `open_output`. **Can wait.**

## Deferred minors triage (the ledger's "minor (deferred)" lines and each task-N-review.md)

| Source | Minor | Call |
|---|---|---|
| T1 | repeat test skips Esc and the Restart state | wait (defensive test; the brief's scope) |
| T1 | `State` lacks `Debug`, so `assert!(a == b)` prints no values | wait (out of scope) |
| T2 | 24/25/40 spot loop duplicates the 2..=80 sweep | wait (reads as documentation) |
| T2 | `screen`/`cell_at` repeat the Terminal setup | wait |
| T2 | no counted mutation for colour, bg or 100 h | wait (my 882-screen cell comparison covers colour and bg parity) |
| T2 | tiny-terminal test asserts only that it does not panic | wait (as specified) |
| T3 | no-extension test asserts `check` only | wait |
| T3 | sample count cannot catch silence | wait |
| T3 | fixtures README "Task 7" pointer | **fix before merge** (Minor 1b) |
| T4 | no mutation for a dependency bump | none |
| T5 | 15-colour list duplicated in the test | wait |
| T5 | variety assert is weak | wait (a uniformity test would be flaky) |
| T6 | u16 overflow above 1337 rows; comment overclaims | **fix before merge** (Minor 5) |
| T6 | termwiz/termina lock entries (audit noise) | none (not compiled) |
| T6 | Percentage revert count not pre-registered | none (process) |
| T7 | `decode_detail` drops the `IoError` payload | **fix before merge**, promoted to Important 1 (measured, and covers every variant) |
| T7 | mono upmix on outputs with more than 2 channels | **ruling + parking-lot entry before merge** (Important 3); the code fix is optional |
| T7 | Cargo.toml minimp3 comment describes 0.16 | **fix before merge** (Minor 1a) |
| T7 | doubled no-device prefix (also R5) | **fix before merge** (Minor 2) |

## Declined to judge (set aside; the executor rules on each)

- **Whether a real device prints anything at runtime** (the drop line, the fallback): this is the developer's
  hands-on item (spec, Task 8), and I did not play sound on his machine.
- **Loudness and resampling quality** of rodio 0.23's sample-rate converter, for files whose rate differs from
  the device's: not measurable without a device. The fade-in shape was measured by the Task 7 implementer.
- **The Windows-GNU clippy check**: CLAUDE.md scopes it to sessions without native Windows. Native MSVC clippy
  ran here.
- **Linux and macOS behaviour of `open_output`'s "null"-driver filter** (cpal's `description().driver()` on
  each host): it mirrors rodio's own code and cannot be run here.
- **The content of `.beads/issues.jsonl`**: tracker data, not product.
- **Key repeat on Windows**: globals.md already rules that the repeat test is defensive.
- **Lock entries of crates that are not compiled** (termwiz, termina): not built, per Task 6's review.

## Recommendations

- Do Important 1 and Minors 1, 2 and 5 as one small fix commit on the branch, with the truncated-flac test
  pre-registered red, then re-run the gates.
- Push the branch for CI (Important 2) before the squash merge, which needs the developer's go-ahead.
- Put the surround question (Important 3) to the developer as a recognition choice: "accept and park" or
  "write the wrapper now".
- Use the stale-line list and the NEXT.md list in risk 5 for the merge commit.

## Assessment

**Ready to merge?** With fixes. The verdict is "needs fixes".

**Reasoning:** The port is faithful. The layout is cell-identical at every height of 2 or more, there is no new
print path, keys act on Press only, the oracles are intact, and the gates are green on Windows. But one
user-visible `-s` error line regressed and was measured, the surround change has neither a ruling nor a
parking-lot entry, and Linux and macOS have never been built on this branch. All of these are small, and none
needs a redesign.

### Counts

Critical 0 · Important 3 · Minor 7.

Must fix before merge: Important 1 (decode-detail parity, with a test), Important 2 (CI green on the branch),
Important 3 (a surround ruling and a parking-lot entry), Minor 1 (four doc lines), Minor 2 (the doubled prefix)
and Minor 5 (the overflow-free row floor).

## Fix wave re-review (2026-10-27)

# Re-review of the final fix wave (dc75451..8814ced)

### Finding Verdicts

- **Important 1: decode error detail keeps rodio 0.16's text for every `DecoderError` variant, on stderr and in `--json`.** ADDRESSED.
  - src/sound.rs:88-96 `decode_detail` now matches all six variants, with no catch-all arm.
  - Check: each string matches rodio-0.16.0/src/decoder/mod.rs:552-569. UnrecognizedFormat gives "Unrecognized format", IoError gives `&msg[..]`, DecodeError and LimitError give `msg`, ResetRequired gives "Reset required", NoStreams gives "No streams".
  - The payloads are built the same way in both versions: rodio-0.16.0 and rodio-0.23.0 src/decoder/symphonia.rs:37-47 are identical (`IoError(e.to_string())`). So `msg` is the text 0.16 printed ("end of stream").
  - Check: rodio-0.23.0's `DecoderError` has no `#[non_exhaustive]` (grep found none), so the exhaustive match compiles and will break loudly on a new variant.
  - Tests: src/sound.rs:556 `a_truncated_flac_reports_end_of_stream` and :568 `a_wav_cut_off_before_its_data_reports_end_of_stream` pin the full line through `check`. :583 `every_decoder_error_reads_as_it_did_with_rodio_0_16` covers all variants. stderr and `--json` both use `SoundError`'s Display (:76), and the report shows both end-to-end lines.

- **Important 3: on outputs with more than 2 channels, fill channels 3 and up as rodio 0.16 did, for mono and stereo. 1-2 channel outputs stay bit-identical. Applied in `play` and `App::start_sound`.** ADDRESSED.
  - Sample-by-sample comparison with rodio-0.16.0/src/conversions/channels.rs:56-80:
    - 1 to 6: 0.16 reads at pos 0 (`== from-1`, saving the repeat) and repeats at pos 1-5. `FillChannels::next` (src/sound.rs:241-254) reads at pos 0 (`pos < from`, `last` = s) and emits `last` at pos 1-5. Both give [s,s,s,s,s,s].
    - 2 to 6: 0.16 reads L (pos 0), reads and saves R (pos 1), then repeats R at pos 2-5. FillChannels reads L, then R (`last` = R), then emits R at pos 2-5. Both give [L,R,R,R,R,R].
    - End of input: at a frame boundary both return None. When `from > to`, FillChannels passes the frame through (width = from) and lets rodio drop the extra channels, as 0.16 and 0.23 both do.
  - Source impl against rodio 0.23's trait (rodio-0.23.0/src/source/mod.rs:167-201, where the span is the total length, `Some(0)` means exhausted, and spans are frame multiples):
    - `channels()` is `widen(inner.channels())` at a frame boundary and the frame's fixed width mid-frame (:264-270). The bootstrap at uniform.rs:55-66 reads it at a frame boundary, so it sees 6.
    - `current_span_len` is `len / from * to` (:258-262). It is always a multiple of the reported channel count, and it is `Some(0)` exactly when the inner span is `Some(0)`.
    - `sample_rate`, `total_duration` and `try_seek` pass through; a seek resets `pos`. Duration does not depend on the channel count.
    - Downstream, the mixer's converter at uniform.rs:66 then runs 6 to 6, which is a pass-through (rodio-0.23.0/src/conversions/channels.rs:57-62: every position below `from` reads).
    - The mixer's width is `config.channel_count` (rodio-0.23.0/src/stream.rs:494), the same value the callers pass.
  - Gate: `channels.get() <= 2` returns `source` itself (src/sound.rs:200-202), so 1-2 channel outputs are identical by construction.
    - The new `Box::new(source)` at src/app.rs:227 forwards every `Source` method (source_pointer_impl, mod.rs:796-830), plus `next` and `size_hint`.
    - `is_exhausted` is not overridden anywhere in rodio 0.23 (grep found only the default at mod.rs:187). So the box changes nothing.
  - Call sites: src/sound.rs:338 (`play`) and src/app.rs:226-229 (`start_sound`), each after `open_output`. The fade-in is applied before the widening, so its shape is unchanged.
  - The parking-lot entry for the next rodio upgrade names `fit_channels` (docs/parking-lot.md:63-67).

- **Minor 1: four stale doc lines.** ADDRESSED.
  - Cargo.toml:13-15: the minimp3 abort is told as history, and `minimp3_fixed` is named as opt-in.
  - tests/fixtures/README.md:4 no longer says "Task 7". :16-18 is in the past tense and names `an_adpcm_wav_is_rejected_as_undecodable`.
  - docs/parking-lot.md:63-67 has a dated addition for the `open_output` fallback copy (and for `fit_channels`).
  - README.md:15-16: "mp3, flac, ogg vorbis or wav (PCM)", covering `-t`, `remind` and `ding`.

- **Minor 2: no doubled "no audio output available" prefix in the full-screen no-device line; remind and ding wording unchanged.** ADDRESSED.
  - src/sound.rs:299: `open_output` returns `Result<MixerDeviceSink, DeviceSinkError>`. src/app.rs:220 now prints the bare rodio error after "Could not open output stream: ".
  - `play` (src/sound.rs:336) converts the error with `?` through `#[from] DeviceSinkError` (:79), so remind and ding keep their text. Test: :736 `remind_and_ding_still_word_a_missing_output_the_same_way`.
  - grep found only two callers of `open_output` (app.rs:217 and sound.rs:336).

- **Minor 5: the time-row height must not overflow u16, with the same rows at every height.** ADDRESSED.
  - src/app.rs:135 is `h / 100 * 49 + h % 100 * 49 / 100`. With h = 100q + r this is 49q + floor(49r/100) = floor(49h/100). Its largest value at u16::MAX is 32 095 + 48, so it cannot overflow.
  - Test: src/app.rs:382 renders 80x2000 and checks rows 980 and 981. The 2..=80 sweep at :309 is unchanged.

### Checks run

1. **Channel mapping and Source impl**: compared against rodio-0.16.0 channels.rs and rodio-0.23.0 source/mod.rs, uniform.rs, mixer.rs, queue.rs, player.rs and conversions. Results are above.
2. **Decoder texts**: compared against rodio-0.16.0 decoder/mod.rs:552-569, and both versions' symphonia.rs:37-47. They match.
3. **src/app.rs at HEAD 8814ced (worktree clean)**:
   - It contains Minor 5 (:135), the `fit_channels` call in `start_sound` (:226-229), the 2000-row test (:382), and the bare `{e}` line that Minor 2's type change relies on (:220).
   - `git diff dc75451 8814ced -- src/app.rs` holds only those three hunks.
   - Per-commit stats: app.rs is touched only by 6d4bec2 (5 lines) and 2fd6fda (13 lines). bf0e518 touches only sound.rs, which fits the staging account.
   - Mutation grep: `if false|if true|&& false|\|\| true|MUTATION|mutat` over src/*.rs found nothing (exit 1). As a control, `grep -c "if "` on src/app.rs counted 7, so the search does read the file.
4. **The diff file is the real diff**: `git diff --stat dc75451 8814ced` matches the review package (6 files, +289/-16).

### New Breakage in the Fix Diff

- **Minor** (latent, not blocking), src/sound.rs:258-262 together with rodio-0.23.0/src/source/uniform.rs:55:
  - The mixer caps every span at 32768 samples, and 32768 is not a multiple of 6. When `FillChannels` reports a widened span above 32768 on a 5.1 output, the mixer re-bootstraps mid-frame.
  - This matters only when the device rate differs from the source rate. At equal rates `SampleRateConverter` is a pass-through (sample_rate.rs:133-136) and the 6-to-6 converter is a pass-through, so nothing happens.
  - When it does happen, the resampler frames straddle two output frames.
  - For the built-in alarm and chime (mono, 48 kHz, spans of 64 800 samples, so 388 800 once widened), every channel holds the same sample. The skew is at most one frame (about 21 µs) and cannot be heard.
  - For stereo `-s` files it needs a decoded packet of more than 5461 frames: mp3 and PCM wav packets are 1152 frames (symphonia-format-riff common.rs:28), and default FLAC and Vorbis packets are 4096 or fewer. So no common file hits it.
  - Cheap hardening if wanted: report `(len / from).min(32768 / to) * to`. It needs no action for this release.
- No Critical or Important issues.

### Out-of-Scope Observations

None.

### Verdict

**Fix round:** All findings addressed, no new Critical/Important breakage. Important 2 (CI on Linux and macOS) was not in this wave and is still open; it needs the developer's go-ahead to push.

## Executor rulings (from the SDD ledger)

- | T3/T7 | src/sound.rs tests, fixtures | T7 says fixture tests "unchanged" but also ".get()" on NonZero rate/channels. Ruling R1. |
- | T8 self | | Merge/push/tag need the developer. Ruling R2. |
- Ruling R1: T7 may adapt T3's assertions to compare through .get() (type forced), but no expected number changes — the plan's "unchanged" means values — costs a reviewer flag if wrong.
- Ruling R2: T8 is run by the controller (version bump, final review, merge, release with developer gates), not an implementer — every step past the bump needs the developer — cheap to redo if wrong.
- Ruling R3: work stays on branch deps-refresh in the main checkout (no worktree) — plan Global Constraints say so; single implementer at a time — none.
- Ruling R4: accept rodio 0.23's eprintln "error getting list of output devices" (stream.rs:243) — it fires only when the default device fails AND device enumeration fails, i.e. no usable audio at all, when the command is already failing with exit 3 / "no audio output available" on stderr; stdout JSON is untouched; avoiding it means copying rodio's fallback loop or enabling tracing (breaks the 7-feature set) — cost if wrong: one extra stderr line on audio-less machines; noted for NEXT.md and parking lot.
- Ruling R5: the full-screen no-device line gains the "no audio output available:" prefix — it is drawn over the alternate screen in both versions and has no test; accept, list in final-review minors.
- Ruling R4 WITHDRAWN: accepting the stderr line before review was pre-judging; the reviewer showed a silent fix fits inside open_output with no new dependency (rodio re-exports cpal), so Important 1 enters the fix loop.
- Task 7: minor (deferred): decode_detail drops rodio 0.23 IoError payload (add IoError(msg) arm).
- Task 7: minor (deferred): rodio 0.23 sends mono to first two channels only on >2-channel devices (5.1/7.1 lose centre/LFE/surround) — audible on surround outputs only; for hands-on + parking lot.
- Task 7: minor (deferred): Cargo.toml rodio comment's minimp3/slice-deque reason describes 0.16; reword as history.
- Task 7: minor (deferred): app.rs:219 no-device line has doubled prefix "Could not open output stream: no audio output available:".
- Ruling R6: fix Important 3 (surround upmix) in the fix wave with our own channel-mapping source wrapper rather than park it — the spec's anti-goal "any audible difference" is binding and the wrapper is device-free and testable — cost if wrong: ~40 lines to maintain until rodio's next release fixes it upstream.
- Ruling R7: Important 2 (CI on the branch) needs a branch push; ask the developer after the fix wave rather than merging untested on 3 OSes — cost: one round trip.
- Ruling R8: fix wave carries Important 1 + Important 3 + Minors 1, 2, 5 (the reviewer's must-fix list); the other 4 Minors (clap help spacing / one reworded usage error, no-device error losing cpal cause, 1-row terminal render, rodio mid-playback error wording) are accepted and listed for the developer — cost: those wordings differ from 0.2.1.
- Parked: span cap 32768 is not a multiple of 6-channel frames (sound.rs:258-262 vs rodio uniform.rs:55) — Ruling: real but latent, inaudible for the mono built-ins and needs >5461-frame stereo packets plus a device/file rate mismatch on a >2-channel output; no second fix wave; hardening `(len / from).min(32768 / to) * to` goes to the parking lot — cost if wrong: a rare mid-frame cut on surround outputs.
