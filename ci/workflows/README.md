# Staged GitHub workflows

These belong in `.github/workflows/`. The cloud session that wrote them could not push there
(its GitHub access lacks the `workflow` permission, which also rules out deleting the old
`build.yml`), so they are staged here, where GitHub does not run them.

- `check.yml`: fmt, clippy and tests on Linux, Windows and macOS.
- `release.yml`: a pushed `vX.Y.Z` tag builds Windows, macOS (arm64 and x86_64) and Linux
  binaries and publishes a GitHub release. A manual run (`workflow_dispatch`) is a dry run that
  uploads the archives as workflow artifacts and publishes nothing.
- `.github/workflows/build.yml` is the old Windows-only release job. `release.yml` replaces it.

To activate, from a machine with normal git access, as one commit:

    git mv ci/workflows/check.yml .github/workflows/check.yml
    git mv ci/workflows/release.yml .github/workflows/release.yml
    git rm .github/workflows/build.yml
    git commit -m "ci: activate check and release workflows, drop build.yml" && git push

Then delete this directory too, since the workflows it describes are gone, and update the CI line
in `CLAUDE.md` (it still says the workflows are "staged"):

    git rm -r ci/workflows

Do the move on a short branch with a pull request, so `check.yml` runs before it lands, and merge only once
`check.yml` is green on Linux, Windows and macOS. It is the first time the code is compiled on
macOS and the tests run on Windows and macOS.

Then, once it is on the default branch, run the `release` workflow by hand (Actions, release,
Run workflow) and confirm all four archives appear before pushing a tag.
