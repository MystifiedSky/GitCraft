# GitCraft agent instructions

This directory is the root of the public GitCraft app repository. Use its Git history and tags when preparing app commits and release notes.

## Track bugs and optimizations

Use [BUGS_AND_OPTIMIZATIONS.md](BUGS_AND_OPTIMIZATIONS.md) as a concise checklist of unresolved work only. Check existing IDs and Git history before adding findings or starting a fix; do not reuse retired IDs. Verify the original failure and record regression evidence in the fix commit or work summary, then remove completed entries. Keep pending validation concise. Do not accumulate completion checkpoints, rejected candidates or historical narratives in the checklist; Git preserves that history. Leave existing historical material in AUDIT.md unchanged unless requested.

## Maintain the changelog

- Review [CHANGELOG.md](CHANGELOG.md) before every app commit. Include an entry in the same commit for every notable change to the app or how it is built, installed, or updated. Describe what users or contributors will notice in plain language. Skip entries for changes with no notable effect, such as formatting or test-only edits.
- Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Put pending changes under `## [Unreleased]`, using only the applicable categories: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, and `Security`. Keep dated releases newest first and link them to the GitHub release or comparison.
- Add new entries under `## [Unreleased]`. The Release workflow compares the release commit with the previous published tag, so published bullets should stay as written until archived. Use the procedure in [RELEASING.md](RELEASING.md) to archive shipped entries in dated sections through a pull request.
- Each push to `main` builds the tip of that push and adds the workflow run number to the patch component of the base version in `tauri.conf.json`, so several commits pushed together share one version, and failed runs or manual dispatches can consume run numbers. Check the published version with `gh release view -R MystifiedSky/GitCraft` rather than guessing it.
- Keep entries about GitCraft itself, in plain language. Do not name or compare other products, and do not cite where a behaviour was learned from, anywhere in the repository (code, comments, messages, documentation, tests).
