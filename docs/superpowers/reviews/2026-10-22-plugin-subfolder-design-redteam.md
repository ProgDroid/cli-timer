# Red-team review: plugin in its own subfolder

Reviews `docs/superpowers/specs/2026-10-22-plugin-subfolder-design.md` (bead `clitimer-b1j`).
Stance: find why this is the wrong thing to build. Reviewed 2026-10-22.

## Evidence gathered (measured this session)

- **Who installs from a checkout.** `~/.claude/plugins/known_marketplaces.json` has the `cli-timer`
  marketplace as `"source": "directory", "path": "G:\\rustDev\\cli-timer"`. `installed_plugins.json`
  has one install, `cli-timer@cli-timer` 0.2.0, `gitCommitSha` 87ede31. The README tells every
  other user to run `/plugin marketplace add ProgDroid/cli-timer` (GitHub).
- **The cache copy, decomposed** (`du -sb`, `~/.claude/plugins/cache/cli-timer/cli-timer/0.2.0`):

  | Part | Bytes | Tracked in git? |
  |---|---:|---|
  | `target/` | 1,737,589,582 | no (gitignored) |
  | `sounds/` | 8,309,541 | no (gitignored) |
  | `.beads/embeddeddolt` + `.beads/backup` (the local Dolt issue DB) | 1,500,278 | no |
  | rest of `.beads/` | 21,540 | mostly |
  | every other top-level entry | 366,108 | yes |
  | **total** | **1,747,787,049** | sums exactly |

  The plugin itself is 6,994 bytes (`skills/` 6,214 + `.claude-plugin/` 780, and the latter
  includes `marketplace.json`).
- **`.git/` was NOT copied.** The checkout has `.git/` (1.3 MB); the cache has none. Gitignored
  `target/`, `sounds/` and the Dolt DB were copied. So Claude Code applies at least one exclusion
  (`.git`), but does not honour `.gitignore`.
- **What a GitHub install would carry.** All tracked files total 388,223 bytes. Minus the
  6,994-byte plugin, the waste per GitHub install is about 381 KB. (Not measured on a real GitHub
  install: whether that path also drops `.git/` is UNKNOWN, though the local copy suggests it does.)
- **Claude Code writes into the version dir.** Elsewhere in this machine's cache there are 106
  `.orphaned_at` files and `.in_use` markers inside version dirs
  (e.g. `cache/android-skills/android-skills/1.0.15/.in_use`).
- `plugin_version_matches_crate` (`tests/cli.rs:433`) pins `plugin.json` version to the crate
  version. Neither workflow touches `skills/` or `.claude-plugin/`.

## Objection 1 (simpler design): stop installing from the checkout; the move buys about 381 KB

99.42% of the 1.7 GB is `target/`, and the remaining 10.2 MB is also gitignored. All of it
reaches the cache only through the one directory-source marketplace, which is the developer's
own. The zero-change fix: re-add the marketplace from GitHub (the command the README gives every
user), delete `cache/cli-timer/` once, and use `claude --plugin-dir` for local skill testing
(check that flag loads the plugin in place before relying on it). That removes 100% of the measured
problem without moving files, editing the test or making old docs point at stale paths.

What the design adds beyond that is the GitHub-install residual, about 381 KB per install per
version. The spec presents that residual as a list of directories (`src/`, `docs/`, `.beads/`,
`tests/`) rather than a number. The list also leaves out `Cargo.lock` (72.5 KB) and `LICENSE`
(35.8 KB), the two largest files after `src/` and `docs/`.

If the move is still wanted for tidiness, call it tidiness and size it as such. It is not a fix
for a 1.7 GB bug.

## Objection 2 (assumed requirement): no existing install improves, and checkout installs are not a dev loop

The goal says "any install, from GitHub or from a checkout". Two parts of that are assumed:

1. **That merging delivers anything.** The non-goals say no release and no version bump are
   needed because "marketplace installs track the default branch, not tags". But the spec's own
   "Existing installs" section and the docs say the cache is keyed by version and an update does
   not replace a cached copy at the same version. With no bump, the change reaches no existing
   install. The one install that is actually bloated, the developer's, is fixed by the manual
   step "delete `~/.claude/plugins/cache/cli-timer/`", and that step works with or without the
   move. The move only stops the bloat from coming back on the next checkout install.
2. **That checkout installs need supporting at all.** Because the cache is keyed by version, a
   checkout install is a one-off snapshot: later edits to `SKILL.md` in the checkout never reach
   it until `plugin.json`'s version changes, and the test ties that version to the crate's. So
   the path the design optimises cannot serve as an edit-and-retest loop. It has one user, who
   uses it at most once per release. Nobody has shown it needs to be a supported path. If it does
   (for example to try a skill before pushing), `--plugin-dir` is the tool built for that.

## Objection 3 (six-month collision): the plugin version is still the crate version

The subfolder makes the plugin look independent, but its version is still forced to equal the
crate's (`plugin_version_matches_crate`), and that version is the cache key. Put those together
with "installs track the default branch" and you get this:

- **A skill-only fix ships only with a crate release.** A wording fix or new guidance in
  `SKILL.md` reaches existing users only after a version bump. The only way to bump is to bump the
  crate, which means the full release ritual (dry run, four archives, tag) for a change that
  touches no Rust. The likely outcome: skill fixes sit on `main` unseen, or the crate version
  bumps for non-binary changes. The parking lot already lists skill-touching work (MCP mode via
  the plugin's `.mcp.json`, detached reminders with list/cancel), so this will come up.
- **There is a window between the bump and the tag.** NEXT.md's release order is: bump `version`
  in both files (squash-merged to `main`), then do a dry run, then push the tag. In that window, a
  fresh GitHub plugin install gets a skill that may require the new binary version before the
  Releases page has it. `cargo install --git` users are unaffected because it builds `main`.
- Approach C was dropped *because* it would split this check. The spec never establishes that
  plugin version == crate version is a requirement and not just a habit. That coupling, not the
  folder layout, is what will cost time.

Recommendation: if anything ships here, it should be a decision about plugin versioning (an
independent plugin version, or a documented "skill change => bump" rule with a cheap path for
it). The folder move is optional on top.

## Fixed check: paired facts and the claim a reader will infer

| # | Sentence (spec) | Facts set side by side | Inferred third claim | Verdict |
|---|---|---|---|---|
| I1 | "copies a plugin's whole directory ... with no exclude mechanism (docs are silent on `.gitignore` and on any ignore list)" | the copy is whole-directory + the docs are silent | Claude Code excludes nothing | **Unsupported, and partly false.** The docs being silent is not evidence of absence. The cache measurably lacks `.git/`, so an exclusion exists. What is established: `.gitignore` is not honoured (`target/`, `sounds/` and the Dolt DB were all copied). Whether a user-configurable exclude exists is UNKNOWN, not ABSENT. |
| I2 | "Installing from a local checkout therefore copied `target/` too: 1.7 GB, measured" (the bead says "all of it target/") | target/ was copied + the cache is 1.7 GB | the 1.7 GB is `target/` | **Supported to 99.42%.** The remainder, decomposed until it sums: `sounds/` 8,309,541 + Dolt DB/backup 1,500,278 + other `.beads/` 21,540 + other tracked files 366,108 = 10,197,467; plus `target/` 1,737,589,582 = 1,747,787,049, exactly. Unmentioned side effect: a checkout install also copies the live issue database and any uncommitted working-tree state. |
| I3 | "A GitHub install has no `target/` but still copies `src/`, `docs/`, `.beads/` and `tests/`, none of which the plugin uses." | an absence (`target/`) + a list of what is present | GitHub installs are also materially bloated, so the fix matters for every user | **Unsupported.** Neither fact gives a size. Measured: about 381 KB of tracked non-plugin files (388,223 total minus 6,994 plugin). The list is also incomplete (`Cargo.lock`, `LICENSE`, `.github/`, `CLAUDE.md`, `NEXT.md`). A real GitHub install was not measured. This one feeds Objection 1. |
| I4 | Non-goals: "a release (the binary is unchanged; marketplace installs track the default branch, not tags), a version bump" | the binary is unchanged + installs track the default branch | merging is enough for users to get the change | **Unsupported, and contradicted by the spec's own "Existing installs" section.** True for fresh installs only. This one displaced the weaker "folder name / docs example" objection and became Objection 2.1. |
| I5 | "Installs already at 0.2.0 keep their old (larger) copy until the next version bump; harmless, since the skill's content is identical." | the cache is keyed by version + the skill is identical | nothing behaves differently | Plausibly supported. "Larger" is about 381 KB for GitHub installs and 1.7 GB only for the developer's, which the spec deletes by hand. Not verified: whether Claude Code minds a cached 0.2.0 layout whose marketplace source now points at `./plugin`. |
| I6 | "This is the layout the Claude Code docs use in their own example (`./plugins/my-first-plugin`)." | the docs example + this repo | the docs endorse this layout for a repo that is also a Rust crate | Weak. The example is a multi-plugin marketplace repo. It shows the layout works, not that it suits this repo. |
| I7 | Done when: "the cache folder holds only `.claude-plugin/plugin.json` and `skills/timer/SKILL.md`" | expected files + "only" | any other entry means the fix failed | **The check is fragile.** Claude Code writes `.in_use` and `.orphaned_at` markers into version dirs (seen elsewhere in this cache). Phrase it as "no entries besides the plugin's files and Claude Code's own markers", and record the byte total. |

## Smaller notes

- The spec's "Why" omits that `sounds/` (8.3 MB) and the Dolt issue database were copied too. If
  the move goes ahead, put nothing gitignored under `plugin/`, or a checkout install will copy it.
- Approach B's rejection says it "still copies the source tree". It would also still copy
  `sounds/` and the Dolt DB, which is a stronger reason than the one given.
