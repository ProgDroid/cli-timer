# Red-team review, second pass: agent-friendly timers design

2026-10-22. Reviewing `docs/superpowers/specs/2026-10-22-agent-timer-design.md` as revised in `825eafa`
(HEAD `38ec0e4`). The first review (`2026-10-22-agent-timer-design-redteam.md`) is answered where the
revision answers it: the detached store, ids and liveness problem are gone, the 3-OS requirement is now
the user's stated choice, and the clippy count is updated. I don't repeat those points. Stance: a hostile
staff engineer looking for reasons this is the wrong thing to build. I accept the user's choices:
reminders held by background tasks (they die with Claude Code and have a length cap), toast plus bounded
sound, dings only on request, CLI plus skill, all three OSes, `cargo install` plus release binaries.

## Verdict

The revision cut the right things. What remains wrong is where the risk now sits. The design hands
lifetime, timeout, listing, cancelling and "tell the user it fired" to Claude Code's background tasks.
It describes how they behave using numbers from the cloud harness, which the design excludes. It leaves
confirming them to the builder, after approval. Those numbers already contradict the doc's own example.
The skill will hard-code them, and they will drift. Separately, the done-alert path assumes the agent
always reaches its last step without help, and that is the one moment the user is guaranteed to be
away.

## Evidence

| Claim | How checked | Result |
|---|---|---|
| Cloud background Bash: 30 min default, 2 h max, agent woken on exit | The Bash tool description in this cloud session: "`run_in_background` … keeps running across turns and re-invokes you when it exits … timeout … (default 1800000, max 7200000); at that limit it is stopped and you are re-invoked" | True **for this cloud harness, today**. High confidence. Says nothing about local Claude Code on Windows. |
| Clippy clean | `cargo clippy --all-targets -- -D warnings` at HEAD | 0 warnings (confirmed) |
| Edition 2024 cost | HEAD copy with `edition = "2024"` | clippy still 0 warnings; `cargo fmt --check` gives 4 hunks in 3 files (`app.rs` 2, `main.rs` 1, `tui.rs` 1) |
| Policy enforces "no `unwrap`/`expect`" | `Cargo.toml` `[lints.clippy]`; `grep` of `src/` | Not enforced. Only `all`/`pedantic`/`nursery` are on, and `unwrap_used`/`expect_used` are restriction lints. `src/` has 0 `.unwrap()`/`.expect(` today (1 `.unwrap_or`), so the rule holds by accident. The 12 `"allow"` entries and 5 inline `#[allow]`s (naming 6 restriction lints) are still inert. |
| Existing release | `gh api repos/ProgDroid/cli-timer/releases` | 1 release, `v0.1.0`, published 2023-01-18. 2 assets: `cli-timer_null_x86_64-pc-windows-gnu.tar.gz` and its `.sha256sum`. The `null` is the version step failing. `origin/main`'s newest commit is also 2023-01-18. The Actions API reports 0 runs. |
| Dependency age | `cargo search` / `cargo info` | `rodio` latest is 0.23.0 and the repo is on 0.16.0. `notify-rust` latest is 4.18.2 (MSRV 1.89). |
| Native scheduling exists beside the skill | This session's tool list | The cloud harness has `send_later` (one-shot wake into the session, survives container restarts) and scheduled tasks. A built-in `loop` skill is listed. |

### Claims about Claude Code background tasks, with confidence

| Claim | Confidence | Basis |
|---|---|---|
| Cloud: 30 min default, 2 h max, killed at the limit, agent re-invoked on exit or kill | High | My own Bash tool schema in this session, quoted above |
| Local Claude Code uses the same numbers | **Unknown; I doubt it** | From memory (medium-low): the local Bash tool has a 2 min default and 10 min maximum for foreground commands. Both can be changed in settings (`BASH_DEFAULT_TIMEOUT_MS`, `BASH_MAX_TIMEOUT_MS`). I don't know whether that timeout applies to `run_in_background` locally. In earlier versions I believe it did not. |
| Local: the agent is woken when an idle session's background task finishes | Low-medium | Memory of release notes about background-task completion notices. Not verified. It may instead be delivered on the next user turn. |
| A task survives `/clear` | Unknown | No basis either way |
| A task survives compaction | Medium that the process survives (compaction rewrites context, not processes). Medium that the label stays recoverable, because the task list shows the command line with `-l` in it. | Reasoning, not observation |
| Stopping a task on Windows (Git Bash) kills `cli-timer.exe` and not just `bash.exe` | Low | Depends on whether Claude Code tree-kills on Windows. Not verified. |
| The background-task tools have been renamed before | Medium | Memory: `BashOutput`/`KillShell` earlier, `TaskOutput`/`TaskStop` now. `TaskStop` is in this session's deferred tool list. |
| A skill's `allowed-tools` frontmatter pre-approves tools while the skill is active | Medium | Memory of the skills docs |

---

## Objection 1 (b): the background-task behaviour the design rests on is assumed from the wrong harness, and it already breaks the doc's own example

**Claim.** The chosen approach is justified by four harness behaviours: the length cap, the default
timeout, waking the agent when a task finishes, and cancel killing the process. The doc's numbers for
the first two are explicitly from "the cloud harness" (lines 38, 140), which Non-goals rules out (line
42). The other two are stated as fact (lines 74, 108–109, 143–144). All four are left to "Left to the
builder" (lines 188–189), which comes after approval. In practice that means after the remind/skill
contract is built around them. The first review's five-minute spike was not run. The approach was
adopted anyway.

**It already contradicts the doc.** The Intent's second example is "ping me in 2 hours" (line 12). The
skill sets the timeout to duration + ring + margin (lines 138–140): 7,200 s + 30 s + m, which is more
than the stated 7,200 s cap for any m > 0. Under the doc's own numbers, the skill must refuse its
headline example and offer at most about 1 h 59 m.

**The numbers can't be right for everyone.** If the local cap exists, it may be user-configurable
(medium-low confidence, see the table). The plugin ships to "everyone else" (line 31), so any number
written into the skill (line 189) will be wrong for some users by construction.

**Cancel is unverified exactly where it matters.** On Windows the task is `bash.exe -c "cli-timer
remind …"`. If stopping the task kills only `bash.exe`, a cancelled reminder still fires. It has also
left the task list, so it can't be cancelled again. This is the reverse of the first review's "dead
timer listed as pending". The Windows acceptance check (lines 165–167) covers firing, the toast, no
console window, and `ding`. It does not cover cancel or a timeout kill.

**Instead.**
1. Before approval, spend 10 minutes on the PC with four probes. (i) `sleep 2400 && echo done` in the
   background with no timeout: does it survive 30 min? (ii) The same with timeout 7,200,000 and then
   10,800,000: what is the maximum? (iii) End the turn and wait: is the agent woken on completion? (iv)
   Start a long-running native child as a background task (`ping -n 600 127.0.0.1` stands in for
   `cli-timer.exe`), stop the task, and check Task Manager to see whether `PING.EXE` is gone.
   Write the results into the spec, not the skill.
2. The skill should not do arithmetic or hard-code numbers. It should always pass the maximum timeout
   that its Bash tool description states. `remind` exits by itself, so a generous timeout costs
   nothing. The only check left is duration + ring ≤ max.
3. Add "cancel silences and prevents firing" and "an over-cap request is refused, not silently killed"
   to the Windows acceptance check.

**What would refute this:** the four probes match the doc's assumptions on the user's PC.

## Objection 2 (c): in 6 months, Claude Code has moved and the skill hasn't

**Claim.** This design outsources list, cancel, lifetime, timeout and the "fired" message to a
dependency that auto-updates every week. That dependency has no stability promise on any of these
points. It has already renamed its background-task tools once (medium confidence). The doc claims
"nothing to go stale" (line 74) and, in the same breath, plans to write the harness's real numbers into
the skill (line 189). Those numbers and tool names are exactly what goes stale. There is no test that
can notice. CI can't run Claude Code, and the acceptance check is one-off and manual.

**How it fails.** It fails quietly, in the anti-goal's direction. A shorter default or cap, a changed
completion behaviour, or a renamed stop tool means a reminder the agent called "set" gets killed. Or
"cancel that" calls a tool that doesn't exist. The user learns about it by missing the oven.

**A second collision is already visible.** The harness is growing its own schedulers. This session has
`send_later` and scheduled tasks, and a built-in `loop` skill (verified in this session's tool list;
whether local Claude Code has an equivalent, I don't know). When a native "wake me in 20 minutes"
exists, the trigger phrases in lines 136–137 compete with it. The agent will pick one or the other
inconsistently. The native one delivers into chat with no toast and no sound. The first review listed
this as speculative. It is no longer speculative in the cloud harness.

**Instead (cheap).** The skill names behaviours, not tools or numbers ("stop the background task",
"use the maximum timeout your Bash tool allows"). It carries a dated line saying which Claude Code
version it was checked against. The four probes from Objection 1 become a re-runnable two-minute script
in the repo, `scripts/check-harness.sh`, to run after a Claude Code update or when a reminder goes
missing. The skill's description says explicitly to prefer `cli-timer remind` over native session
wake-ups when the user wants to be alerted away from the chat.

## Objection 3 (b): "the agent runs `ding` as the very last step" assumes an unattended agent always gets there

**Claim.** `ding` exists for the moment the user is not watching (line 15). The design delivers it as
the agent's last action (lines 121, 146). The agent fails to reach that action in two common ways, and
both are silent. From the user's chair, silence means "still working".

1. **A permission prompt.** In default mode, an unapproved `cli-timer ding "…"` stops and waits for
   approval, from a user who by definition has walked away. The design only *mentions* the allow rule
   (line 150). Nothing installs it. On first use, the done alert turns into a prompt nobody sees. (High
   confidence that unapproved Bash prompts in default mode. Unverified whether Claude Code's own
   "needs permission" notification reaches a Windows desktop.)
2. **The turn ends abnormally.** An API error, a hit usage limit, a full context window, or the PC
   sleeping mid-work all end the turn before the last step runs. "Including when the work failed" (line
   121) covers failures the agent sees, not failures that end the agent.

**Instead.**
- Pre-approve in the skill itself (`allowed-tools: Bash(cli-timer:*)` in the frontmatter; medium
  confidence this is honoured). If it isn't, make the allow rule a required install step, not a
  mention. Run the Windows acceptance check in default permission mode.
- When the request is about a command ("ding me when the build finishes"), chain the alert to the
  command in the same shell call: `cargo build; cli-timer ding "build finished (exit $?)"`. The alert
  then depends on the build ending, not on the agent finishing.
- The skill tells the user the limit in one line when it accepts the request: "if you don't hear it,
  the session stopped; check back".

**What would refute this:** the user always runs with `Bash(cli-timer:*)` already allowed, and has
never seen a turn die mid-task.

---

## Fixed check: sentences that put two verified facts side by side

| # | Sentence (spec line) | Fact 1 / Fact 2 | Third claim a reader infers | Supported? |
|---|---|---|---|---|
| F1 | Goals, l.29 "Windows, macOS and Linux all work and are tested in CI", read with l.167 "CI cannot check sound or toasts" and l.185–186 (quiet mode turns them into log lines) | 3 OSes in CI / CI can't observe sound or toasts | macOS and Linux reminders have been seen to fire with a toast and a sound | **Unsupported implicature.** The output cells are 3 OSes × {toast, sound} = 6. Hands-on: 2 (Windows toast, Windows sound). CI: 0, because quiet mode stubs both. Unverified: 4 (macOS ×2, Linux ×2). 6 = 2 + 0 + 4. The cancel cell is 0 of 3 verified (Objection 1). On macOS, the relied-on toast comes from an unbundled binary, which the first review called unreliable. |
| F2 | Non-goals, l.38–39 "capped … (2 hours in the cloud harness; to be confirmed on the user's machine)"; l.53 "the default 30-minute task timeout" | 2 h and 30 min (verified, cloud) / not confirmed locally | The local limits are about the same, so reminders up to 2 h work | **Unsupported implicature.** The numbers come from the excluded harness. l.53 drops the qualifier. Under these numbers the doc's own "ping me in 2 hours" needs 7,200 + 30 + m s > 7,200 s. Objection 1. |
| F3 | Approaches, l.74–77 "Smallest build; nothing to go stale … Everything it builds is also needed by option 2, so upgrading later throws nothing away" | Everything built (total) / needed by option 2 → 0 thrown away (remainder) | Moving to the detached design later costs only new work | **Unsupported implicature, partly.** The build lists 13 deliverables. 9 carry over to option 2: duration parser, wall-clock wait, notification, ring with cap, synthesised sounds, `ding`, plugin packaging, release/CI, quiet mode. 4 are replaced: the `fired` event, which has no reader once the child is detached; the skill's timeout sizing; its cap refusal; and its list/cancel-via-harness instructions. 13 = 9 + 4. The core carries over; the agent-facing contract does not. "Nothing to go stale" also contradicts l.189 (Objection 2). |
| F4 | Constraints, l.64–66 "the `[lints.clippy]` policy in `Cargo.toml` (clean as of 2026-10-22), no `unwrap`/`expect` in production paths" | Policy is clean (verified, 0 warnings) / no `unwrap`/`expect` (true today: 0 in `src/`) | The policy enforces the no-`unwrap`/`expect` rule | **Unsupported implicature, carried over from the first review's F1-ii and still not fixed.** The restriction lints are off. The 12 allow entries + 5 inline allows are inert. Fix: `unwrap_used = "warn"`, `expect_used = "warn"`. |
| F5 | Also in this work, l.177 "Bump to edition 2024 (clippy is already clean)" | Clippy clean (verified, and still 0 under 2024) | The bump costs nothing else | **Mostly supported.** It also brings 4 rustfmt hunks in 3 files (re-verified). Trivial, but the parenthetical implies none. |
| F6 | When a reminder fires, l.115–116 (sound failure specified) beside l.25–26 "The notification is the thing relied on" | 2 outputs / 1 failure path specified | The notification cannot fail, or its failure is handled | **Unsupported implicature.** Failure paths: 2 outputs = 1 specified (sound) + 1 unspecified (toast). The unspecified one is the relied-on channel: Focus Assist suppresses it silently, Linux has no notification daemon in some sessions, macOS has the unbundled-binary issue. The exit code should report a toast error where one is detectable. |
| F7 | Install and release, l.156–158 "re-publishes `v0.1.0` each time (the only release, January 2023)" | 1 release (verified) / Jan 2023 (verified) | A usable Windows download exists today for the "release download" route | **Weakly unsupported.** The 2 assets of the 1 release are named `cli-timer_null_…` (the version step failed), and the build predates every change in this design. Moot, because the doc replaces the workflow, but the README must not point at `v0.1.0`. |
| F8 | Non-goals vs `docs/parking-lot.md` (the CLAUDE.md rule) | 7 non-goals / 4 parking-lot entries | Every deferred item has a parking-lot entry | **Supported.** 7 non-goals = 4 deferred and covered (outlive Claude Code and own list/cancel/stop → the "detached" entry; restart/absolute → its entry; MCP → its entry) + 3 rejected, not deferred (cloud, hook, click-to-dismiss). 4 entries = 3 from this spec + 1 (ratatui). |

Outside the spec: `NEXT.md`'s "CI does not run clippy yet, so nothing is red" is true because nothing
runs at all. There is 1 workflow (release-only, push to `main`), `main` has not moved since 2023-01-18,
and the API reports 0 runs.

**Does a flagged implicature displace an objection?** F2 and F3 are evidence for Objections 1 and 2.
F1 is the strongest independent flag. "All work and are tested" claims verification that 4 of the 6
cells will never get. I keep Objection 3 over it. F1 is fixed by rewording the goal ("built and
unit-tested on all three; toast and sound verified by hand on Windows only") plus a one-minute manual
checklist in the README. Its cost falls on hypothetical users. Objection 3 breaks the target scenario
for the one user who exists.

## Runners-up

- **(a) Simpler design, the strongest one I found: one verb, prose output.** `remind` and `ding` are
  the same thing (toast + sound for N seconds) with a different delay and ring length:
  `cli-timer alert [msg] [--in 20m] [--ring 30s]`. `--json` has exactly one consumer, an LLM that reads
  "Reminder set: check the oven at 14:32" just as well. Dropping it removes a schema to keep stable and
  a dependency. It ranks below the three because it saves perhaps a fifth of the Rust work and fixes no
  failure.
- **Notification failure is unhandled** (F6). The first review raised Focus Assist. The revision still
  doesn't say what happens.
- **`rodio` is 7 minor versions behind** (0.16 vs 0.23; the stream/sink API changed in between). The
  first review raised it. It is still "left to the builder". Decide before writing the synthesised
  sounds, not after.
- **F1 rewording**, above.
