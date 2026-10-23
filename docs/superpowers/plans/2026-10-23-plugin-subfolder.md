# Plugin Subfolder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the Claude Code plugin into `plugin/` so an install copies only the plugin, never `target/` or the source tree.

**Architecture:** The repo root stays the marketplace root (`.claude-plugin/marketplace.json`); the plugin (its manifest and `skills/`) moves under `plugin/`, and the marketplace entry's `source` becomes `"./plugin"`. No Rust code changes; one test path changes.

**Tech Stack:** Claude Code plugin manifests (JSON), Rust integration test (`tests/cli.rs`), `claude plugin` CLI.

**Spec:** `docs/superpowers/specs/2026-10-22-plugin-subfolder-design.md`

## Global Constraints

- Plugin folder is `plugin/`; manifest at `plugin/.claude-plugin/plugin.json`; skill at `plugin/skills/timer/SKILL.md`.
- Marketplace stays at `.claude-plugin/marketplace.json`; entry `"source": "./plugin"` (must start `./`, no `..`, forward slashes).
- Entry name and manifest name stay `cli-timer`; version stays `0.2.0`. No release, no version bump, no change to the skill's text.
- Historical files under `docs/superpowers/` keep the old paths.
- Work on a branch in the MAIN checkout (`G:\rustDev\cli-timer`), not a linked worktree: the installed marketplace is a directory source pointing at this folder, so Task 2 installs from the branch's files before merge. Do not edit `NEXT.md` on the branch.
- Repo allows squash merges only; the PR title (the squash subject) starts `handoff:`.

## Review Focus

1. **The root still holds `.claude-plugin/`** (marketplace only). `claude plugin validate .` warned that it treats a folder with a marketplace manifest as a plugin; after the move the root has no `plugin.json`, so validate must be run on both `.` (marketplace) and `plugin` (plugin) and both must pass. Covered in Task 1 Step 5.
2. **A stale cache masks the result.** The cache folder is keyed on version 0.2.0, so a reinstall without deleting `~/.claude/plugins/cache/cli-timer/` can reuse the 1.7 GB copy and look "installed". Task 2 deletes it and measures the new folder.
3. **The skill must still load and trigger** after the move, in a new session. Task 2 Step 5.
4. **GitHub installs resolve `./plugin` from a clone**, a different copy path from the directory install, and the README's route for everyone. Task 3 Step 4 installs from GitHub after the merge into an isolated config folder and checks the copy. Existing installs are unchanged by design.
5. **CI on all three OSes** runs the moved-path test (Windows path joins with `/` inside `concat!` work on all three today). Task 3 waits for green.

---

### Task 1: Move the plugin into `plugin/`

**Files:**
- Move: `.claude-plugin/plugin.json` → `plugin/.claude-plugin/plugin.json`
- Move: `skills/` → `plugin/skills/`
- Modify: `.claude-plugin/marketplace.json` (the entry's `"source"`)
- Test: `tests/cli.rs` `plugin_version_matches_crate` (~line 432)

**Interfaces:**
- Produces: the layout above, which Task 2 installs.

- [ ] **Step 1: Branch.** First push `main` so it equals `origin/main` (it was 2 commits ahead: `990d8db`, `7b06c33`; the developer approved the push); `git status -sb` must show no `ahead`. Then `git switch -c plugin-subfolder`. Claim the bead: `bd update clitimer-b1j --claim` (the Beads pre-commit shim stages the export into this branch's commit; that is expected).
- [ ] **Step 2: Point the test at the new path.** In `plugin_version_matches_crate`, the path becomes `concat!(env!("CARGO_MANIFEST_DIR"), "/plugin/.claude-plugin/plugin.json")` and the `expect` message names `plugin/.claude-plugin/plugin.json`. Assertion unchanged.
- [ ] **Step 3: Run it, expect failure.** `cargo test --test cli plugin_version_matches_crate` → FAIL, panic `read plugin/.claude-plugin/plugin.json`.
- [ ] **Step 4: Move and repoint.** `git mv .claude-plugin/plugin.json plugin/.claude-plugin/plugin.json` (create `plugin/.claude-plugin/` first), `git mv skills plugin/skills`, set the marketplace entry's `"source"` to `"./plugin"`.
- [ ] **Step 5: Verify.** `cargo test --test cli plugin_version_matches_crate` → PASS. `claude plugin validate .` and `claude plugin validate plugin` → both "Validation passed" (capture each exit code without a pipe). `git ls-files .claude-plugin plugin` lists exactly `.claude-plugin/marketplace.json`, `plugin/.claude-plugin/plugin.json`, `plugin/skills/timer/SKILL.md`.
- [ ] **Step 6: Full gates.** `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` → all pass (157 tests Unix / 156 Windows, unchanged count).
- [ ] **Step 7: Parking lot.** In `docs/parking-lot.md`, add to the MCP server mode entry that its `.mcp.json` must live in `plugin/` (a root `.mcp.json` becomes a project MCP config, not part of the plugin). Add an entry: installed plugin copies carry no `LICENSE` and `plugin.json` has no `license` field (GPL-3.0 per `LICENSE`; Cargo.toml has no `license` either), from the plan red team.
- [ ] **Step 8: Commit.** Stage the three moved/edited files plus `tests/cli.rs`, the spec, plan, both red-team reviews and `docs/parking-lot.md`. Message `refactor: move the Claude Code plugin into plugin/`, body with `clitimer-b1j` alone on a line.

### Task 2: Reinstall on the developer's machine and measure

Needs the developer only for the restart in Step 5.

- [ ] **Step 1: Record the before size.** `du -sk ~/.claude/plugins/cache/cli-timer` (expect ~1,716,000 KB).
- [ ] **Step 2: Uninstall and clear.** `claude plugin uninstall cli-timer@cli-timer`, then delete `~/.claude/plugins/cache/cli-timer/` (look at it first; it must hold only `cli-timer/0.2.0`).
- [ ] **Step 3: Refresh and reinstall.** `claude plugin marketplace update cli-timer`, then `claude plugin install cli-timer@cli-timer`.
- [ ] **Step 4: Measure.** `find ~/.claude/plugins/cache/cli-timer -type f` lists exactly `.claude-plugin/plugin.json` and `skills/timer/SKILL.md` under `cli-timer/0.2.0/`; `du -sk` of it is under 50 KB. `installed_plugins.json` shows `cli-timer@cli-timer` with the new `installedAt`.
- [ ] **Step 5: Skill loads.** Developer restarts Claude Code in this folder; the session's skill list includes `cli-timer:timer`, and "ding me when you've listed this folder" chimes with no prompt.

### Task 3: PR, merge, hand off

- [ ] **Step 1: PR.** Push the branch; `gh pr create` titled `handoff: move the Claude Code plugin into plugin/ (cache copies only the plugin)`; body summarises the move, the measured before/after cache size, and that existing 0.2.0 installs are unchanged.
- [ ] **Step 2: CI.** `gh pr checks --watch` → `check` green on Linux, Windows, macOS.
- [ ] **Step 3: Merge** (developer confirms first). Squash merge, keeping the `handoff:` title as the subject; `git switch main && git pull --ff-only`.
- [ ] **Step 4: GitHub install, isolated.** With `CLAUDE_CONFIG_DIR` set to a fresh empty folder in the scratchpad (never the developer's `~/.claude`): `claude plugin marketplace add ProgDroid/cli-timer`, `claude plugin install cli-timer@cli-timer`, then `find` the cache under that folder: exactly `.claude-plugin/plugin.json` and `skills/timer/SKILL.md` under `cache/cli-timer/cli-timer/0.2.0/`. Afterwards confirm the developer's `~/.claude/plugins/known_marketplaces.json` still shows the directory source (untouched), and delete the temp folder.
- [ ] **Step 5: Close and hand off.** `bd close clitimer-b1j`; commit the beads export on its own; update `NEXT.md` (manifest path in Housekeeping, remove the 1.7 GB note from "Things you might notice", Where we are) and commit it alone as `handoff: ...`.
