# Red-team review: agent-friendly timers design

2026-10-22. Reviewing `docs/superpowers/specs/2026-10-22-agent-timer-design.md` as committed at
`06fdfdc` and unchanged at HEAD `57600b2`. Stance: hostile staff engineer looking for reasons this is
the wrong thing to build. I accept the user's stated choices: desktop notification plus bounded sound,
dings only on request, CLI plus skill rather than MCP, and timers that last within a sitting. I attack
the design built on those choices.

## Verdict

The behaviours are right. The machinery is too big for what has been established. Most of the spec's
cost sits in four parts: the detached re-launch, the per-user state store, the id scheme, and
`list`/`cancel`/`stop`. Only one requirement justifies them, which is that a timer outlives the Claude
Code process, and the spec never states it. The same parts also carry the design's worst long-term
failure: the store lists timers whose processes have died as if they were still pending. Build the
headless core first, ship it with the harness's background shell, and build the detached store only if
the user actually needs timers to outlive Claude Code.

## What I checked (evidence)

| Claim | How checked | Result |
|---|---|---|
| The human CLI requires `-s` | `src/app.rs:31` `sound: String` (not `Option`) | True |
| The tool "blocks" | Ran `cli-timer -t 00:00:01 -s /nonexistent </dev/null` with no TTY (Linux) | It does **not** block: it exits 1 after about 12 ms with `Os { code: 6, "No such device or address" }` from `enable_raw_mode` (`src/tui.rs:25`). With a TTY it runs until `q`, even after firing. Windows not tested. |
| `-t` accepts only `hh:mm:ss` | `-t 1m` | Rejected: `invalid digit found in string` |
| "currently 14 warnings to clear" | `cargo clippy --all-targets` on `f3f282c` (spec-era tree) and on HEAD | 14 at `f3f282c`; **0 at HEAD**, cleared by `666bf18` one minute after the spec was committed. The spec was not updated. |
| Edition 2024 bump cost | HEAD copy with `edition = "2024"` | 0 new clippy warnings; `cargo fmt --check` reports 4 diff hunks in 3 files (`app.rs` 2, `main.rs` 1, `tui.rs` 1) |
| The lint policy enforces "no `unwrap`/`expect`" | `f3f282c` copy with the `#[allow(clippy::expect_used)]` removed from `event.rs` (4 `.expect(` calls) | 0 `expect_used` warnings. The policy enables only `all`, `pedantic` and `nursery`; `expect_used`/`unwrap_used` are restriction lints. |
| The `[lints.clippy]` allow list does anything | HEAD copy with the 12 allow lines (Cargo.toml lines 19–30) deleted | Still 0 warnings. All 12 name restriction lints, so all 12 are inert. The 5 inline `#[allow(...)]` attributes in `src/` (naming 6 lints: `arithmetic_side_effects` ×3, `modulo_arithmetic`, `indexing_slicing`, `partial_pub_fields`) are restriction lints too, so they are also inert. |
| Existing CI | `.github/workflows/` | 1 workflow in total: `build.yml`, a release job (push to `main`, cross-builds `x86_64-pc-windows-gnu` only, re-tags the fixed `v0.1.0`). 0 check workflows. |
| Distribution channel for "anyone" | README, `build.yml` | README has no install section. The only built artifact is a Windows tar.gz. There is no macOS or Linux artifact and no `cargo install` instructions. |
| Ringing loop | `src/app.rs:288–297` | `loop { rx.try_recv() ... }` with no sleep: it spins one core for as long as the sound plays. A 30 s background ring that reuses this spins a core for 30 s. |

---

## Objection 1 (a): a simpler design gets about 80 percent of the value. Let the harness hold the timer.

**Claim.** Claude Code's Bash tool already has a `run_in_background` mode. It returns at once, keeps the
process running across turns within the session, lists background tasks, and can stop them
(`TaskStop`). The anti-goal "a shell call hangs because the timer kept stdout open" is already solved by
the harness. So are "returns immediately", "list" and "cancel". What cli-timer cannot do today is run
without a TTY: it exits 1 at once (verified above). The minimum change is therefore:

1. A headless path. When stdout is not a TTY, or with an explicit flag, wait until the wall-clock fire
   time, show the notification, ring for `--ring` (default 30 s, and fix the busy loop), then exit.
2. Humane durations (`20m`, `1h30m`) next to `hh:mm:ss`.
3. A built-in synthesised sound, making `-s` optional.
4. `ding`.
5. The skill: "run `cli-timer -t 20m -l "check the oven"` with `run_in_background`; to cancel, stop
   that background task".

**This removes:** the detached re-launch and its per-OS creation flags, the state directory, the
timer-file schema, id allocation and its race, the 1 s file polling, pruning, `list`, `cancel`, `stop`,
and the `--json` contract on four commands. It keeps every user choice: notification plus bounded
sound, on-request dings, CLI plus skill, and within-a-sitting lifetimes.

**What it loses (the missing 20 percent):** a reminder does not survive the user closing Claude Code,
and a human cannot list or cancel timers from another terminal. Both can be added later. The headless
core from step 1 is exactly what the detached child would run, so none of this work is thrown away.

**Why the spec's dismissal does not hold.** Approach 3 was dropped because "the tool blocks and requires
a sound file". Those facts disqualify *this binary as it is*. They do not show that the agent needs a
self-daemonising timer with its own store. See fixed-check row F2. The stated "blocks" is also wrong for
the agent's case, where the binary errors out instead.

**Side effect, possibly a bonus:** when a background task exits, the harness re-invokes the agent. The
agent can then repeat the reminder in the chat, which acts as a second channel if the toast is missed.
The cost is an extra turn.

**What would refute this:** the user says reminders must keep running after they close Claude Code
mid-sitting, or a test on the Windows PC shows that background Bash tasks do not survive the end of a
turn. The second takes five minutes to test: `sleep 60 && <notify>` with `run_in_background`, then end
the turn.

## Objection 2 (b): "macOS and Linux must work too, so anyone can install it" is assumed, not established

**Claim.** The facts given are one developer on one Windows PC. The spec turns that into "Windows first,
but nothing Windows-only… so anyone can install it". Nothing says who "anyone" is or how they would get
the binary. The repo has no channel for them (see the evidence table: 1 workflow, 1 artifact,
Windows-only, no install docs). The plugin would ship a skill whose first step, `cli-timer --version`,
fails for every user except one.

**Why it matters.** The cross-platform requirement multiplies exactly the parts that tests cannot check:

- process detachment (`DETACHED_PROCESS | CREATE_NO_WINDOW` on Windows; a new session on Unix);
- notifications on three backends. On macOS, notifications sent from a binary that is not in an app
  bundle are known to be unreliable, and the spec makes the notification "the thing relied on";
- audio from a background process on three audio stacks;
- Linux CI system dependencies ("whatever the notification crate needs", which the spec leaves open).

Each one is a matrix of three cells. Only the Windows cell has a user, and the spec's quiet-test mode
turns sound and notifications into log lines, so the cells nobody runs are never exercised.

**Instead:** make Windows the only target that is promised and tested by hand. Keep the code portable by
construction: platform code behind `cfg`, with the other platforms compiling and logging rather than
claiming support. Say in the README that macOS and Linux are best effort. If the user really does want
other people to install it, that becomes its own scoped requirement: release artifacts for 3 targets,
an install section, and the skill's install message pointing at them. It should not be a side clause
here. If the brainstorm transcript shows the user asked for cross-platform, this objection shrinks to
"define the install channel before shipping the plugin".

## Objection 3 (c): in 6 months, the store will say timers are pending when their processes are dead

**Claim.** In the proposed design a timer exists in two places: a file and a process. Nothing ties them
together. `list` decides liveness from the clock ("prunes files whose timers are clearly dead, e.g. fire
time plus ring time long past"), not from the process. Ordinary use of a Windows PC over months will
kill timer processes while their files remain:

- Windows Update restarts, and shutdowns with Fast Startup (which ends the user session);
- crashes, or an end-task in Task Manager;
- Claude Code killing a process tree after a Bash timeout or an interrupt. Unverified on Windows:
  whether a `DETACHED_PROCESS` grandchild escapes Claude Code's process-tree kill and job object. Spike
  this before building anything.

Each of these leaves a file whose fire time is in the future. `list` reports it as **pending**, the
agent tells the user "your reminder is set for 14:32", and nothing fires. Restarts are a declared
non-goal, so the timer is allowed to be lost. Losing it **silently while still claiming it exists** is
the worst outcome for a design whose goal is "the notification is the thing relied on".

Ids make it worse. Ids "only need to be unique among timers that currently exist", so once a file is
pruned its id can be reused. The agent's "timer 3" from earlier in the conversation can then cancel a
different timer.

**The residual (spec's own terms).** Let N be the number of timer files in the store. Then
N = A + B + C + D + E, where:

| Part | Meaning | Spec handles it? |
|---|---|---|
| A | process alive, before fire time | yes: `list` shows pending |
| B | process alive, ringing | yes: `list` shows ringing; `stop`/`cancel` |
| C | process dead, fire time not yet "long past" (killed before or during firing) | **no**: shown as pending or ringing; this is the lie |
| D | process finished normally, file still present | **unspecified**: the spec never says whether the child deletes its own file when it completes. If it does not, the file shows as pending or ringing until it is pruned. |
| E | process dead, fire plus ring "long past" | yes: pruned on the next `list` |

Cancelled timers are not in N: `cancel` deletes the file. The spec's rules cover A, B and E. They do not
cover C or D, so the files the spec does not account for are N − (A + B + E) = C + D.

**Instead (cheap):** have the child hold an OS-level exclusive lock on its timer file for its whole life
(`LockFileEx` on Windows, `flock` on Unix, for example through `fd-lock`). The kernel releases the lock
when the process dies. `list` then tests the lock: if a file is unlocked and its fire time has not
passed, that is C, reported as **lost** rather than pending. The next `start` or `list` can show a
notification that a reminder was lost. The child deletes its own file on normal completion, which
removes D. Ids increase monotonically per day ("3" is still short to say), which removes the reuse
problem. Under Objection 1's design none of this is needed: the harness's task list is the process list.

---

## Fixed check: sentences that put two verified facts side by side

For each sentence that pairs a count with an absence, or a total with a remainder, this table names the
third claim a reader will infer and says whether the facts support it.

| # | Sentence (spec section) | Fact 1 / Fact 2 (verified?) | Inferred third claim | Supported? |
|---|---|---|---|---|
| F1 | Constraints: "the `[lints.clippy]` policy now in `Cargo.toml` (currently 14 warnings to clear)" | A policy exists (yes) / 14 warnings (yes at `f3f282c`) | (i) 14 warnings are outstanding work for this project. (ii) The policy enforces the house rules listed with it, including "no `unwrap`/`expect`". (iii) 14 is the whole lint cost, including the edition-2024 bump in the same bullet. | (i) **Stale.** True when written; 0 at HEAD since `666bf18`. (ii) **Unsupported implicature.** `expect_used`/`unwrap_used` are not enabled; 4 `.expect(` calls produced 0 warnings. All 12 allow entries and all 5 inline allow attributes are inert, which suggests restriction lints are on when they are not. (iii) Clippy part supported (2024 adds 0), but the bump also brings 4 rustfmt hunks. |
| F2 | Approaches, #3: "Skill only… Dropped: the tool blocks and requires a sound file, so the skill would be fighting it." | Requires a sound file (yes) / blocks (half true: with a TTY it never exits on its own; without one it exits 1 at once) | So cli-timer must become a self-daemonising, self-tracking tool (approach 1), and a lighter route offers nothing. | **Unsupported implicature.** The facts rule out the binary as it is. They do not rule out a headless mode run under the harness's background shell. This is the basis of Objection 1. |
| F3 | Approaches, #1: "Costs: the binary must be on PATH and the agent needs permission to run it (one allow rule)." | 2 costs named / a count of "one" rule | These are all the adoption costs. | **Unsupported implicature.** Getting the binary onto PATH has no channel (1 Windows-only artifact; 0 install docs; macOS and Linux need a source build, and Linux also needs ALSA headers). Installing the plugin and marketplace is a further cost. Feeds Objection 2. |
| F4 | Left to the builder / Ids: "unique among timers that currently exist" next to "`list` prunes files whose timers are clearly dead" | A file per timer (proposal) / time-based pruning (proposal) | The files present are the timers that exist. | **Unsupported implicature.** Decomposed above as N = A+B+C+D+E; the spec covers A, B and E, and leaves C and D. This is Objection 3. |
| F5 | Non-goals: two items "Parked in `docs/parking-lot.md`" | 5 non-goals / 2 parked | The other 3 are rejected, not deferred (so the CLAUDE.md parking-lot rule is met). | **Supported.** 5 = 2 parked (MCP; restarts/absolute times) + 3 not deferred (cloud sessions: impossible; hook: the user's choice; click-to-dismiss: rejected). The parking lot holds 3 entries = those 2 + 1 from the brainstorm (ratatui). |
| F6 | Also in this work: "Add a CI check workflow… deferred… until the warnings are cleared." | CI deferred (yes) / warnings exist (yes when written) | There is no CI, and the warnings are the only thing blocking it. | **Partly stale.** Workflows: 1 in total = 1 release + 0 check, so "no check CI" holds. The warnings blocker is gone (0 at HEAD). What still blocks CI is Linux system dependencies, which the next sentence states, plus the rustfmt drift from the 2024 bump (F1-iii). |

Outside the spec, the same kind of problem appears once. `NEXT.md` says "No code changed yet beyond the
project-start ritual", but `666bf18` changed 4 files in `src/`.

**Does a flagged implicature displace an objection?** No. F2, F3 and F4 are the evidence behind
Objections 1, 2 and 3. F1-ii (the lint policy looks stricter than it is) is real but cheap to fix: add
`unwrap_used = "warn"` and `expect_used = "warn"`, and delete the 12 inert allows or say why they are
there. It is weaker than all three objections, so they stand.

## Runners-up (considered, not in the top three)

- **The notification is assumed to be reliable.** Windows Focus Assist / Do Not Disturb (on a schedule,
  or automatic during full-screen apps and presentations) suppresses toast banners without telling
  anyone. If the user is away for more than the 30 s ring, all that is left is an entry in Action
  Center. The re-invoke-the-agent bonus in Objection 1 partly covers this.
- **The harness may make this redundant.** Agent harnesses are adding native scheduling, background
  tasks and notifications; this session's own harness already has scheduled tasks. A skill that
  competes with a built-in tool will lose the trigger. Speculative, so it is not ranked.
- **The old dependencies will collide with the new code.** `tui` 0.19 is unmaintained (already parked)
  and `rodio` 0.16 is old. A later `rodio` upgrade changes the stream and sink API that the synthesised
  sounds would be written against. If `rodio` is going to be upgraded, do it before writing the sound
  code, not after.
- **`stop` adds little.** With a bounded 30 s ring, `stop` and `cancel <ringing id>` overlap with each
  other and with waiting 30 s.
- **The MCP rejection has an asymmetry.** "Tool schemas loaded into every session's context" is also
  true, at a smaller size, of a skill's description.
