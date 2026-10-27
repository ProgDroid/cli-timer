# v0.2.2 dependency refresh: design

2026-10-27. A small change in what it does (one deliberate narrowing, below) with API churn
underneath; agreed in conversation, written down here. Red team:
`../reviews/2026-10-27-dependency-refresh-design-redteam.md` (its three objections were verified and
are answered in "Decided" and "Goals").

## Intent

Bring every direct dependency up to its current release, so the project is not sitting on an
unmaintained UI crate (`tui`, last release 2022-08) and on `rodio` 0.16 (2022). No defect in either
is driving this; it is upkeep. Ships as **0.2.2** (agent's call, developer agreed). Resolves the
parking-lot entries "Migrate `tui` 0.19 to `ratatui`" and "Upgrade `rodio`".

## What moves (measured on crates.io, 2026-10-27)

| Dependency | Now (lock) | Target | Kind of change |
|---|---|---|---|
| `tui` -> `ratatui` | 0.19.0 | 0.30.2 | crate swap; `src/app.rs`, `src/tui.rs`, `src/main.rs` |
| `crossterm` | 0.25.0 | 0.29.0 | follows ratatui; `src/event.rs`, `src/handler.rs`, `src/tui.rs`; **changes key events on Windows** (below) |
| `rodio` | 0.16.0 | 0.23.0 | breaking redesigns in 0.21, 0.22, 0.23; `src/sound.rs`, `src/app.rs` (alarm loop thread) |
| `rand` | 0.8.5 | 0.10.3 | renamed calls; `random_color` in `src/app.rs` |
| `clap` | 4.1.1 | 4.6.7 | semver-compatible; lock update, raise the `Cargo.toml` floor |
| `chrono` | 0.4.23 | 0.4.45 | semver-compatible; lock update, raise the floor |

Already current: `serde`, `serde_json`, `thiserror` 2, `notify-rust`. "Current" means the direct
dependencies only: transitive crates follow whatever their parents ask for (for example
`thiserror` 1.x stays in the lock, required by `ndk`, which rodio 0.23's audio backend pulls in on
Android targets). The toolchain stays at `1.99` (stable is 1.99.1); the highest `rust-version` among
the targets is 1.89 (`notify-rust`), below it.

**rodio 0.23 is a stopgap upstream** ("a minimal release for users blocked on old cpal version
while we are working on a bigger release", its CHANGELOG). The developer chose to upgrade now
anyway (2026-10-27), accepting that the sound code may be touched again when that release lands; a
parking-lot entry records it.

## Goals

- cli-timer behaves as today on the new dependencies:
  - the full-screen timer: same layout, black background, random colour, centred time with the
    `-` prefix once triggered, the paused / restart-confirm / label lines, the looping alarm with
    its 500 ms fade-in, stopping on restart and exit;
  - **keys act once per press, on every OS.** crossterm 0.25 drops key-release events on Windows
    (`event/sys/windows/parse.rs`, `if !key_event.key_down { return None; }`); 0.26+ delivers them
    as `KeyEventKind::Release`, and `src/handler.rs` does not look at `kind`. Unfixed, on Windows
    one press of `r` would restart without confirming, Space would pause and resume at once, and
    Esc on the restart prompt would quit. The handler must act on `KeyEventKind::Press` only;
  - `remind` and `ding`: same human lines and JSON, same exit codes (0 / 1 / 3), same sounds, same
    ring and cut-off times;
  - a bad `-s` file is still rejected before anything starts, without opening an audio device.
- **Tests written on the old code first**, so each upgrade is checked against an oracle that
  predates it:
  - key-handler tests (each key in each state, and that a `Release` event of any key changes
    nothing; the Release test is the one expected to fail on the new crossterm until the fix);
  - screen tests through ratatui's / tui's `TestBackend` (the time line, the `-` prefix, the paused,
    restart and label lines), written on `tui` 0.19 before the swap;
  - real-format sound tests (mp3, flac, ogg vorbis, wav), written on rodio 0.16 before its upgrade.
- fmt, clippy (`-D warnings`, pedantic + nursery) and tests green after every upgrade commit; CI
  green on Linux, Windows and macOS; the Windows-GNU clippy check passes.
- v0.2.2 published: both version fields bumped, a release dry run shows four archives, then the tag.

## The one deliberate change: `-s` formats

Today rodio 0.16 turns on symphonia's default features, so `-s` also decodes MKV/WebM audio and
ADPCM WAV (`cargo tree -e features -i symphonia`). rodio 0.23 no longer does that. **The developer
chose the four named formats only** (2026-10-27): mp3, flac, ogg vorbis, and PCM wav. In rodio 0.23
terms that is `symphonia-mp3`, `symphonia-flac`, `symphonia-ogg` + `symphonia-vorbis`,
`symphonia-wav` + `symphonia-pcm` (plus `playback`), with default features off. An MKV/WebM or ADPCM
file that plays today is rejected at start with the usual "cannot decode" error. NEXT.md and the
release notes say so. Keep decoding through symphonia (the `Cargo.toml` comment: the old minimp3
decoder aborts debug builds on a non-MP3 file); update that comment to the new feature list.

## Non-goals

- New features the new crates make possible (for example ratatui's big-digit text).
- Other parking-lot items: ARM builds, archive names, runner retirement, MCP, detached reminders.
- Restructuring beyond what the new APIs need, and pinning transitive crates.

## Anti-goals (a failure even if every test passes)

- Any audible or visible difference: alarm, chime, fade, loudness, layout or colour.
- New text on stderr or stdout. Newer rodio may log when an output stream is dropped; that must not
  reach the terminal under `remind`, `ding`, `--json` or the full-screen timer. Check the installed
  rodio source for the exact behaviour and switch it off, rather than trusting docs.
- Tests that need an audio device or play sound (`check` and `source` must stay device-free).
- A key acting twice, or on release, on any OS.

## Approaches considered

- **A. One branch, one upgrade per commit, tests first (picked).** Order: the old-code tests above
  (keys, screen, sound formats); `clap` + `chrono` lock updates; `rand`; `crossterm` + `ratatui`
  together, with the Press filter; `rodio` last and alone. Green after each, so a later regression
  points at one upgrade. Squash-merged into `main` as one change, subject starting `handoff:`.
- **B. Everything at once.** Slightly quicker; a regression could not be traced to one crate, and
  the new tests would never have passed on the old code.
- **C. Two releases** (screen side, then rodio). Two release rounds and hands-on checks for a
  change that does not warrant it.
- **Red team's 80% option: re-park rodio until its bigger release.** Offered; the developer chose to
  upgrade now.

## Decided

- Approach A; version 0.2.2; release published at the end (developer, 2026-10-27).
- Every direct dependency, not only `tui` and `rodio` (developer).
- rodio to 0.23 now, despite its stopgap status (developer).
- `-s` formats: the four named only (developer).

## Left to the builder (agent's calls)

- **crossterm through ratatui's re-export** (`ratatui::crossterm`), dropping the direct dependency,
  so the two can never disagree. If the re-export lacks something we use, keep a direct `crossterm`
  pinned to the version ratatui depends on.
- **rand: upgrade to 0.10** rather than replace it with std (the red team's suggestion): the
  developer asked for dependencies brought up to date, not removed; dropping it is a later call.
- **Fixtures** in `tests/fixtures/` (or wherever the unit tests can reach them), generated by us
  with the local `ffmpeg` from a synthesised tone, so there is no licence question; about a second,
  mono, a few KB each; the exact commands in a `README` next to them; marked binary in
  `.gitattributes` (`core.autocrlf` is on here). Assertions: passes `check`, decodes, expected
  channels and sample rate, decoded length close to the expected duration (mp3 and vorbis pad, so a
  tolerance, not an exact count). One more: an ADPCM WAV or MKV fixture is rejected after the
  rodio upgrade, pinning the chosen format set.
- How to express the built-in alarm and chime in rodio 0.23, as long as the existing sample-exact
  tests in `src/sound.rs` still pass, changing only where a type name forces it.
- The `SAMPLE_RATE` doc comment that cites "rodio 0.16's `SineWave`".

## Hands-on check for the developer (after the merge, before the tag)

Install from the checkout, then on Windows: the full-screen timer with the built-in alarm, and with
an mp3 `-s` file; press Space (pauses, stays paused), Space again (resumes), `r` (asks to confirm),
Esc (back to running, does not quit), `r` `r` (restarts), `q` (quits). Then `remind 5s -l test`
(pop-up, 30 s alarm, nothing extra printed), `ding "done"`, and `remind 5s --json` (one JSON object
per line, nothing else).
