# Red team: v0.2.2 dependency refresh plan

2026-10-27. Reviews `docs/superpowers/plans/2026-10-27-dependency-refresh.md` against
`docs/superpowers/specs/2026-10-27-dependency-refresh-design.md` and the code on branch
`deps-refresh` (HEAD `8d619c1`). Nothing in the repo was changed except this file. All
experiments ran in throwaway crates in the session scratchpad: a tui 0.19 / ratatui 0.30.2 render
comparison, rodio 0.16 and rodio 0.23.0 decoders (each in its own crate so symphonia features do
not unify), a port of `src/sound.rs` to rodio 0.23.0 with its tests, and a `git archive` copy of the
repo with Task 4 applied.

Legend: **[V]** verified by source or by a command I ran; **[S]** suspected, not measured.

## Objection 1: Task 6 moves the time down one row at 80x24, and Task 2's tests are written so they cannot see it (a)

**[V]** I rendered `App::render`'s exact layout (`Percentage(49)`, `Length(1)`, `Min(1)`,
`src/app.rs:136`) through `TestBackend` on tui 0.19.0 and on ratatui 0.30.2 (ratatui-core 0.1.2,
ratatui-widgets 0.3.2), same text, same styles:

| Terminal | tui 0.19: time row | ratatui 0.30.2: time row |
|---|---|---|
| 80x24 (the default terminal size) | 11 | **12** |
| 120x40 | 19 | **20** |
| 80x25, 80x23, 80x7, 80x3, 10x2 | identical | identical |

Sweep at width 80, heights 2 to 80: 79 heights, **39** put the time on a different row and 40 match.
The 39 are every even height 4 to 50 (24), height 51 (1) and every odd height 53 to 79 (14):
24 + 1 + 14 = 39. The 40 are height 2 (1), every odd height 3 to 49 (24) and every even height 52
to 80 (15): 1 + 24 + 15 = 40. tui floors `0.49 * h` (24 -> 11.76 -> 11); ratatui rounds it (-> 12).
The paused, restart and label lines move with the time, since they sit on the row below it.

This violates the spec's first anti-goal ("Any ... visible difference: ... layout"), and the plan
has disarmed the oracle meant to catch it. Task 2 Step 1 says the row is "`24 * 49 / 100` rounded
as tui's layout does; find it by searching rows rather than hard-coding". Every Task 2 assertion is
relative to wherever the time is found (centred, the row below blank or `Paused`), so all of them
pass on both crates. Task 6 Step 3 then says "all Task 1 and Task 2 tests pass unchanged", which
will be true and will be read as "the screen is unchanged", which is false. The word "rounded" in
Task 2 even points at ratatui's answer (12), not tui's (11).

Fix:
- Task 2 must pin the absolute row on tui 0.19: row 11 at 80x24, row 12 at 80x25, row 19 at
  120x40, so that Task 6 goes red.
- Task 6 then has to keep the layout. **[V]** Replacing `Constraint::Percentage(49)` with
  `Constraint::Length(area.height * 49 / 100)` on ratatui 0.30.2 puts the time on tui's row for
  all 79 heights (0 differ in the same sweep).

## Objection 2: Task 7 changes the `-s` error text on stderr and in `--json`, while claiming "same messages" (a, b)

**[V]** `SoundError::Decode` is `"cannot decode sound file {path}: {source}"` (`src/sound.rs:66`),
where `{source}` is rodio's `DecoderError`. That text comes from rodio, and rodio 0.23 rewrote it:

| Case | rodio 0.16 (`decoder/mod.rs:555`ff) | rodio 0.23.0 (`decoder/mod.rs:765`ff) |
|---|---|---|
| not audio (measured on a text file) | `Unrecognized format` | `The format of the data has not been recognized.` |
| ADPCM WAV after Task 7 (measured) | decodes | `The format of the data has not been recognized.` |
| I/O error | the io message itself | `An IO error occurred while reading, writing, or seeking the stream.` (the message string is dropped from Display) |

That string reaches the user and agents unchanged: `remind.rs:65` and `ding.rs:28` do
`sound::check(..).map_err(|e| e.to_string())`, and `output.rs:87-101` writes it as `error: ...` on
stderr or as `{"error":"..."}` on stdout under `--json`. So every bad `-s` file gets a different
human line and a different JSON value. That goes against the spec Goal ("same human lines and JSON")
and against Task 7's Interfaces line ("`SoundError::{Open, Decode, Output}` kept with the same
messages"). Only the variant's format string stays the same. Nothing will catch it: the only
end-to-end assertion is `tests/cli.rs:229`, `stderr.contains("cannot decode sound file")`, a prefix.
The `Output` variant's `{0}` text changes in the same way (`StreamError` becomes `DeviceSinkError`,
with new Display strings).

The plan's own map is also wrong here **[V]**: `open_default_sink()` returns
`rodio::DeviceSinkError` (`stream.rs:233`, `456`), not `rodio::CpalError` (which is only the
re-exported `cpal::Error`, `lib.rs:186`). `log_on_drop` is `fn log_on_drop(&mut self, bool)`, which
returns `()` (`stream.rs:78`), so "`open_default_sink()?` then `.log_on_drop(false)`" cannot be
chained: it needs `let mut`. The compiler will catch both. The text change is the part no tool
flags.

Fix: in Task 3 (on rodio 0.16), pin the full decode-error line for a non-audio file. In Task 7,
either map `DecoderError` to cli-timer's own wording, so the text stops depending on rodio's Display
(and the I/O detail is kept), or record the new wording as a second deliberate change in the spec,
NEXT.md and the release notes.

## Objection 3: the "no stderr on drop" guarantee rests on a one-off grep across two copies of the sink code, and rodio says that API is going away (c)

**[V]** rodio 0.23's `MixerDeviceSink` `eprintln!`s on drop unless `log_on_drop(false)`
(`stream.rs:83-91`). The plan's only guard is Task 7 Step 4: one `grep` at one moment, plus a
hands-on check. Nothing in `cargo test` or CI keeps it true afterwards. Opening a device is
duplicated: `src/app.rs:218` (the alarm thread) and `src/sound.rs:189` (`play`). The plan keeps both
copies, so every future caller has to remember the flag on its own.

In six months, rodio's own source says the type the flag lives on will be replaced: "We have fixed
that in the new speakers API, which will eventually replace this" (`stream.rs:213`). The
experimental `speakers` module in 0.23 has no `log_on_drop` at all (`grep -rn log_on_drop src`
hits only `stream.rs`), and it has its own `eprintln!` error callback (`speakers/builder.rs:109`).
`docs/parking-lot.md:57-60` names "the real-format fixture tests from v0.2.2" as the oracle for that
next rodio pass, but those tests are device-free by design and cannot see drop logging. So the
guarantee Review Focus 2 calls critical is the one with no oracle when the code is next touched. The
"detached reminders" parking-lot item would also add a third sink. **[S]** The grep itself is weak
too: see the fixed check, item C.

Fix: one `sound::open_output() -> Result<(MixerDeviceSink, Player), SoundError>` that sets
`log_on_drop(false)`, used by both `play` and `App::start_sound`. Add a test that fails if any
other file names `DeviceSinkBuilder`/`open_default_sink` (a source scan in `tests/`, kept outside
the files it scans). Add to the parking-lot entry that the drop-logging check must be redone when
rodio's bigger release lands.

## Fixed check: paired facts and the claims they imply

**A. Review Focus 1 (plan line 28): "A key held down on Windows sends `Repeat` events; it must act
once, like a single press. Test: `repeat_events_change_nothing`."**
Implied: the held-key case on Windows is handled, and that test proves it. **Unsupported, and its
first fact is false [V].** crossterm 0.29.0 on Windows only ever produces `Press` or `Release`
(`event/sys/windows/parse.rs:223-226` and `286-289`: `if key_event.key_down { Press } else { Release }`).
OS auto-repeat arrives as more `key_down` records, so as more `Press` events. `Repeat` appears only
on terminals using the kitty keyboard protocol with `REPORT_EVENT_TYPES` pushed, which cli-timer
never does. So a held Space still toggles pause on every auto-repeat, as it does on 0.25 today. That
is not a regression, but the plan tells the final reviewer that it is fixed and tested.
`repeat_events_change_nothing` is a fine defensive test. It is not evidence about Windows. Fix:
reword the Review Focus line ("Release events must change nothing; held keys repeat as Press on
Windows, unchanged from today"), or, if acting once per hold is really wanted, record it as a new
behaviour in the spec, since it is not current behaviour.

**B. Task 6 Step 3 with Task 2 Step 1 ("find it by searching rows"): "all Task 1 and Task 2 tests
pass unchanged".** Implied: the screen is unchanged. **Unsupported [V]**: see Objection 1 (39 of
79 heights differ, including 80x24).

**C. Task 7 Step 4: "`grep -n "log_on_drop(false)" src/sound.rs src/app.rs` shows two hits."**
Implied: each of the two device openings is silenced. **Unsupported by the count.** The total of
two has to break down as 1 code hit in `sound.rs` + 1 code hit in `app.rs`. Two hits in `sound.rs`
(for example a doc comment that quotes the call, which Step 3's comment rewrite makes likely) plus
zero in `app.rs` also gives "two hits". Fix: assert one hit per file, outside comments, and that
the number of `open_default_sink` call sites (2 today: `app.rs:218`, `sound.rs:189`, counted from
`OutputStream::try_default`) equals the number of `log_on_drop(false)` hits.

**D. Task 7 Step 4: "`cargo tree -e features -i symphonia` lists no `mkv`, `adpcm` or `isomp4`."**
Implied: only the four chosen formats decode. **Supported today, but not by this probe [V].** With
the plan's seven features, rodio 0.23.0 enables symphonia `flac`, `mp3`, `ogg`, `pcm`, `vorbis`,
`wav` and nothing else (measured), so the set is right now. But the check only names three things
that must be absent, and `aac`, `alac`, `caf`, `aiff`, `mp1` and `mp2` (all rodio 0.23 features) would
pass it. Fix: compare the feature set as an enumeration against the expected list.

**E. Task 1 line 61: "`App::restart` sends on `app.sender` only if set; `App::default()` has
none, so no sound thread starts."** Implied: the test is device-free. **Supported, but for another
reason [V].** `restart` never starts a sound thread whatever the sender is (`src/app.rs:189-202`).
Only `tick` reaching zero does, through `start_sound`. The conclusion holds.

**F. Task 8 Step 4: "on this Windows machine the native build plus CI's Windows job cover" the
Windows-GNU clippy check.** Implied: the spec Goal "the Windows-GNU clippy check passes" is met.
**Unsupported as stated [V]:** both are `x86_64-pc-windows-msvc` (`check.yml:17` `windows-latest`,
`release.yml:46`). Low risk, since no `target_env` cfg exists in `src/` (grep: no hits) and cpal and
rodio do not differ by env in what they compile, but this is a dropped spec goal. Either run it or
amend the spec.

**Residuals, broken down:**
- Height sweep: 79 = 39 differ (24 + 1 + 14) + 40 match (1 + 24 + 15). See Objection 1.
- 15-colour list (Task 5): 14 explicit arms + `_ => Red` for index 0 = 15 (`src/app.rs`
  `random_color`); `Black` and `Reset` are absent.
- Fixture decode counts against the 11 025 expected (0.5 s at 22 050 Hz), measured:
  - mp3 on 0.16: 12 672 = 22 frames x 576 (MPEG-2 Layer III). The surplus of 1 647 = 1 105
    encoder delay + 542 padding to the frame boundary (the split is **[S]**, from LAME's usual
    576 + 529 delay; the total is **[V]**). On 0.23: 11 025 exactly (gapless trimming).
  - ogg: 11 264 on both = 11 x 1 024; surplus 239 = padding to the Vorbis block.
  - flac: 11 025 on both, surplus 0.
  - adpcm wav on 0.16: 12 216 = 6 blocks x 2 036 samples; surplus 1 191 = padding of the last block.
    On 0.23: rejected as `Decode` (`UnrecognizedFormat`).
  - The plan's tolerance (plus or minus 2 205) holds on both versions: worst case 1 647. Note that
    0.23's gapless trimming removes about 75 ms (1 647 samples at 22 050 Hz) of silence from each
    loop of an mp3 `-s` file in the full-screen timer. That is an audible timing change in the
    spec's terms, small and arguably a fix, and no test pins it. Mention it in the release notes.

## Checked and found sound

- Task 1: crossterm 0.25 has `KeyEvent::kind`, `KeyEventKind::{Press, Repeat, Release}`,
  `new_with_kind`, and `new` gives `Press` (`event.rs:608-679`) **[V]**. The expected red/green
  split in Step 2 follows from `src/handler.rs:9-50` **[V]**.
- Task 2: the 86-column restart prompt overflows 80 columns the same way on both crates (starts at
  column 0, keeps the first 80) **[V]**. 10x2 renders the same on both; 1x1 does not panic on either
  (the only cell's `bg` differs, `Reset` against `Black`, which is negligible) **[V]**.
- Task 3: the plan's ffmpeg command works with the local ffmpeg 4.4.1 (all four encoders present).
  The four files are 2.5 to 11.9 KB, under 16 KB, and byte-identical on a rerun **[V]**. All of
  mp3, flac and ogg, plus the no-extension and `.MP3` copies, decode with the right channels and
  rate on 0.16 and on 0.23 **[V]**.
- Task 4: on a `git archive` copy, `cargo update -p clap -p chrono` gives clap 4.6.7 and chrono
  0.4.45. `cargo test` passes (130 unit + 33 end-to-end, with two 0-test harnesses) and
  `cargo clippy --all-targets -- -D warnings` is clean **[V]**.
- Task 5: rand 0.10.3 exports `rng()` and puts `random_range` on `RngExt` (`rng.rs:56`, `163`), not
  `Rng`. The plan's "check the trait" hedge covers it **[V]**.
- Task 6: `ratatui::crossterm` is re-exported (`ratatui` `lib.rs:483`). The default features select
  crossterm 0.29 only, so the tree has one crossterm, 0.29.0 **[V]**. Minor: tui's `Buffer::get(x, y)`
  is `#[deprecated]` in ratatui-core 0.1.2 (`buffer.rs:129`), so the Task 2 helpers need
  `buffer[(x, y)]` and not only `symbol()`, or `-D warnings` fails.
- Task 7: with the plan's map (corrected for `DeviceSinkError` and `let mut`), all 16 existing
  `src/sound.rs` tests pass unchanged on rodio 0.23.0, including the sample-exact alarm, chime,
  `once` and `loop` counts, with only `.get()` added **[V]**.
- Global constraint "Do not edit `NEXT.md`" contradicts Task 8 Steps 3 and 7, which edit it. That is
  probably meant as "not before Task 8"; say so.
