# GitCraft agent instructions

This directory is the root of the public GitCraft app repository. Use its Git history and tags when preparing app commits and release notes.

## Maintain the changelog

- Review [CHANGELOG.md](CHANGELOG.md) before every app commit. Include an entry in the same commit for every notable change to the app or how it is built, installed, or updated. Describe what users or contributors will notice in plain language. Skip entries for changes with no notable effect, such as formatting or test-only edits.
- Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Put pending changes under `## [Unreleased]`, using only the applicable categories: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, and `Security`. Keep dated releases newest first and link them to the GitHub release or comparison.
- Before assigning a version, check the latest published release (`gh release view -R MystifiedSky/GitCraft`) and recent runs of `.github/workflows/release.yml`. Each push to `main` builds the tip of that push and adds the workflow run number to the patch component of the base version in `tauri.conf.json`. Several commits pushed together share one version, and failed runs or manual dispatches can consume run numbers. Treat the next version as expected until GitHub publishes it.
- On the next app change after a release publishes, move the shipped entries out of `Unreleased` into a section for the actual tag and publication date (`YYYY-MM-DD`). Leave anything that did not ship under `Unreleased`, and update the comparison links and expected next version. Fold this into the next app commit so it does not trigger a release just to record the previous one.
