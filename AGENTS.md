# GitCraft agent instructions

This directory is the root of the public GitCraft app repository. Use its Git history and tags when preparing app commits and release notes.

## Maintain the changelog

- Review [CHANGELOG.md](CHANGELOG.md) before every app commit. Include an entry in the same commit for every notable change to the app or how it is built, installed, or updated. Describe what users or contributors will notice in plain language. Skip entries for changes with no notable effect, such as formatting or test-only edits.
- Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Put pending changes under `## [Unreleased]`, using only the applicable categories: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, and `Security`. Keep dated releases newest first and link them to the GitHub release or comparison.
- Only ever add entries under `## [Unreleased]`. Do not create version sections or edit the comparison links by hand: after each release is published, the **changelog** job of `.github/workflows/release.yml` (`scripts/changelog-release.mjs`) moves the entries that shipped into a `## [x.y.z] - date` section on `main`, updates the links, and the same entries become the GitHub release notes. Pull that commit before your next change. If the job could not push, run `node scripts/changelog-release.mjs record <version> MystifiedSky/GitCraft <changelog-of-the-released-commit>` yourself.
- Each push to `main` builds the tip of that push and adds the workflow run number to the patch component of the base version in `tauri.conf.json`, so several commits pushed together share one version, and failed runs or manual dispatches can consume run numbers. Check the published version with `gh release view -R MystifiedSky/GitCraft` rather than guessing it.
- Keep entries about GitCraft itself, in plain language. Do not name or compare other products, and do not cite where a behaviour was learned from, anywhere in the repository (code, comments, messages, documentation, tests).
