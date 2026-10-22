# Agent-friendly timers: design

2026-10-22. Status: approved by the user 2026-10-22. Revised after the builder check and two red-team reviews
(`docs/superpowers/reviews/2026-10-22-agent-timer-design-redteam.md`, `...-v2-redteam.md`).

## Intent

Make cli-timer something a coding agent (Claude Code, running on the user's own machine) can use
as a tool, without making it worse for a human at a terminal. Two uses:

1. **Reminders.** The user says "remind me in 20 minutes to check the oven" or "ping me in 2
   hours", optionally naming a sound. The agent starts a reminder in the background, confirms, and
   carries on. The reminder fires after the agent's turn has ended.
2. **Done alerts, on request.** The user says "ding me when the build finishes". The agent does the
   work and, as its last step, plays a short alert so the user hears it instead of having to watch
   the output.

Origin: switchboard note `20261021-352d` (raised 2026-10-21) and the brainstorm of 2026-10-22.

## Goals

- A terminal-less reminder command that an agent runs as a background task: it waits, fires, and
  exits. Claude Code's own background tasks provide "returns immediately", listing and cancelling.
- When a reminder fires: a desktop notification showing the label, plus the sound looping for a
  bounded time (default 30 s), then silence and exit. The notification is the thing relied on; the
  sound is the attention-grabber.
- A one-shot `ding` for done alerts: short sound, plus a notification with the message.
- Works with no sound file: built-in default sounds.
- Windows, macOS and Linux all work and are tested in CI in this round.
- Installable without building by hand: `cargo install --git` for anyone with Rust, and prebuilt
  release binaries for all three platforms for everyone else.
- A Claude Code plugin, shipped in this repo, whose skill teaches the agent when and how to use it.
- The existing full-screen timer keeps working as it does today for a human.

## Non-goals

- **Reminders that outlive Claude Code.** A pending reminder dies when Claude Code exits ("Background
  tasks are automatically cleaned up when Claude Code exits", code.claude.com interactive-mode docs).
  The user accepted this on 2026-10-22. The detached design that removes it is parked in
  `docs/parking-lot.md`.
- Length caps: per the Claude Code docs (tools-reference, checked 2026-10-22), background commands
  in a local terminal, desktop app or VS Code session have **no time limit**; only unattended runs
  (`-p`, Agent SDK, CI, cloud) cap them (30 min default, 2 h max). Interactive local use is the
  target, so no cap applies; the skill must still not claim a reminder in an unattended run that its
  limits would kill.
- Our own `list` / `cancel` / `stop` commands: Claude Code's background-task tools do this.
- Cloud sessions. A timer in a remote container cannot reach the user; the skill says so.
- Restart survival and absolute times ("tomorrow at 9"). Parked.
- An MCP server. Parked; the library/CLI split keeps it a thin later layer.
- Automatic alerts on every agent turn via a hook. The user wants done alerts on request only.
- Dismissing the sound by clicking the notification, and notifications that stay on screen until
  dismissed: a standard toast that goes to the notification centre is fine (user, 2026-10-22).

## Anti-goals (it would be a failure even if it "worked")

- A reminder rings forever while the user is away.
- The agent says a reminder is set when it will in fact be killed before it fires.
- The agent is asked for a done alert and the user waits in silence because the `ding` is stuck
  behind a permission prompt nobody is watching.
- The human CLI gets worse: `cli-timer -t hh:mm:ss -s file -l label` stops working.
- Alerts nobody asked for.

## Constraints

- Rust; existing stack is `clap` 4, `tui` 0.19 + crossterm, `rodio` 0.16, `chrono`.
- Today the binary needs a terminal: the TUI fails straight away without one. The new commands
  must not touch the terminal at all.
- On Windows, Claude Code runs shell commands through Git Bash. The reminder must not open a
  console window.
- House Rust standards (`core:rust-standards`): edition 2024, the `[lints.clippy]` policy in
  `Cargo.toml` (clean as of 2026-10-22), no `unwrap`/`expect` in production paths, `# Errors`
  docs, `log` over `eprintln!`, tests in the same file.
- Tests must not make noise or pop notifications.
- A standard Windows toast from an unregistered binary shows as coming from "Windows PowerShell".
  Accepted.

## Approaches considered

1. **CLI + a skill in a plugin shipped from this repo; reminders held by Claude Code's background
   tasks.** *Chosen* (user, 2026-10-22). Smallest build; nothing to go stale. Cost: reminders die
   with Claude Code (and are capped in unattended runs, which are not the target). Everything it builds is also needed by option 2, so upgrading later throws
   nothing away.
2. **CLI + skill; cli-timer detaches its own background process and tracks timers in files, with
   `start` / `list` / `cancel` / `stop`.** The first draft of this doc. No length cap, survives
   Claude Code closing, works for humans and any agent. Rejected for now: roughly three times the
   work, in the most fragile parts (detaching on three OSes, a timer store), and the red-team found
   it needs per-timer file locks and never-reused ids to avoid reporting dead timers as pending or
   cancelling the wrong one. Parked with those fixes noted.
3. **Same CLI plus an MCP server mode.** Rejected for now: extra code, tool schemas in every
   session's context, MCP-only benefit. Parked.
4. **Skill only, no changes to cli-timer.** Dropped: the tool needs a terminal and a sound file.

## What was decided

### Command surface

The user decided the behaviours; the exact names and flags are my proposal and the builder may
adjust them if something reads better, as long as the old invocation keeps working.

| Command | What it does |
|---|---|
| `cli-timer -t <duration> [-s file] [-l label]` | Today's full-screen timer. Unchanged, except `-s` becomes optional (built-in sound) and `-t` also accepts the short forms below. |
| `cli-timer remind <duration> [-l label] [-s file] [--ring 30s] [--json]` | No terminal. Prints a "set" line straight away, waits, fires (notification + sound for `--ring`), prints a "fired" line, exits 0. |
| `cli-timer ding [message] [-s file] [--json]` | No terminal. Plays a short alert once, shows a notification with the message, exits. A second or two at most. |

- **Durations** accept the existing `hh:mm:ss` and short forms: `90s`, `20m`, `1h30m`, `2h`.
- **Output.** Human-readable by default; with `--json`, one JSON object per line on stdout, e.g.
  `{"event":"set","label":"check the oven","fires_at":"2026-10-22T14:32:00+01:00"}` then
  `{"event":"fired",...}`. Errors are `{"error":"..."}` with a non-zero exit. The agent reads the
  "set" line to confirm, and the "fired" line when Claude Code reports the task finished.
- **Sleep:** the reminder waits for a wall-clock time, so if the machine sleeps past it, it fires
  on wake.
- **Early silence or cancel:** stopping the background task (Claude Code's own tool) kills the
  process, which ends the wait or the sound.

### When a reminder fires

- Desktop notification: title "cli-timer", body the label (or "Timer finished").
- The sound loops for `--ring` (default 30 s, sensibly capped), then the process exits.
- If sound cannot play (no device, bad file), the notification still shows; the error is printed
  on stderr (which Claude Code captures) and the exit code says so.

### Done alerts

`ding` is a short alert sound played once plus a notification with the message. The agent runs it
only when the user asks for it, as the last step, including when the work failed (the message
says so). There is no hook.

### Default sounds

Built-in sounds so nothing needs configuring. My proposal: synthesise them with rodio's sine
sources (a few-second alarm pattern for reminders, a short chime for `ding`) instead of bundling an
audio file, avoiding asset licensing. `-s` overrides both.

### Agent integration

The repo becomes a Claude Code plugin and its own marketplace (`.claude-plugin/plugin.json` and
`marketplace.json`, one skill under `skills/`), separate from the user's `claude-setup` plugin.
The skill:

- triggers on "remind / ping / alert me in X", "set a timer for X", "ding / let me know when this
  is done" and similar;
- runs `remind` as a **background** shell command. It does not hard-code Claude Code's timeouts or
  tool names: it tells the agent to check its own Bash tool's stated background limits, and if a
  limit applies (unattended runs), to set the timeout to cover duration + ring + margin, or say
  plainly that the request is too long rather than pretend;
- confirms in one line (what, when) from the "set" line, and if the agent is told the task
  finished, tells the user it fired;
- prefers cli-timer over the agent's own scheduling tools whenever the user wants to be alerted
  (sound or pop-up), since those only wake the agent, not the user;
- for "what reminders do I have" and "cancel that", uses Claude Code's background-task tools;
- when a done alert is requested, makes sure `cli-timer` can run without a permission prompt
  **at that moment, while the user is present** (e.g. run `cli-timer --version`, which surfaces
  any prompt now), and recommends the allow rule if it isn't set; then runs `ding` as the very
  last step of the work. Known limit: a turn that ends abnormally (crash, interrupt, usage limit)
  never reaches the `ding`; the skill tells the user so when they first ask;
- checks `cli-timer --version` first and, if the binary is missing, gives the install routes
  instead of improvising another timer;
- says the alert can't reach the user in a cloud session;
- mentions the allow rule `Bash(cli-timer:*)` for prompt-free use.

### Install and release

- README documents `cargo install --git https://github.com/ProgDroid/cli-timer` (needs a Rust
  toolchain; Linux also needs ALSA headers and `pkg-config`) and the release download.
- Replace `build.yml`. Today it cross-compiles only Windows on Ubuntu, runs on every push to
  `main`, re-publishes `v0.1.0` each time (the only release, January 2023), and uses unpinned or
  unmaintained actions. The new one builds on native Windows, macOS and Linux runners, and only
  when a version tag is pushed.

### Checks

- A CI check workflow on pushes and PRs: `fmt`, `clippy -D warnings`, `test`, as a Windows, macOS
  and Linux matrix (deferred from the project-start ritual).
- Acceptance includes a hands-on check on the user's Windows PC through Claude Code. CI cannot
  check sound, toasts or Claude Code itself. Checked there:
  - a background `remind` fires with a toast and sound, and no console window appears;
  - `ding` works and, with the allow rule set, never prompts;
  - stopping the background task stops `cli-timer.exe` and its sound (the docs only promise this
    on macOS and Linux);
  - whether the agent is told when the task finishes in an interactive session (not documented;
    the skill must read well either way).
- If stopping the task on Windows leaves `cli-timer.exe` running, the builder makes `remind` exit
  when its parent shell goes away (or picks another way to tie its life to the task).

### Structure

The timer logic (durations, waiting, firing, sound, notifications) lives in the library crate.
`main.rs` only parses arguments and dispatches. A future detached mode or `mcp` mode is another
thin front end.

### Also in this work

- Bump to edition 2024 (clippy is already clean).
- README: the new commands, the plugin install, both install routes.

## Left to the builder

- Crate choices: notifications (e.g. `notify-rust`, which covers all three OSes), whether to
  upgrade `rodio`, the short-duration parser.
- The cap on `--ring`.
- How tests keep quiet: e.g. an environment variable that turns sound and notifications into log
  lines, used by integration tests.
- Exact wording of the skill and the human-readable output.

## As built (2026-10-22)

Where the shipped code differs from the text above:

- **Workflows are staged in `ci/workflows/`**, not in `.github/workflows/`: the cloud session that
  built this could not push workflow files (its GitHub access lacks the `workflow` permission), so
  `build.yml` is not yet replaced. `ci/workflows/README.md` has the activation steps; merge only
  once `check.yml` is green on all three OSes.
- **Notifications:** notify-rust on Windows and Linux only. On macOS, notify-rust's `show()` always
  returns `Ok` and uses an API deprecated from macOS 14, so macOS uses `osascript` with the title
  and body passed as arguments.
- **Decoding:** rodio 0.16 with symphonia (flac, mp3, vorbis, wav) instead of rodio's default
  decoders: the default minimp3 decoder aborts debug builds when probing a non-MP3 file.
- **Built-in sounds** are synthesised sample buffers at 48 kHz (not `SineWave` sources).
- **`CLI_TIMER_QUIET`** is quiet when present with any value other than exactly `0`.
- **Empty labels/messages** count as none; `fires_at` is printed in whole seconds.
- **Hyphen-leading** `remind -l` values and `ding` messages are accepted (`allow_hyphen_values`);
  the skill passes `--json` before `--` and the message.
- **Durations** are capped at 100 days; `--ring` at 10 minutes.
