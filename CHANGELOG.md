# Changelog

This file records notable changes to GitCraft. It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

Expected next release: `0.1.6`.

### Added

- Rename an existing commit with a “Write with AI” button that uses that commit’s saved changes. Review and edit the suggested message before saving, or restore the original.

- Hovering a commit expands its grouped branches and tags into an animated stack of colored labels. Each label supports selection and its own right-click actions, with keyboard navigation and reduced-motion support.

- Fetch model lists in AI Preferences and pick a default model or an override for each helper. Find and verify the installed Codex CLI or Claude Code without running an AI request.

### Changed

- Commit menus now say “Rename commit”, keep common actions near the top, and group advanced history edits in a submenu. Menus use the darker background, rounded corners and full-width separators of the reference design.

### Fixed

- Repository summaries now count each new file inside a folder, honor the same rename threshold as the file lists, and count a staged rename or a file changed in both staging and the working folder once.
- Selecting a saved commit now keeps the working-directory file count and View Changes button visible. Files changed in both staging and the working folder are counted once.
- Agents now groups Codex CLI and Claude Code sessions under worktree cards, with file counts, agent selection, resume actions, and loading/error states. Saved activity no longer claims an agent is running. Starting or resuming a session uses the correct worktree and configured CLI folder, and opening a hidden terminal no longer loses the launch request.

## [0.1.5] - 2026-11-11

### Fixed

- GitCraft checks for updates as soon as its saved preferences load, then hourly while it’s open. A downloaded update keeps its bottom-right restart notice until you install it.

## [0.1.4] - 2026-11-11

### Added

- Use your ChatGPT or Claude subscription for commit and stash messages, change explanations, pull request drafts, and other AI helpers. GitCraft connects to the account you signed into with the installed Codex CLI or Claude Code.

## [0.1.3] - 2026-11-11

### Added

- The toolbar shows a spinner while Pull, Fetch, Push, Stash, or Pop is running. The corresponding button ignores repeat clicks while busy, and automatic background fetches do not trigger the spinner.
- Repository tabs can be dragged to rearrange them (Escape cancels), moved with Move Tab Left / Right or Ctrl+Shift+PageUp / PageDown, and the tab strip scrolls with the mouse wheel.
- The "// WIP" row in the graph is a small box you can type the commit summary into, with the change counts beside it, and Ctrl+Enter commits from it.
- Copy a link to a commit, branch or tag on its hosting site, and open a remote's page in the browser.
- File menus can copy a file's contents, open it in your editor or its default program, and show it selected in the file manager.
- Hide or show all local branches, remote branches or tags from a section header's right-click menu.
- Home and End select the first and last commit in the graph, and error notices have a button to copy the message.

### Changed

- Opening files, folders and web links now asks Windows directly (as the original does) instead of starting Explorer, so a file is shown selected in its folder and a failure is reported ("Failed to open path").
- Start-up brings back all the tabs from last time at once, with the last active one showing; each other repository loads when you first open its tab, instead of the app opening and flashing through every repository.
- Stashing without a message now names the stash "WIP on <branch>" (or "WIP on (no branch)" when no branch is checked out), and new files come back staged after Pop because Stash stages everything first.

### Security

- The release build now pins its two third-party GitHub actions (the Rust toolchain installer and the Rust build cache) to exact commits, so a moved tag can't change what runs when installers are built and signed.
- The release build installs front-end packages with `--ignore-scripts`, so a dependency's install script can't run during the build that signs updates.

### Fixed

- Exported patches now consistently use three context lines and keep separate hunks separate, even when personal Git diff settings specify other values.

- The tab-list arrow now sits at the right end of the tab strip. Pressing Escape or cancelling a repository tab resize restores its starting width.

- A repository whose remote default-branch pointer (origin/HEAD) is empty or points at a deleted branch now shows its history instead of an empty graph. One damaged branch no longer hides the rest of the history.
- Stashing when there is nothing to stash now reports "No local changes to save" as an error instead of a success.
- Use the keyboard to select repository and terminal tabs, browse the graph and file lists, dismiss notifications, and select commits in interactive rebase. Confirmation dialogs focus Cancel first.
- Font choices sort alphabetically in Preferences, and long or incomplete custom date formats no longer cause excessive processing.
- GitCraft reports unexpected startup and repository picker errors instead of silently leaving the operation incomplete.

## [0.1.2] - 2026-11-11

### Added

- Repository tabs can be resized by dragging their edge or using the keyboard. GitCraft remembers the width for each repository, and double-clicking the edge restores the default width.

### Changed

- The tab strip scrolls when tabs no longer fit and brings the active tab into view.

## [0.1.1] - 2026-11-10

### Added

- First public release of GitCraft, with Windows NSIS and MSI installers, a universal macOS DMG, and Linux AppImage, DEB, and RPM packages.
- GitHub Actions builds installers for each push to `main` and publishes signed updates for installed copies.

[Unreleased]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/MystifiedSky/GitCraft/releases/tag/v0.1.1
