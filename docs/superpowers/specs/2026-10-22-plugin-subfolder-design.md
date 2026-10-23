# Plugin in its own subfolder

Bead: `clitimer-b1j`. Short design (small change).

## Why

The marketplace entry's plugin source is `"./"`, the repo root. Claude Code copies a plugin's
whole directory into `~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/` at install, with
no exclude mechanism a plugin author can use (docs are silent on `.gitignore` and on any ignore
list; the copy left out `.git/` but kept the git-ignored `target/`). Installing from a
local checkout therefore copied `target/` too: 1.7 GB, measured 2026-10-22. A GitHub install has no
`target/` but still copies `src/`, `docs/`, `.beads/` and `tests/`, none of which the plugin uses.

## Goal

Any install, from GitHub or from a checkout, copies only the plugin: its manifest and
`skills/timer/SKILL.md`. Behaviour of the plugin and the binary is unchanged.

Non-goals: a release (the binary is unchanged), a version bump, changing the skill's content.
Merging does NOT change any existing install: the version stays 0.2.0, which names the cache
folder, so cached copies are never replaced (see "Existing installs"). The change protects future
installs; the developer's own 1.7 GB is removed by the one-off cleanup below.

Measured 2026-10-22: the developer's cache copy is 1,716 MB, of which `target/` is 1,706 MB; the
git-tracked files total about 0.5 MB (what a GitHub install copied before the move was not
measured). The move is worth little per GitHub install; its value is that a checkout install
cannot pull in `target/`. After the move (measured 2026-10-23, both from a checkout and from a
GitHub clone of the branch in an isolated config folder): 9 KB, exactly the manifest and the skill.

## Approaches

- **A. Plugin in a subfolder** (chosen by the developer). Move `plugin.json` and `skills/` under
  `plugin/`, point the marketplace entry at `"./plugin"`. This is the layout the Claude Code docs
  use in their own example (`"source": "./plugins/my-first-plugin"`).
- B. Keep the plugin at the root and move Cargo's build output out of the repo
  (`.cargo/config.toml` `build.target-dir`). Fixes the 1.7 GB only; still copies the source tree,
  and changes where every build lands. Rejected.
- C. A separate repo for the plugin. Splits the plugin-version-equals-crate-version check across
  repos. Dropped.

## The change

- `git mv .claude-plugin/plugin.json plugin/.claude-plugin/plugin.json`
- `git mv skills plugin/skills`
- `.claude-plugin/marketplace.json` stays at the root (the marketplace root); its entry's
  `"source"` becomes `"./plugin"`. Entry name and manifest name stay `cli-timer`.
- `tests/cli.rs` `plugin_version_matches_crate` reads `plugin/.claude-plugin/plugin.json`.
- `NEXT.md` housekeeping line names the new manifest path.
- Historical specs and plans under `docs/superpowers/` keep the old paths (they record what was
  true then).

Folder name `plugin/` is the agent's call.

Red-team review: `docs/superpowers/reviews/2026-10-22-plugin-subfolder-design-redteam.md`. The
developer kept approach A after reading it. The version coupling it raised (plugin version must
equal crate version, and the version names the cache folder) predates this change and went to the
parking lot.

## Existing installs

The cache directory is named after the manifest version, and an update does not replace a cached
copy whose version is unchanged. Installs already at 0.2.0 keep their old (larger) copy until the
next version bump; harmless, since the skill's content is identical.

The developer's own machine: uninstall the plugin, delete
`~/.claude/plugins/cache/cli-timer/`, update the marketplace, reinstall, restart.

## Done when

- `cargo test` and `cargo clippy --all-targets -- -D warnings` pass; CI green on the PR.
- After the reinstall, the cache folder holds only `.claude-plugin/plugin.json` and
  `skills/timer/SKILL.md` (measured, with its size), and the `cli-timer:timer` skill loads in a new
  session.
- Merged by squash with a subject starting `handoff:`.
