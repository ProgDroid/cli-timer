# Red-team review: plugin-subfolder implementation plan

Reviews `docs/superpowers/plans/2026-10-23-plugin-subfolder.md` against
`docs/superpowers/specs/2026-10-22-plugin-subfolder-design.md` (bead `clitimer-b1j`).
Stance: find why executing this plan fails. Reviewed 2026-10-23. Nothing in the repo or the
plugin install was changed; the new layout was simulated in a scratch copy (`git archive HEAD`).

## Evidence gathered (measured this session)

- **Local `main` is not the remote `main`.** `git ls-remote origin refs/heads/main` returns
  `87ede31`; local `main` is `7b06c33`. `git rev-list --left-right --count origin/main...main`
  gives `0 2`: two unpushed commits, `990d8db` (`.beads/issues.jsonl` only, adds the
  `clitimer-b1j` line) and `7b06c33` (`NEXT.md` only, the v0.2.0 handoff).
- **`pull.rebase` is `false`** (set in `C:/Program Files/Git/etc/gitconfig`).
- **The beads pre-commit hook stages the export.** `.beads/config.yaml` has `export.auto: true`,
  `export.git-add: true`, `interval: 1ms`; `.beads/PRIME.md:27-28` says the pre-commit shim
  "flushes and stages it".
- **Validate on the new layout** (scratch copy, files moved exactly as Task 1 Step 4 says):
  - `claude plugin validate .` → `✔ Validation passed`, exit 0 (marketplace only; today's
    root-`CLAUDE.md` warning and the "folder also holds a marketplace manifest" note disappear).
  - `claude plugin validate plugin` → `✔ Validation passed`, exit 0, **only when the copy is a git
    repo**. Before `git init`, the same command listed `README_MISSING` and `LICENSE_MISSING` as
    "problems the plugin directory refuses a submission for": the validator counts the root
    `README.md`/`LICENSE` only when it can find the repo root.
  - Today, `claude plugin validate .` prints `✔ Validation passed with warnings`, exit 0.
- **The cache copy, re-measured** (`du -sk`, KiB, `cache/cli-timer/cli-timer/0.2.0`): total
  1,716,129; `target/` 1,706,036; `sounds/` 8,136; `.beads/` 1,533; `src/` 129; `docs/` 116;
  `tests/` 16; `.github/` 12; `skills/` 8; `.claude-plugin/` 2; `.claude/` 2; top-level files 135.
  Parts sum to 1,716,125, residual 4 KiB (directory entries). Apparent size (`du -sb`):
  1,747,787,049 bytes. No `.git/` in the copy.
- **Tracked files today:** 388,836 bytes apparent, 465 KiB allocated (`git ls-files -z | du -c`).
- **Registry:** `known_marketplaces.json` has `cli-timer` as `"source": "directory"`, path
  `G:\rustDev\cli-timer`. `installed_plugins.json` has `cli-timer@cli-timer` 0.2.0,
  `gitCommitSha` `87ede31`. `~/.claude/plugins` is a symlink to
  `G:/Users/Nando Ferreira/Documents/dotfiles/ai/plugins/` (git-ignored there by `ai/plugins/*`).
- **Markers:** the cache holds 100 `.in_use` and 109 `.orphaned_at` entries in version dirs.
  `.in_use` is a directory of per-PID files written today by running sessions (e.g.
  `claude-plugins-official/claude-md-management/1.0.0/.in_use/{4216,46940,5200,5516}`). The current
  `cli-timer/0.2.0` dir has none.
- **Test count:** 157 `#[test]` across `src/` and `tests/cli.rs`; exactly one is `#[cfg(unix)]`
  (`src/alert.rs:382`), so 157 Unix / 156 Windows matches the plan.
- **Other references to the old paths** outside `docs/superpowers/`: `tests/cli.rs:434-435` and
  `NEXT.md:55` only (both handled by the plan). `README.md:77` tells users to
  `/plugin marketplace add ProgDroid/cli-timer` (GitHub). Neither workflow references the plugin.
- `LICENSE` is GPL-3.0; neither `plugin.json` nor `Cargo.toml` has a `license` field.

## Objection 1 (false assumption about the codebase): the branch is cut from an unpushed `main`, and the plan's own merge step then conflicts

Task 1 Step 1 runs `git switch -c plugin-subfolder` from local `main`, which is two commits ahead
of `origin/main` (verified with `ls-remote`, not just the stale tracking ref). The plan assumes
they are the same. Consequences, in order:

1. The PR carries `990d8db` and `7b06c33`. The squash folds the v0.2.0 handoff `NEXT.md` rewrite
   and a beads export into a commit titled "move the Claude Code plugin into plugin/". The global
   constraint "Do not edit `NEXT.md` on the branch" is broken without anyone editing it, and the
   PR body ("summarises the move") misdescribes the diff.
2. Step 1's `bd update clitimer-b1j --claim` rewrites `.beads/issues.jsonl` at once
   (`export.auto`, 1 ms), and the pre-commit shim stages it (`export.git-add: true`). So Step 7's
   "stage the three moved files plus ..." commit also contains the claim, whatever the agent
   stages. The repo's habit is a beads export in its own commit (`990d8db`).
3. Task 3 Step 3, `git switch main && git pull`: local `main` still holds the two originals,
   `origin/main` now holds the squash. Merge base `87ede31` has no `clitimer-b1j` line; local adds
   it as open (`990d8db`), the squash adds it as claimed. Same hunk, different content: with
   `pull.rebase=false` this is a merge with a **conflict in `.beads/issues.jsonl`** (NEXT.md merges
   clean only because both sides hold identical text). Task 3 Step 4's `bd close` and the
   `handoff:` NEXT.md commit then land on a conflicted or merge-commit `main`.

Fix: before Step 1, push `main` (or decide to drop those two commits and say so). Make Step 1
claim the bead and commit the export alone, or accept it in the refactor commit and say so.
After the merge, use `git pull --ff-only` so a divergence stops the run instead of merging.

## Objection 2 (does not deliver the spec): half of the goal is never measured

The spec's goal is "Any install, **from GitHub or from a checkout**, copies only the plugin". The
plan measures only a checkout install (Task 2). Review Focus 4 closes the GitHub half by
assertion: "the directory install exercises the same relative-source resolution, so no separate
GitHub test", then "After merge, nothing to check". The two routes differ in exactly the layer at
issue: the GitHub route clones the repo into `marketplaces/<name>` and copies from the clone, the
directory route copies from the working tree, and the one thing measured about the copier so far
(it skips `.git/`, ignores `.gitignore`) came from the directory route only. Two installs sharing
one assumption are one probe. And the GitHub route is the one the README gives every user other
than the developer, so it is the route the change is for.

It also cannot be tested casually on this machine: the GitHub marketplace and the directory one
both declare `"name": "cli-timer"`, so adding the GitHub one collides with the existing entry.

Fix: after merge (or after the push in Task 3 Step 1, if a branch-ref source works), install from
GitHub in an isolated config directory (e.g. `CLAUDE_CONFIG_DIR` pointing at a scratch folder,
`claude plugin install cli-timer --marketplace ProgDroid/cli-timer`), list the cache version dir
and record its size. Add it to "Done when", or narrow the spec's goal to checkout installs.

## Objection 3 (six months): nothing guards the new layout, and the root no longer means "the plugin"

After the move, the plugin's location is stated twice with nothing tying them together: the
marketplace entry's `"source": "./plugin"` and the hard-coded path in `plugin_version_matches_crate`.
CI (`.github/workflows/check.yml`) runs fmt, clippy and test only; no `claude plugin validate`, no
check that the source resolves to the manifest the test reads, no check that `plugin/` holds only
plugin files. Each of these regresses silently:

- **Root-level plugin components stop being plugin components.** The parking lot's next
  plugin-facing item (`docs/parking-lot.md:6-7`, MCP server mode) says it ships "through the same
  plugin's `.mcp.json`", written when the root was the plugin. After the move that file must be
  `plugin/.mcp.json`. Put at the root, as the old layout taught, it becomes a project MCP config
  for anyone opening the repo in Claude Code, and the plugin ships without it, with no error. The
  same holds for `hooks/`, `commands/`, `agents/`.
- **Anything gitignored placed under `plugin/`** (a built helper, a downloaded asset) is copied
  by the checkout install again, which is the bug this change fixes; no test catches it.
- **Licence and README stop shipping.** Every installed copy loses `LICENSE` (GPL-3.0) and
  `README.md`, and `plugin.json` has no `license` field. The validator flags exactly this
  (`LICENSE_MISSING`, `README_MISSING`) whenever it cannot see the repo root.

Fix: derive the test's path from `marketplace.json` (read `plugins[0].source`, join
`.claude-plugin/plugin.json`) so one edit cannot strand the other; add `"license": "GPL-3.0-only"`
(or whichever the project means) to `plugin.json`; give `plugin/` a one-line README pointing at
the root; add a parking-lot line that the MCP item's `.mcp.json` lives under `plugin/`.

## Fixed check: paired facts and the claim a reader will infer

| # | Sentence | Facts side by side | Inferred third claim | Verdict |
|---|---|---|---|---|
| F1 | Spec: "the developer's cache copy is 1,716 MB, of which `target/` is 1,706 MB; the git-tracked files total about 0.5 MB, which is roughly what a GitHub install copies today." | a total and a part; then a second figure | the ~10 MB left after `target/` is the tracked files, i.e. a checkout install minus `target/` is a GitHub install | **Unsupported implicature.** Residual decomposed (KiB): 1,716,129 − 1,706,036 = 10,093 = `sounds/` 8,136 + `.beads/` 1,533 + `src/` 129 + `docs/` 116 + top-level files 135 + `tests/` 16 + `.github/` 12 + `skills/` 8 + `.claude-plugin/` 2 + `.claude/` 2 + 4 (dir entries); sums. About 9,600 KiB (95%) of it is gitignored (`sounds/`, the Dolt DB). Also: "MB" here is thousands of KiB from `du -sk` (allocated); "about 0.5 MB" is the allocated 465 KiB, while the apparent tracked size is 0.39 MB; and "roughly what a GitHub install copies" was never measured (UNKNOWN). The design review recorded the full decomposition; the spec dropped it. |
| F2 | Spec "Why": "copies a plugin's whole directory ... with no exclude mechanism (docs are silent on `.gitignore` and on any ignore list)" | whole-directory copy + docs silent | nothing is ever excluded | **Unsupported, partly false, and a correction that did not propagate.** The cache has no `.git/` (re-checked today), so an exclusion exists. The design review flagged this (its I1); the spec still says it. |
| F3 | Plan Review Focus 4: "GitHub installs resolve `./plugin` from the clone; the directory install in Task 2 exercises the same relative-source resolution ... After merge, nothing to check: existing installs are unchanged by design." | GitHub resolves from a clone + directory install resolves relatively; existing installs unchanged | new GitHub installs are verified to copy only the plugin | **Unsupported.** No GitHub install is ever made. This is Objection 2. |
| F4 | Plan Global Constraints: "the installed marketplace is a directory source pointing at this folder, so Task 2 installs from the branch's files before merge" | directory source (verified) + branch checked out | the branch contains only this change | **Unsupported.** The branch also carries two unpushed `main` commits. This is Objection 1. |
| F5 | Plan Task 2 Step 4: "`find` lists exactly `.claude-plugin/plugin.json` and `skills/timer/SKILL.md` ...; `du -sk` of it is under 50 KB" | exact list + size bound | any other entry means the move failed | **Fragile, and a review point the plan did not absorb.** The design review (I7) asked for "no entries besides the plugin's files and Claude Code's own markers". This cache has 100 `.in_use/` dirs of per-PID files and 109 `.orphaned_at` files; `find -type f` would list `.in_use/<pid>` if a session marks the new dir before Step 4. The current `cli-timer/0.2.0` has none, so it may pass by ordering. |
| F6 | Spec Goal: "the version stays 0.2.0, which names the cache folder, so cached copies are never replaced" | version names the folder + version unchanged | an existing install is never touched or confused | Partly supported: the keying is visible (`installPath` ends `\0.2.0`). "Never replaced" and "harmless" for an install whose marketplace entry now says `./plugin` were not measured (design review I5, still open). |
| F7 | Plan Task 1 Step 6: "157 tests Unix / 156 Windows, unchanged count" | a count + "unchanged" | the moved test still runs | **Supported.** 157 `#[test]`, one `#[cfg(unix)]` (`src/alert.rs:382`). The red-then-green in Steps 3 and 5 is what proves the new path is read. |
| F8 | Plan Review Focus 5: "Windows path joins with `/` inside `concat!` work on all three today" | CI green + current test uses `/` | the new path works on Windows | **Supported** (`tests/cli.rs:434` already uses `/` and CI is green on Windows per `87ede31`). |
| F9 | Plan Review Focus 1: after the move "validate must be run on both `.` and `plugin` and both must pass" | two runs + both pass | the layout is correct | **Supported, with a condition.** Simulated: both print `✔ Validation passed`, exit 0, but `validate plugin` passes only when run inside the git checkout (outside it: `README_MISSING`, `LICENSE_MISSING`). Feeds Objection 3. |

F3 and F4 were promoted into Objections 2 and 1. F1 and F2 are spec text to correct before the
spec is committed in Task 1 Step 7.

## Smaller notes

- Task 1 Step 7 commits "the red-team review" (singular); there are now two
  (`2026-10-22-plugin-subfolder-design-redteam.md` and this file).
- The new install will record the branch tip as `gitCommitSha`; after the squash and branch
  deletion that SHA exists on no branch. What Claude Code does with it for a directory source is
  UNKNOWN.
- Task 2 deletes through the `~/.claude/plugins` symlink into the dotfiles tree. `~/...` as
  written is safe; an agent that rewrites it as unquoted `$HOME/...` splits on the space in
  `Nando Ferreira` and hands `rm -rf` two wrong paths. Keep the tilde or quote it.
- `claude plugin uninstall` deletes `~/.claude/plugins/data/<id>/` unless `--keep-data`; harmless
  for this skill-only plugin, but worth knowing before it gains state.
- Task 2 Step 1's expected figure is right: `du -sk` measured 1,716,129 KiB today.
