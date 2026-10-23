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
- **Migrate `tui` 0.19 to `ratatui`**: `tui` is unmaintained. Not needed for the agent work.
  2026-10-22, noted during the agent-timer brainstorm.
- **Detached reminders that outlive Claude Code** (`cli-timer start/list/cancel/stop`): the
  first-draft design, where cli-timer detaches its own process and tracks timers in files. Removes
  the background-task length cap and survives Claude Code closing. If built, each timer process
  must hold a lock on its file (so `list` never reports a dead timer as pending) and ids must never
  be reused (red-team review, `docs/superpowers/reviews/2026-10-22-agent-timer-design-redteam.md`).
  Deferred 2026-10-22, agent-timer brainstorm: the simpler design is a subset of this one.
- **Human `set` line has no date for reminders over a day**: `remind 30h` prints "Reminder set at
  16:39:46", which reads as today. Show the weekday or date when `fires_at` is not today (`src/output.rs`).
  JSON is fine (full RFC 3339). 2026-10-22, agent-timer build (final review m6).
- **Git Bash path-rewrites a label starting with `/`** on Windows (MSYS turns `/standup` into
  `C:/Program Files/Git/standup` for a native exe). Only the skill warns about it; no code fix known.
  2026-10-22, agent-timer build (final review m5).
- **The full-screen timer validates `-s` only at fire time**, then prints into its own alternate
  screen, so a bad file shows as a silent, negative timer. Cheap fix: `sound::check` in `run_timer`
  before `tui.init()`. Pre-existing. 2026-10-22, agent-timer build (final review m12).
- **CI runner retirement**: `release.yml` pins `ubuntu-22.04` for an older glibc; GitHub will retire
  that image, so move to a container or `ubuntu-24.04` then. `Swatinem/rust-cache` is only in
  `check.yml` now. 2026-10-22, agent-timer build (final review, ledger Task 8).
- **Release archive names carry no version** (`cli-timer-<target>.zip`); add the tag if people keep
  several. 2026-10-22, agent-timer build (ledger Task 8).
- **Windows and Linux release binaries are x86_64 only**: no arm64 Linux or Windows builds, and
  macOS x86_64 is cross-built on an arm64 runner. 2026-10-22, agent-timer build (ledger Task 8).
- **Upgrade `rodio`** (and re-check sound decoding of real mp3/flac/ogg files, which no test
  covers). 2026-10-22, agent-timer build (ledger Task 3).
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
- **Installed plugin copies carry no licence**: since the plugin moved to `plugin/`, an install
  copies only the manifest and the skill, not `LICENSE` (GPL-3.0), and `plugin.json` has no
  `license` field (`Cargo.toml` has none either). Add `"license"` to both if it matters.
  2026-10-23, red team of the plugin-subfolder plan
  (`superpowers/reviews/2026-10-23-plugin-subfolder-redteam.md`).
