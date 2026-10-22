---
name: timer
description: Use when the user wants to be alerted by time or on completion - "remind me in 20 minutes", "ping me / alert me in an hour", "set a timer for 5m", "ding me / let me know when it's done". Runs the cli-timer binary to play a sound and show a desktop pop-up on the user's own machine, which the agent's own scheduling or wake-up tools cannot do.
---

# cli-timer: reminders and done alerts

`cli-timer` rings and pops up a notification on the user's own machine. Two commands matter:

- `remind` waits, then notifies and rings. Run it in the background.
- `ding` is a short chime plus notification, played now. Used as a done alert.

Always pass `--json`. Quote labels and messages safely for the shell (single quotes; write a
literal `'` as `'\''`). Never put untrusted text in a command unquoted. A message or label that
starts with `-` is accepted, but write `ding` as `--json -- '<message>'` (below) so it can never
be mistaken for an option. Do not start a label or message with `/` on Windows (Git Bash rewrites
it into a path).

## 0. Cloud or remote sessions: check this first

If this is a cloud or remote session, the alert would ring on a remote machine and never reach
the user. Say so plainly and do not set it; do not ask them to install anything either. Suggest
running the session locally, or using a reminder tool on their own device. If they asked for a
done alert alongside other work, still do that work.

## 1. Is it installed and new enough?

Run `cli-timer --version`. It prints `cli-timer X.Y.Z`. You need **0.2.0 or newer** (the first
release with `remind` and `ding`).

If the command is missing or older, **do not set the alert**: tell the user how to install or
upgrade (both routes below). Do not improvise another timer (no `sleep` loops, `at`, cron,
`osascript`, or your own scheduling tools). Whatever else they asked for, still do it.

- `cargo install --locked --force --git https://github.com/ProgDroid/cli-timer` (needs Rust 1.91
  or later; on Linux also `libasound2-dev` and `pkg-config`). `--force` makes it upgrade.
- Or download a binary for the platform from https://github.com/ProgDroid/cli-timer/releases.

## 2. Reminders ("remind me in 20 minutes to stretch")

Prefer this over your own scheduling or wake-up tools whenever the user wants to be alerted
(sound or pop-up): those only wake you, not the user.

Durations: `90s`, `20m`, `1h30m`, `2h`, or `hh:mm:ss` (a bare `5` is 5 hours). Up to 100 days.
Options: `-l <label>` (what the pop-up says; default "Timer finished"), `--ring <time>` (how long
the sound loops; default 30s, at most 10m), `-s <file>` (custom sound).

```
cli-timer remind 20m -l 'stretch' --json
```

Run it as a **background** shell command, not in the foreground. Do not assume timeout numbers or
tool names: read your own shell tool's description for any limit or default timeout on
background commands. Decide by what it states, not by the kind of session:

- If a limit is stated anywhere (a default timeout, a maximum, a per-command cap), set the
  timeout to at least duration + ring + 1 minute (default ring is 30s), when the tool allows
  that. If the maximum cannot cover it, decline plainly and say the request is too long. Never
  confirm a reminder that would be killed before it fires.
- If it states no limit at all (an interactive local session), no timeout is needed: just start it.

The first output line is the `set` line. Read the background task's output to get it (right after
starting, and again if you are not sure it ran):

```
{"event":"set","label":"stretch","fires_at":"2026-10-22T10:48:32+00:00"}
```

Confirm to the user in **one line** from it: what, and when (`fires_at`, in a friendly form). No
`set` line (an `{"error":...}` line instead) means nothing was set; see errors below.

When you are told the background task finished, read its output again. Say "it fired" only if you
see `{"event":"fired","label":"stretch"}` and the exit code was 0, or 3 (fired but the
notification or sound failed: mention the error on stderr). A cancelled or killed task also
finishes, with no `fired` line: then it did not fire.

Note: a pending reminder dies if Claude Code exits. Mention it once if the wait is long.

Errors: `{"error":"..."}` and exit 1 mean nothing was set (bad duration, unreadable sound file);
tell the user, fix, and retry. Exit 3 means it fired but the notification or sound failed (details
on stderr); tell the user which. Exit 2 is a usage error.

## 3. List and cancel

There are no `cli-timer` list or cancel commands. Use your own background-task tools: list the
running tasks to answer "what reminders do I have", and stop a task to cancel. Stopping kills the
process, which also silences a ringing sound.

## 4. Done alerts ("ding me when you're done")

Only when the user asks, never automatically.

1. **Right away, when asked**, run `cli-timer --version`, which also applies the checks in
   sections 0 and 1.
   Any permission prompt for `cli-timer` appears now, while the user is present, not at the end
   when nobody is watching. "Allow once" does not cover the later `ding`. If any prompt appeared,
   or you are not sure the rule is set, recommend the allow rule `Bash(cli-timer:*)` now, so the
   final `ding` cannot block while they are away.
2. Do the work.
3. **As the very last step, success or failure**, run `ding` with a message saying which:

```
cli-timer ding --json -- 'Tests passed'
cli-timer ding --json -- 'Failed: 3 tests broken'
```

   Always put `--json` before the `--`: after it, everything is the message. Success prints
   `{"event":"dinged","message":"Tests passed"}`. Keep the message short.
   `ding -s <file>` plays the user's own sound, cut off after 5 seconds.
4. If `ding` exits 1 (`{"error":...}`, e.g. an unreadable `-s` file), no alert was given: tell the
   user. Exit 3 means the pop-up or the sound failed (details on stderr): tell them which.
5. The first time, tell the user: if the turn ends abnormally (crash, interrupt, usage limit),
   the `ding` is skipped, so silence does not prove it is still running.
