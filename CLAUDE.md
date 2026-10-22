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


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
