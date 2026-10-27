# Parking lot

Ideas that are out of scope for the current work. One bullet per idea: the idea, the date, where it
came from, and a pointer to a detail doc when there is one.

- **MCP server mode** (`cli-timer mcp`, stdio) exposing remind/ding (and list/cancel, if the detached mode exists) as typed tools,
  shipped through the same plugin's `.mcp.json` (it must live in `plugin/`, next to the plugin's
  `.claude-plugin/`: a root `.mcp.json` would be a project MCP config, not part of the plugin). Deferred: the CLI + skill covers it for now; add
  it if per-command permission prompts or command wording become a nuisance. The core is a library
  so this stays a thin layer. 2026-10-22, agent-timer brainstorm
  ([design](superpowers/specs/2026-10-22-agent-timer-design.md)).
- **Timers that survive a restart or log-off, and absolute times** ("tomorrow at 9") via the OS
  scheduler (Task Scheduler / launchd / systemd). Out of scope: reminders are within-a-sitting.
  2026-10-22, agent-timer brainstorm.
- **Detached reminders that outlive Claude Code** (`cli-timer start/list/cancel/stop`): the
  first-draft design, where cli-timer detaches its own process and tracks timers in files. Removes
  the background-task length cap and survives Claude Code closing. If built, each timer process
  must hold a lock on its file (so `list` never reports a dead timer as pending) and ids must never
  be reused (red-team review, `docs/superpowers/reviews/2026-10-22-agent-timer-design-redteam.md`).
  Deferred 2026-10-22, agent-timer brainstorm: the simpler design is a subset of this one.
- **Git Bash path-rewrites a label starting with `/`** on Windows (MSYS turns `/standup` into
  `C:/Program Files/Git/standup` for a native exe). Only the skill warns about it; no code fix known.
  2026-10-22, agent-timer build (final review m5).
- **CI runner retirement**: `release.yml` pins `ubuntu-22.04` for an older glibc; GitHub will retire
  that image, so move to a container or `ubuntu-24.04` then. `Swatinem/rust-cache` is only in
  `check.yml` now. 2026-10-22, agent-timer build (final review, ledger Task 8).
- **Release archive names carry no version** (`cli-timer-<target>.zip`); add the tag if people keep
  several. 2026-10-22, agent-timer build (ledger Task 8).
- **Windows and Linux release binaries are x86_64 only**: no arm64 Linux or Windows builds, and
  macOS x86_64 is cross-built on an arm64 runner. 2026-10-22, agent-timer build (ledger Task 8).
- **Leading `-` in other values**: a label or `ding` message starting with `-` now works (the `-l`
  of `remind` and the `ding` message take hyphen-leading values), but the full-screen timer's `-l`
  and `-s <FILE>` values do not. Add `allow_hyphen_values` there if it ever matters.
  2026-10-22, agent-timer build (final review m4).
- **Test helper duplication**: the WAV writer exists three times (`src/sound.rs`, `src/ding.rs`,
  `tests/cli.rs`), plus `json_lines`/`log_of`; the temp WAVs are never removed. A shared test module
  is not worth it yet. 2026-10-22, agent-timer build (final review m8).
- **Plugin version is tied to the crate version**: `tests/cli.rs` `plugin_version_matches_crate`
  forces them equal, and the plugin version names Claude Code's cache folder, so an install only
  refreshes on a version change. A skill-only fix therefore needs a crate bump and a release, and
  between bump and tag a fresh install can get a skill that expects an unreleased binary. Options:
  decouple the versions, or drop the manifest `version` so git installs key on the commit SHA.
  2026-10-23, red team of the plugin-subfolder design
  (`superpowers/reviews/2026-10-22-plugin-subfolder-design-redteam.md`).
  **Considered for v0.2.1 and kept coupled (2026-10-23)**: dropping `version` was picked, then
  reversed after its red team (`superpowers/reviews/2026-10-23-v0.2.1-polish-design-redteam.md`).
  Facts from that round: the developer's local-path install loads in place and ignores `version`
  (Claude Code plugin loading docs), so only GitHub installs gain; the repo had 0 stars/forks;
  auto-update is off for this marketplace. Revisit when a real skill-only fix exists. If revisited:
  a "skill minimum <= crate version" test does NOT prove `main` only asks for released binaries,
  and the MCP server entry above would re-tie plugin and binary (its server skips the skill's
  version check).
- **rodio's next big release**: v0.2.2 moved to rodio 0.23.0, which its CHANGELOG calls "a minimal
  release ... while we are working on a bigger release". When that release lands, expect the sound
  code (`src/sound.rs`, the alarm thread in `src/app.rs`) to need another pass; the real-format
  fixture tests from v0.2.2 are the oracle for it. 2026-10-27, dependency-refresh red team
  (`superpowers/reviews/2026-10-27-dependency-refresh-design-redteam.md`).
  rodio 0.23's source says its new `speakers` API "will eventually replace" the device sink
  (`stream.rs`), and that API has no `log_on_drop`. Today `sound::open_output` switches off the
  line rodio prints to stderr when the sink is dropped; after the move, re-prove by hand that
  `remind 5s --json` with sound prints nothing extra, because the fixture tests cannot see it.
  2026-10-27, dependency refresh (Task 7).
  Two more copies to re-check then (2026-10-27, final review of the dependency refresh):
  `open_output` copies the device fallback of rodio 0.23's `open_default_sink` (`stream.rs`) to
  avoid its stderr line; compare it with the new release's, and drop the copy if the print is
  gone. `fit_channels` copies rodio 0.16's channel mapping because 0.23 leaves channels 3 and up
  silent; if the new release fills them (the last channel into every extra one), drop it.
- **Surround channel-fill span length**: `FillChannels` (`src/sound.rs`) reports a widened span of
  `len / from * to` samples, but rodio 0.23 caps spans at 32 768 samples, which is not a whole
  number of 6-channel frames, so a long widened span on a 5.1 output can be cut mid-frame. Only
  matters when the device's sample rate differs from the file's and a stereo `-s` file has packets
  over 5 461 frames (mp3/wav use 1 152, default FLAC/Vorbis 4 096 or fewer); inaudible for the mono
  built-ins. Hardening: report `(len / from).min(32768 / to) * to`. 2026-10-27, v0.2.2 final fix
  re-review.
