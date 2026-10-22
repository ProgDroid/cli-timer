# cli-timer

Rust CLI countdown timer: `tui` 0.19 + crossterm for the full-screen UI, rodio for sound.

## Commands
- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-targets -- -D warnings` (policy lives in `Cargo.toml` `[lints.clippy]`)
- Test: `cargo test` (unit tests in each `src/` file, end-to-end tests in `tests/cli.rs` that run the real binary). Tests are silent and need no `CLI_TIMER_QUIET` in your shell: unit tests never build the system alerter, and the end-to-end helper sets `CLI_TIMER_QUIET=1` on every process it spawns. Set `CLI_TIMER_QUIET=1` yourself when you run the binary by hand and don't want a pop-up or sound.
- Linux builds need ALSA headers: `libasound2-dev` and `pkg-config`
- Windows check without CI (e.g. in a cloud session): `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` (needs `rustup target add x86_64-pc-windows-gnu` and the `gcc-mingw-w64-x86-64` package). Compiles and lints only; no tests.
- CI: `.github/workflows/check.yml` (fmt, clippy, test on Linux, Windows, macOS, every push and PR) and `.github/workflows/release.yml` (a pushed `vX.Y.Z` tag builds Windows, macOS arm64/x86_64 and Linux archives and publishes a release; a manual run is a dry run that only uploads the archives as workflow artifacts). Before pushing a tag, do a manual run and check all four archives appear.

## Decisions
Design docs live in `docs/superpowers/specs/`.

## Working rules

- **Parking lot.** An idea that is out of scope for the current work goes to
  `docs/parking-lot.md` (dated, with where it came from); that includes anything a spec or plan
  would call "deferred" or "later" — the spec or plan may point at the entry but must not be the
  only place it lives. Read `docs/parking-lot.md` when scoping or brainstorming the next piece of
  work, not at session start. Notes about ANOTHER project don't go here: use
  `sb add --project <x> "…"` once the `sb` CLI provides it; until then put them in that project's
  parking lot if reachable, else tell the user.

## Session routine

- **Handoff.** This repo follows the `handoff` skill (`NEXT.md` plus `handoff:` commits); read it for the rules.
- Session start: ask the `.claude/handoff.json` question (default: "How did the last change go?") and log the dated answer in `NEXT.md`'s feedback log.
- Then show what changed since the newest commit whose subject starts `handoff:`.
- Session end: update `NEXT.md` and commit it on its own, with a subject starting `handoff:`.
- A squash merge to the default branch also gets a subject starting `handoff:`.
