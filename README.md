# cli-timer

Ever wanted a quick timer without having to resort to your browser to Google one? Here it is, right from your terminal.

It does three things:

- a full-screen countdown (`cli-timer -t ...`),
- background reminders that pop up and ring when the time is up (`cli-timer remind`),
- a short "done" chime for scripts and agents (`cli-timer ding`).

## The full-screen timer

`cli-timer -t 25m -s /path/to/sound/file -l "Optional label for when the timer goes off"`

`-s` is optional: without it you get a built-in alarm. The label is optional too.

Durations can be `hh:mm:ss` (one to three parts; the first part is hours, so a bare `5` is five
hours) or short forms like `90s`, `20m`, `1h30m` and `2h`. The longest is 100 days.

## `remind`: a reminder in the background

Waits without taking over the terminal, then shows a pop-up and rings. It prints one line per event.

    cli-timer remind 20m -l "stretch"

The label is what the pop-up says (default "Timer finished"). `--ring 1m` sets how long the sound
loops (default 30 seconds, at most 10 minutes), and `-s` picks your own sound. Nothing rings forever.

Add `--json` for one JSON object per line, which is easier for scripts to read:

    $ cli-timer remind 1s -l "stretch" --json
    {"event":"set","label":"stretch","fires_at":"2026-10-22T10:28:32+00:00"}
    {"event":"fired","label":"stretch"}

A bad duration or an unreadable sound file fails straight away (exit code 1, and
`{"error":"..."}` with `--json`) instead of hours later. Exit code 3 means it fired but the pop-up
or the sound failed; the reason is on stderr.

## `ding`: a short done alert

A pop-up and a chime, right now. It is meant to be the last thing a long job does.

    cli-timer ding "Tests passed"

With `--json`:

    $ cli-timer ding "Tests passed" --json
    {"event":"dinged","message":"Tests passed"}

The message defaults to "Done". `-s <file>` plays your own sound, cut off after 5 seconds.

## Install

You need a recent stable Rust (1.91 or later). On Linux you also need `libasound2-dev` and
`pkg-config`.

    cargo install --locked --git https://github.com/ProgDroid/cli-timer

To upgrade, run it again with `--force` (without it cargo may refuse to reinstall the same version):

    cargo install --locked --force --git https://github.com/ProgDroid/cli-timer

Or skip Rust and download a ready-made binary for Windows, macOS (Apple silicon or Intel) or Linux
from [Releases](https://github.com/ProgDroid/cli-timer/releases). Unpack it and put `cli-timer`
somewhere on your `PATH`. Each archive has a `.sha256` file next to it. The Windows and Linux
binaries are 64-bit x86 only (use `cargo install` on ARM), and the Linux one needs the ALSA runtime
library (`libasound2` on Debian and Ubuntu).

Check it worked with `cli-timer --version`.

## Claude Code plugin

The plugin teaches Claude Code to use `remind` and `ding`, so "remind me in 20 minutes to stretch"
or "ding me when the tests are done" rings and pops up on your own machine. Install `cli-timer`
first (above), then in Claude Code:

    /plugin marketplace add ProgDroid/cli-timer
    /plugin install cli-timer@cli-timer

Claude Code asks before running a command it hasn't run before. To stop that getting in the way
(especially for the final `ding` while you're away), allow the binary in your settings:

    Bash(cli-timer:*)

Alerts only reach you in local sessions. In a cloud or remote session the sound and pop-up would
happen on the remote machine, so the plugin tells you instead of setting a reminder that nobody
will hear. A pending reminder also ends if Claude Code exits.

## Tested

Real sound and pop-ups are checked by hand on Windows. macOS and Linux are built and tested in CI
with alerts in quiet mode, but their real pop-ups and sound have not yet been checked by a person.
