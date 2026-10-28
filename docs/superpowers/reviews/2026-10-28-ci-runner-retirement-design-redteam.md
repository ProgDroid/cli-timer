# Red team: CI runner retirement design

Spec: `docs/superpowers/specs/2026-10-28-ci-runner-retirement-design.md` (bead `clitimer-195`).
Reviewer stance: hostile staff engineer, looking for why this is the wrong thing to build.
Date: 2026-10-28.

## What I measured (so the objections stand on evidence, not reading)

All Docker runs on this machine. Scratch artifacts were in the session scratchpad and are gone
once the session ends. The commands are below so anyone can re-run them.

1. **v0.2.3 released Linux binary** (`gh release download v0.2.3 -p 'cli-timer-x86_64-unknown-linux-gnu.tar.gz'`,
   `readelf -V` / `readelf -d` in `ubuntu:24.04`):
   - NEEDED: `libasound.so.2`, `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, `ld-linux-x86-64.so.2` (5).
   - Version needs: 23 distinct = 18 `GLIBC_*` (2.2.5 to 2.34) + 2 `ALSA_*` (0.9, 0.9.0) + 3 `GCC_*`
     (3.0, 3.3, 4.2.0). 18 + 2 + 3 = 23. The spec's "18 distinct versions" is the GLIBC subset only.
2. **The same source built on `ubuntu:24.04`** (glibc 2.39, toolchain 1.99.1 from
   `rust-toolchain.toml`, `cargo build --release --locked`, `REAL_EXIT=0`):
   - The newest `GLIBC_` version is now **2.39**. It comes from exactly two symbols, both **WEAK**
     undefined: `pidfd_spawnp@GLIBC_2.39` and `pidfd_getpid@GLIBC_2.39` (Rust std's process
     spawning). Below that, the newest strong version is still 2.34.
3. **That 24.04-built binary run on older distros** (`CLI_TIMER_QUIET=1 cli-timer --version`, ALSA
   runtime installed):

   | Image | glibc | Result |
   |---|---|---|
   | `debian:12` | 2.36 | runs, exit 0, prints `cli-timer 0.2.3` |
   | `ubuntu:22.04` | 2.35 | runs, exit 0 |
   | `rockylinux:9` (RHEL 9) | 2.34 | runs, exit 0 |

   Every one also prints this to stderr on **every invocation**:
   `libc.so.6: weak version 'GLIBC_2.39' not found (required by cli-timer)`.
4. **The retirement dates** ([actions/runner-images#14254](https://github.com/actions/runner-images/issues/14254)):
   deprecation from 2026-09-17, retirement 2027-04-17; brownouts on March 23, March 30, April 6 and
   April 13, each 14:00 to 00:00 UTC. The spec's dates are right. "Late March and early April" is
   loose, though: April 13 is mid-April, and these are 10-hour windows, not whole days.

Only `--version` was run on the old distros. The pidfd fallback path (std spawning a child when
`pidfd_spawnp` resolves to null) was not exercised. `notify-rust` and the `ding` pop-up spawn nothing
on Linux as far as I know, but I did not check that.

## The three objections

### 1. (b) Assumed, not established: "pinning 24.04 moves the floor and drops 22.04, Debian 12 and RHEL 9." Measured: it does not.

The whole design rests on this trade: Non-goals ("keeping old distros working ... the developer
does not care"), Approach C dropped because "its only payoff is old-distro reach", the README line,
and the `debian:12` smoke test "expected to fail". The binary built on 24.04 still **runs** on glibc
2.34, 2.35 and 2.36. The 2.39 requirement is a *weak* version: the loader complains and carries on.

Two things follow.
- **The verification method is wrong for the new binary.** "Read the newest `GLIBC_` symbol
  version" (Decided, and the README line under My calls) would report 2.39. The README would then
  tell Ubuntu 22.04, Debian 12 and RHEL 9 users that the binary needs Ubuntu 24.04 or Debian 13,
  which is false. The method only worked for v0.2.3 by accident, because that binary had no weak
  versions. The floor that decides whether it runs is the newest *strong* (non-`VER_FLG_WEAK`)
  version. `readelf -V` shows the flag per entry.
- **The pre-registered smoke test will be falsified, and the spec has no branch for that.** In
  `debian:12` the binary exits 0. A builder following the spec either "fixes" a test that was
  right, or reports the floor "proved" by a container that never failed.

### 2. (c) The collision in 6 months: the cost is a stderr warning on every run, and nothing stops the floor drifting.

The design's actual user-visible effect on older distros is not a "cryptic loader error". It is
a loader warning on stderr every time `cli-timer` runs. Ubuntu 22.04 keeps standard support until
April 2027, and Debian 12 and RHEL 9 are supported for years, so these users exist exactly when this
ships. This project has already spent effort keeping stderr clean (`sound::open_output` exists to
suppress rodio's stderr line for `remind --json`, see `docs/parking-lot.md`), and the Claude Code
plugin calls the binary where an agent reads that stderr. The spec never mentions the warning.

The anti-goal ("a glibc minimum that rises without anyone deciding it") is also **not met** by
pinning the runner. The floor is the runner's glibc combined with what the Rust std and the
dependencies reference. The 2.39 references came from **std**, not from the image choice alone. The
CLAUDE.md toolchain-bump routine (change `channel`, fix lints) and the next dependency refresh can
turn a weak reference strong, or add a new one, with `ubuntu-24.04` unchanged. The spec measures
once, on this branch, and relies on a YAML comment afterwards. Within 6 months there will be at
least one toolchain bump and probably the rodio rewrite (parking lot). Nothing in the release
workflow would notice either one moving the floor.

### 3. (a) A simpler design that gets 80% of the value: the one-line pin plus a release-time assertion. Drop the README floor and the Docker ritual.

- Pin `ubuntu-24.04`. This one line delivers the deadline goal. Keep it.
- Add one step after `Build` in the Linux row: `readelf -V` the binary and fail if any *non-weak*
  `GLIBC_` version is newer than a number written in the workflow (today 2.34). This runs on every
  tag and every dry run. It is the only mechanism that actually enforces the anti-goal, it costs
  about five lines, and it replaces the manual Docker smoke test, which proves the floor once and
  then goes stale.
- Leave the README floor out, or state 2.34 if the assertion holds it there. A README number with
  no check behind it is the "metadata is not state" trap. The README already names the one runtime
  dependency users actually hit (`libasound2`).
- If the stderr warning matters (objection 2), the cheap fix is to build where the weak symbols do
  not appear, which is Approach C. Its payoff is then "no warning on supported LTS distros", not
  "old-distro reach", so the spec's reason for dropping C does not hold.

## Fixed check: paired facts and the claim a reader infers

| # | Sentence (paraphrased) | Inferred third claim | Supported? |
|---|---|---|---|
| 1 | "needs glibc 2.34 at most (newest `GLIBC_` symbol version, 18 distinct versions found). That covers Ubuntu 22.04, Debian 12 and RHEL 9." | (i) The newest GLIBC symbol version *is* the floor, so the same method works for the new build. (ii) 18 versions is the binary's whole set of version requirements. (iii) It was run on those three distros. | **(i) Unsupported implicature**, and false for the 24.04 build (weak 2.39, strong 2.34). (ii) False: 23 version needs = 18 GLIBC + 2 ALSA + 3 GCC; libasound is a second runtime floor. (iii) Inferred from version numbers, never run. (I ran the *24.04* build on all three; it works, with a warning.) |
| 2 | "`ubuntu-latest` is moving to 26.04 ... `check.yml` and the publish job use it" + non-goal "`ubuntu-latest` there is fine" | Linux CI still covers what ships. | **Unsupported.** After this change, CI builds and tests on 26.04 and the release builds on 24.04, so the shipped Linux build is exercised only by the manual dry run. (That gap already exists today with 22.04 versus 24.04; the spec does not create it, but "is fine" asserts it away.) The 26.04 move itself rests on a third-party guide, which the spec flags. |
| 3 | "The README promises no glibc version or distro" + non-goal "keeping old distros working" | Dropping old distros costs no users or promises. | **Unsupported implicature.** A missing promise is not a missing user; README line 68 does name "Debian and Ubuntu" with no version. Mostly moot anyway: measurement shows nothing is dropped, only warned at. |
| 4 | Deprecated 2026-09-17 / removed 2027-04-17 / four brownouts | Any release before late March 2027 is safe on 22.04. | **Supported** (verified against #14254). Imprecision: the last brownout is April 13, and each is a 14:00 to 00:00 UTC window, not a day. |
| 5 | Approach A "glibc minimum only changes when someone edits that line" + anti-goal | Pinning the image freezes the floor. | **Unsupported.** The floor also moves with the toolchain and dependencies (the 2.39 references are Rust std's). Displaces nothing on its own; it is objection 2. |
| 6 | Approach C "only payoff is old-distro reach, which is a non-goal" | A loses nothing C would have kept. | **Unsupported.** C (zigbuild with `x86_64-unknown-linux-gnu.2.34`, or an older container) also avoids the weak-2.39 warning and decouples the release from runner retirements. |

Residuals decomposed: v0.2.3 has 23 version needs = 18 GLIBC + 2 ALSA + 3 GCC; NEEDED = 5 libs.
For the 24.04 build, newest GLIBC = 2.39 from 2 weak symbols (`pidfd_spawnp`, `pidfd_getpid`); with
those removed, newest = 2.34, unchanged from v0.2.3. The ALSA and GCC counts for the 24.04 build
were not re-listed (UNKNOWN; expected unchanged).

Flagged implicature 1(i) is the strongest finding in this review, and it is folded into objection 1.

## Re-run commands

```sh
# Floor of a built binary: strong vs weak version needs
readelf -V cli-timer          # look for "Flags: WEAK" on the GLIBC_2.39 entry
readelf --dyn-syms -W cli-timer | grep -E 'GLIBC_2\.(3[5-9])'
# Run on old distros
docker run --rm -v "$PWD":/b debian:12 sh -c 'apt-get update -qq && apt-get install -yqq libasound2 >/dev/null; /b/cli-timer --version; echo $?'
```
