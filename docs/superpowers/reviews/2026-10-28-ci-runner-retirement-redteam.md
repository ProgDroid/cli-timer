# Red team: CI runner retirement plan

Plan: `docs/superpowers/plans/2026-10-28-ci-runner-retirement.md`. Spec:
`docs/superpowers/specs/2026-10-28-ci-runner-retirement-design.md` (bead `clitimer-195`). Earlier
round: `2026-10-28-ci-runner-retirement-design-redteam.md`.
Reviewer stance: hostile staff engineer, looking for why executing this plan fails. Date: 2026-10-28.

## What I checked against the real files

- `.github/workflows/release.yml`: lines 54-55 are the comment and `os: ubuntu-22.04` row, as the
  plan says. Line 5 says "GitHub only dispatches workflows on the default branch".
- `README.md`: lines 67-68 hold the ALSA sentence the plan appends to. Confirmed.
- `docs/parking-lot.md`: lines 24-26 are the "CI runner retirement" bullet. Confirmed. Declined
  entries elsewhere in the file (lines 27-31, 37-40) are kept with a **Declined** note, not deleted.
- `.github/workflows/check.yml`: `on: push` with no branch filter, so Step 7's branch run will exist.
- `plugin/skills/timer/SKILL.md`: lines 28, 62-70, 72-81, 107-111 (quoted in objection 1).
- `src/`: the only `std::process::Command` is `osascript` in `src/alert.rs:85`, under
  `#[cfg(target_os = "macos")]`. So on Linux cli-timer's own code spawns no child; the std pidfd
  fallback is reached only if a dependency spawns (not checked).
- Git status at session start: the spec, the plan and the design red team are untracked;
  `.beads/issues.jsonl` is modified.
- v0.2.3 Linux binary (downloaded with `gh release download v0.2.3`, byte grep of version names,
  no Docker): 23 distinct names = 18 `GLIBC_*` (2.2.5, 2.3, 2.3.2, 2.3.4, 2.9, 2.14, 2.15, 2.16,
  2.17, 2.18, 2.25, 2.27, 2.28, 2.29, 2.30, 2.32, 2.33, 2.34) + 2 `ALSA_*` + 3 `GCC_*`. 18 + 2 + 3 = 23,
  matching the design red team. No `GLIBC_2.4`.

## The three objections

### 1. (a) The warning's real reader is the plugin's agent, and the plan only tells humans

The spec's goal is that a user who sees the loader warning can learn it is harmless, and the plan
delivers that as one README sentence. But the project's headline integration is the Claude Code
plugin, and its skill states a stream contract the warning breaks on every old-distro run:

- `SKILL.md:28`: "Run `cli-timer --version`. It prints `cli-timer X.Y.Z`."
- `SKILL.md:62`: "The first output line is the `set` line." The loader prints its line before
  `main` runs, so in a combined stdout+stderr view (how a shell tool usually shows output) the first
  line is the glibc warning.
- `SKILL.md:74`, `:80-81`, `:111`: on exit 3, "mention the error on stderr" / "details on stderr".
  On Debian 12, Ubuntu 22.04 or RHEL 9, stderr always holds the glibc line, so the agent will report
  "GLIBC_2.39 not found" to the user as the reason the pop-up or sound failed.

The spec's measurement ("stdout, including the JSON line, is unchanged") is true and is exactly why
this was missed: the agent does not read stdout alone. The project already paid to keep stderr clean
for this reader (`sound::open_output`, parking lot lines 64-67). Fix in this change: one line in
`SKILL.md` saying a `weak version 'GLIBC_2.39' not found` line on stderr is harmless and is not the
failure reason, and that the `set` line may not be the very first line. Note that a skill edit plus
the plugin-version coupling (parking lot lines 44-58) means it only reaches GitHub installs on the
next version bump; the plan says "no version bump", so say so in NEXT.md.

### 2. (c) Step 2's comment re-asserts a claim the last red team refuted, and nothing will catch the drift

Plan Step 2 dictates the YAML comment: "pinned rather than `ubuntu-latest` so the glibc the binary
links against changes only when someone edits this line". The design red team (fixed check row 5,
objection 2) called exactly this unsupported: the 2.39 references come from Rust std, so a
`rust-toolchain.toml` bump or a dependency refresh can add a new versioned reference or turn a weak
one strong with this line untouched. The spec's "Decided" section only asks the comment to say *why*
it is pinned; the plan's wording adds the false guarantee.

With the CI guard dropped (spec Non-goals), that comment and the README sentence ("on older distros
it still runs") are the only record, and both are statements of intent with no check behind them.
In the next six months the CLAUDE.md toolchain-bump routine (change `channel`, run fmt, clippy,
tests) will run at least once and checks nothing about glibc; the rodio rewrite is queued. If std
ever makes `pidfd_spawnp` a strong reference (or a dependency pulls in a 2.35+ symbol), old distros
stop starting with "version not found", the README still says it runs, and the comment tells the
next reader it cannot have changed. Cheapest fix that stays inside the "no guard" decision: reword
the comment to "pinned so the runner image only changes when someone edits this line; the toolchain
and dependencies can still raise the glibc floor", and add to CLAUDE.md's release line "before
pushing a tag, `readelf -V` the dry-run Linux binary: only `GLIBC_2.39` may be newer than 2.34, and
only as WEAK". That turns Step 6 from a one-off into the release checklist.

### 3. (b) The plan's commits leave the spec, the plan, both reviews and the bead export uncommitted

Step 4 commits "all three files" (release.yml, README.md, parking-lot.md). The finish line squashes
with "NEXT.md updated in the squash". Nothing commits
`docs/superpowers/specs/2026-10-28-ci-runner-retirement-design.md`, this plan,
`docs/superpowers/reviews/2026-10-28-ci-runner-retirement-design-redteam.md` or this review, all
untracked at session start, nor `.beads/issues.jsonl`, already modified and modified again by
`bd close`. The spec is the "why" the shared rules say must leave scratch for `docs/` and git
(CLAUDE.md: "Design docs live in `docs/superpowers/specs/`"; precedent commit d0be7e1
"docs: v0.2.2 final review ..."). After the squash, `main` carries a parking-lot deletion whose
reasoning exists only in untracked files, and the next `git switch` or clean loses it.

Related, in the same file: the plan deletes the parking-lot bullet outright, while the file's own
convention for a decided idea is to keep it with a **Declined** note (lines 27-31, 37-40). The spec
declines two things a future reader will re-propose: the CI glibc guard and Approach C (build in an
older container to keep stderr clean). CLAUDE.md says to read the parking lot when scoping; the
deleted bullet takes the only pointer to those refusals with it. Fix: Step 4 adds the docs and
`.beads/issues.jsonl` to the commit (or a separate `docs:` commit), and replaces the bullet with a
"**Done 2026-10-28** ... declined: glibc guard in CI, older-container build (spec link)" note.

## Lesser findings (not in the three)

- **Step 5 races the dispatch.** `gh workflow run` returns before the run is registered;
  `gh run list --workflow release.yml --limit 1` right after it can return the previous run (the
  v0.2.3 tag run). The headBranch/headSha check catches that, but the plan gives no retry, so an
  executor reads it as a failure. Add "repeat the list until `headBranch` is `ci-runner-2404`" or
  filter with `--branch ci-runner-2404 --event workflow_dispatch`.
- **`release.yml` line 5 contradicts Step 5.** It says "GitHub only dispatches workflows on the
  default branch", yet the plan dispatches with `--ref ci-runner-2404` (which works: the workflow
  must exist on the default branch, then any ref runs its own copy). The plan touches the file and
  leaves the misleading line; a later reader will believe branch dry runs are impossible.
- **Step 6's grep pattern** `GLIBC_2\.(3[4-9]|4)` also matches `GLIBC_2.4` (an old version), not
  only 2.40+. Harmless today (v0.2.3 has no `GLIBC_2.4`, measured above), but "Expected: exactly"
  two lines will raise a false alarm the day a dependency needs 2.4. Use `GLIBC_2\.(3[4-9]|[4-9][0-9])\b`.
  The mount `<dir>` must be the extracted `cli-timer-x86_64-unknown-linux-gnu/` folder, not the
  download folder, or `readelf` reports no file.
- **README quote character.** glibc prints `` weak version `GLIBC_2.39' not found `` (backtick
  open, as the spec's own Facts line shows); the plan writes a straight `'`. Searching the README for
  the exact text seen fails; searching for `GLIBC_2.39` still works. Keep the search key
  `GLIBC_2.39` intact in the sentence.

## Fixed check: paired facts and the claim a reader infers

| # | Where | Facts set side by side | Inferred third claim | Supported? |
|---|---|---|---|---|
| 1 | Plan, Review Focus line 23 | Older distros were "already measured in the spec" + Step 6 "proves the shipped build has the same shape (2.39 WEAK only)" | The shipped build behaves on old distros as the measured one did, for every command. | **Partly unsupported.** Same version needs does establish the same loader behaviour (starts, one warning). It does not establish runtime behaviour: only `--version` and `ding --json hi` were run, in quiet mode; `remind`, sound and the full-screen UI were not, and the pidfd fallback was never exercised (cli-timer itself spawns nothing on Linux; dependencies not checked). |
| 2 | Spec Facts line 24-26 | One stderr line on every run + "stdout, including the JSON line, is unchanged" | Consumers of the output are unaffected. | **Unsupported implicature**, and false for the plugin's agent (objection 1). Flagged; it displaces nothing because objection 1 already is this finding. |
| 3 | Plan Step 2 comment / spec Approach A | Runner pinned + glibc measured once on this branch | The floor stays put until this line is edited. | **Unsupported** (refuted in the design red team; objection 2). |
| 4 | Spec Non-goals line 46-48 | CI tests on `ubuntu-latest`, release builds on `ubuntu-24.04` + "the dry run covers the shipped build" | Future releases are covered. | **Unsupported.** The dry run covers this branch commit only. A tag cut after a toolchain or dependency change is built and published with no dry run unless the CLAUDE.md "do a manual run before pushing a tag" rule is followed, and that rule checks for four archives, not glibc. |
| 5 | Spec Facts line 17-21 | v0.2.3 (22.04) needs 2.34, no weak + 24.04 build needs 2.34 strong + 2.39 weak | The runner change alone causes the delta. | **Supported** if both used toolchain 1.99.1 (the spec says the container used the pinned channel; v0.2.3 was cut the same day). The container is not the GitHub runner image; Step 6 closes that gap. |
| 6 | Plan Step 5 Expected | Four Build jobs succeed + four artifacts | The Linux artifact is the 24.04 build. | **Supported**, via the `headSha` check tying the run to the branch commit whose YAML says 24.04. |
| 7 | Spec Intent | Deprecated 2026-09-17, removed 2027-04-17, four brownouts | Any release before the first brownout is safe on 22.04. | **Supported** (verified by the design red team against runner-images#14254). |

Residuals decomposed:
- v0.2.3 version names: 23 = 18 GLIBC + 2 ALSA + 3 GCC (my byte grep, listed above; sums).
- Step 6 expected lines: the pattern's window is 2.34 to 2.39 plus 2.4 and 2.40-2.49. For v0.2.3 it
  holds 1 line (2.34). For the 24.04 build the spec predicts 2 = 2.34 (strong) + 2.39 (weak, from 2
  std symbols: `pidfd_spawnp`, `pidfd_getpid`). Whether `readelf -V` lists `GLIBC_2.34` under more
  than one needed library (which would make "exactly" fail) is UNKNOWN; the design red team did not
  record per-library lines.
