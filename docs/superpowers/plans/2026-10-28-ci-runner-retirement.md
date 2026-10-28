# CI Runner Retirement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Linux release archive on `ubuntu-24.04` instead of the retiring `ubuntu-22.04`, and tell Linux users the resulting loader warning on older distros is harmless.

**Architecture:** One-line runner change in `.github/workflows/release.yml`, a README note, and removal of the parking-lot entry. Proved by a manual release run (dry run) from the branch and a `readelf -V` of the built Linux binary.

**Tech Stack:** GitHub Actions, `gh` CLI, Docker (for `readelf` on this Windows machine).

**Spec:** `docs/superpowers/specs/2026-10-28-ci-runner-retirement-design.md`

## Global Constraints

- The Linux row's `os` is exactly `ubuntu-24.04` (pinned, not `ubuntu-latest`).
- No change to `check.yml`, the publish job, the other build rows, or any Rust code.
- No version bump, no tag, no release.
- The README names no glibc version number as a requirement; it names the warning text `weak version 'GLIBC_2.39' not found`.
- Bead: `clitimer-195`; put the id alone on its own line in the body of the commit that finishes it.

## Review Focus

- Older distro (Debian 12, Ubuntu 22.04, RHEL 9): binary runs and prints one loader warning on stderr. Already measured in the spec; the dry-run `readelf` check (Task 1, Step 6) proves the shipped build has the same shape (2.39 WEAK only).
- The dry run builds the branch's workflow, not `main`'s: confirm the run's `headSha` is the branch commit (Task 1, Step 5).
- None further: no code path changes.

---

### Task 1: Move the Linux release build to ubuntu-24.04

**Files:**
- Modify: `.github/workflows/release.yml:54-55` (the Linux matrix row and its comment)
- Modify: `README.md:67-68` (the Linux note in the install section)
- Modify: `docs/parking-lot.md` (the "CI runner retirement" bullet, lines 24-26)
- Modify: `plugin/skills/timer/SKILL.md` (one line on the harmless loader warning)
- Add: this plan, the spec, and both red-team reviews under `docs/superpowers/`

Plan red team: `docs/superpowers/reviews/2026-10-28-ci-runner-retirement-redteam.md`. Its three
objections were accepted (developer, 2026-10-28) and are folded into Steps 2, 3a and 4.

**Interfaces:** none.

- [ ] **Step 1: Branch.** From an up-to-date `main` in the main checkout: `git switch -c ci-runner-2404`.

- [ ] **Step 2: Edit `release.yml`.** The Linux row's `os: ubuntu-22.04` becomes `os: ubuntu-24.04`. Replace the comment above it with one saying: pinned rather than `ubuntu-latest` so the build image only changes when someone edits this line (the glibc symbols the binary needs also depend on the Rust toolchain and dependencies, so this does not freeze them); a 24.04 build still runs on older distros, where the loader prints a harmless weak-version warning.

- [ ] **Step 3: Edit `README.md`.** After "the Linux one needs the ALSA runtime library (`libasound2` on Debian and Ubuntu)." add a sentence: it is built on Ubuntu 24.04, and on older distros it still runs but prints a harmless `weak version 'GLIBC_2.39' not found` line on every run.

- [ ] **Step 3a: Edit `plugin/skills/timer/SKILL.md`.** Add one line (in section 1, after the `--version` paragraph) telling the agent: on Linux older than glibc 2.39, every command may print `... weak version 'GLIBC_2.39' not found ...` on stderr; it is harmless, ignore it, and never report it as the reason a notification or sound failed. Run `cargo test` (the plugin tests read this file) and expect all tests to pass.

- [ ] **Step 4: Edit `docs/parking-lot.md`.** Keep the "CI runner retirement" bullet and append **Done 2026-10-28**: Linux release built on `ubuntu-24.04`; older distros run it with a harmless weak-version warning; a CI guard on the glibc floor and building in an older container were declined (pointer to the spec). Commit the four edited files plus the spec, this plan and both reviews (the beads pre-commit shim stages `.beads/issues.jsonl`):

```
ci: build the Linux release on ubuntu-24.04 (22.04 retires 2027-04-17)

clitimer-195
```
with the session's `Co-Authored-By` trailer, via `git commit -F <file>` from the Bash tool.

- [ ] **Step 5: Dry run from the branch.** `git push -u origin ci-runner-2404`, then `gh workflow run release.yml --ref ci-runner-2404`. Get the run id (`gh run list --workflow release.yml --limit 1 --json databaseId,headSha,headBranch`), check `headBranch` is `ci-runner-2404` and `headSha` equals `git rev-parse HEAD`. `gh run watch <id> --exit-status`.
  Expected: conclusion `success`; the four Build jobs succeed; "Tag matches Cargo.toml" and "Publish release" skipped; four artifacts listed by `gh api repos/ProgDroid/cli-timer/actions/runs/<id>/artifacts`.

- [ ] **Step 6: Check the shipped binary's glibc needs.** `gh run download <id> -n cli-timer-x86_64-unknown-linux-gnu -D <scratchpad>/dry`, extract the tarball, then in Docker (`MSYS_NO_PATHCONV=1 docker run --rm -v <dir>:/b:ro ubuntu:24.04 sh -c 'apt-get update -qq && apt-get install -yqq binutils >/dev/null && readelf -V /b/cli-timer | grep -E "Name: GLIBC_2\.(3[4-9]|4)"'`).
  Expected: exactly `GLIBC_2.34  Flags: none` and `GLIBC_2.39  Flags: WEAK`; nothing newer than 2.34 without `WEAK`.

- [ ] **Step 7: CI on the branch.** `gh run list --branch ci-runner-2404 --workflow check.yml --limit 1` shows `success` for the branch commit.

---

After Task 1: final review, then squash-merge into `main` locally with a `handoff:` subject (NEXT.md updated in the squash), push `main`, close `clitimer-195`, delete the `ci-runner-2404` branch locally and on GitHub.
