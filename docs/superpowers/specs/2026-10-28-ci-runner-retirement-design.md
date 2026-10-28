# CI runner retirement: Linux release build off ubuntu-22.04

Bead: `clitimer-195`. Short design (a small change). Source: the "CI runner retirement" entry in
`docs/parking-lot.md`. Red team: `docs/superpowers/reviews/2026-10-28-ci-runner-retirement-design-redteam.md`.

## Intent

`release.yml` builds the Linux archive on `ubuntu-22.04` so the binary links against an older
glibc and runs on more distros. GitHub deprecated that image on 2026-09-17 and removes it on
2027-04-17. Before that, four brownout windows (March 23, March 30, April 6 and April 13, 2027,
14:00 to 00:00 UTC) fail jobs on it on purpose
([actions/runner-images#14254](https://github.com/actions/runner-images/issues/14254)). A release
tagged in a brownout, or after removal, would fail. Move the build before then.

## Facts (measured 2026-10-28, Docker on the developer's machine)

- The v0.2.3 Linux binary (built on 22.04) requires glibc 2.34 and has no weak glibc versions.
- The same source built in an `ubuntu:24.04` container (glibc 2.39, Rust 1.99.1 from
  `rust-toolchain.toml`) requires glibc 2.34 strongly, plus `GLIBC_2.39` marked WEAK
  (`readelf -V`: `GLIBC_2.34 Flags: none`, `GLIBC_2.39 Flags: WEAK`). The red team traced the weak
  version to two Rust std symbols, `pidfd_spawnp` and `pidfd_getpid`.
- Run with `CLI_TIMER_QUIET=1`, the 24.04 build starts and exits 0 for `--version` and
  `ding --json hi` on `debian:12` (glibc 2.36), `ubuntu:22.04` (2.35) and `rockylinux:9` (2.34).
  On all three it prints one line to stderr on every run:
  ``<binary>: <libc path>: weak version `GLIBC_2.39' not found (required by <binary>)``. stdout,
  including the JSON line, is unchanged. On `ubuntu:24.04` it prints nothing extra. The v0.2.3
  binary prints nothing extra on any of the four.
- `ubuntu-latest` is moving from 24.04 to 26.04 between 2026-10-19 and 2026-11-19 (third-party
  migration guide, not GitHub's own notice). `check.yml` and the release's publish job use it.

## Goals

- The Linux release build runs on a supported runner image well past April 2027.
- A Linux user on an older distro who sees the loader warning can find out from the README that it
  is harmless.

## Non-goals

- Clean stderr on distros older than glibc 2.39 (the developer accepted the warning, 2026-10-28).
- Guarding the glibc minimum in CI. The red team proposed failing the release when a required
  glibc version rises above 2.34; dropped because it guards old-distro reach the developer does not
  care much about, and a release failing over it would be friction.
- Caching the release build. The parking-lot entry's note that `Swatinem/rust-cache` is only in
  `check.yml` is superseded by the entry's Done note; not worth adding for a few releases a year.
- arm64 Linux builds (a separate parking-lot entry).
- Changing `check.yml` or the publish job. CI tests on `ubuntu-latest` while the release builds on
  `ubuntu-24.04`; that gap exists today (24.04 versus 22.04) and the dry run covers the shipped
  build.

## Approaches

- **A. Pin `ubuntu-24.04` and accept the warning** (picked 2026-10-28). One line in
  `release.yml`, supported until about 2029. Nothing stops running on older distros; they print
  one loader warning per run.
- **B. `ubuntu-latest`.** Never needs touching, but GitHub moves the label (to 26.04 now) and each
  move can change what the binary requires without anyone choosing it.
- **C. Build inside an older container (for example `debian:12`) on the 24.04 runner.** Keeps
  stderr clean on older distros, for about 10 more lines (rustup and ALSA headers in the
  container), and the container image becomes the thing that ages. Not picked: the warning is
  harmless and old distros are not a priority.

## Decided

- `release.yml`: the Linux row's `os` becomes `ubuntu-24.04`. Its comment says why it is pinned
  rather than `ubuntu-latest`, and that older distros still run the binary with a harmless loader
  warning.
- README, the Linux note in the install section: the binary is built on Ubuntu 24.04; on older
  distros it still runs but prints a harmless ``weak version `GLIBC_2.39' not found`` line. No glibc
  number (it would go stale unchecked).
- Verify with a manual release run (dry run) from the branch: the Linux archive builds, and
  `readelf -V` on it shows `GLIBC_2.39` only as WEAK.
- Parking lot: remove the "CI runner retirement" entry once merged.

Amended after the plan red team and the final review (developer, 2026-10-28):
- The parking-lot entry is kept with a **Done** note recording the declined options, not removed.
- `plugin/skills/timer/SKILL.md` gets a line telling the agent the warning is harmless and may
  come before the first JSON line (the agent reads stderr; the README reader is not the only one).
- The README and YAML comment name the oldest measured distros (Ubuntu 22.04, Debian 12, RHEL 9):
  below glibc 2.34 the binary does not start, so an unbounded "older distros" was false.
- No version bump and no release: this changes only how the next release is built.

## Left to the builder

- Exact wording of the YAML comment and README line.
