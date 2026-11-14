# Changelog

This file records notable changes to GitCraft. It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Desktop notifications are off by default, sent only while GitCraft is in the background, several at once become "You have N new notifications", an unclicked one is dismissed after 20 seconds, clicking one brings GitCraft forward and opens the bell menu, reading or removing a notification closes its pop-up, and turning the setting on shows a "Desktop notifications enabled" pop-up.

- The graph draws date lines between commits ("20 hours ago", "yesterday", "a month ago", ...) while the date column is hidden or a search is active. The status bar gains a what's-new gift icon and a Support link.

- Keyboard: Shift+Down / Shift+J follows the first parent and Shift+Up / Shift+K goes to the only child (or the WIP row from the checked-out commit); Ctrl+Shift+Z also redoes. The status bar shows the first pull request's state sentence (for example "owner/repo#9 is in draft") once the repository's service is connected, and opens the Launchpad when clicked.

- The bell in the tab bar is now a notification center: unread badge (50+ from fifty), All / Read / Unread filter, newest first with Today / Yesterday / date labels, mark as read or unread, remove, and Mark all as read / unread. Failed actions and available updates appear there. File > Sign Out removes the saved service sign-ins and shows "Successfully signed out". The status bar says "Connect an integration" until a service is connected, then "Select an integration" for repositories on a service you have not connected. The start screen gains Intro Tutorials and Documentation links.

### Changed

- Preferences now start with an Exit Preferences button at the top of the section list, followed by the current profile and the Preferences and Repo-Specific Preferences headings. A staged file's edit button reads "Edit in Working Directory".

- Update checks now repeat every 10 hours instead of every hour. The File menu shows the update lines that fit the situation: "Checking for updates...", "Restart and Install Update", "Check for Update (Last checked 5 minutes ago)", "Update Errored (2 minutes ago)" and "Downloading Update...". A failed check clears the "Last checked" time, and the wording of times follows the usual "a few seconds ago / an hour ago / 2 days ago" steps.

- After every release is published, GitHub Actions now moves the shipped entries from Unreleased into a dated version section of this changelog, updates the comparison links, and uses those entries as the release notes, so the changelog no longer falls behind. The missing 0.1.6, 0.1.7 and 0.1.8 sections were restored.

- Source comments, documentation and test names describe GitCraft on its own terms and no longer refer to other software.

### Fixed

- An outdated test of the AI commit-message request was corrected; the frontend (61) and Rust (63) test suites now run and pass.

## [0.1.8] - 2026-11-12

### Added

- Tag menus can push or delete one tag on a selected remote. Merge-commit cherry-pick and revert menus let you choose the parent, with a confirmation describing that choice.

- Commit menus now include checkout choices, worktree creation from the selected commit, rebase onto the commit, a separate annotated-tag action, and branch visibility actions. Reset soft, mixed, and hard modes are also available on branch labels, including at the current commit.

### Changed

- Staging sections and commit options expand smoothly and respect reduced motion. The repository start screen stacks its columns in narrow windows, diff controls wrap, and keyboard focus is easier to see.

- Menus and dialogs fade in smoothly; grouped labels expand without stretching text. Single branch labels stay stable on hover, rapid tab reorders keep one continuous slide, and reduced-motion preferences apply throughout the interface.

### Fixed

- Failed stashes keep their typed title. Copying staged file contents uses the index, comparisons use the displayed revision, and deleted files use their previous revision. Commit and comparison file lists show errors with Retry and explain empty results. Repository searches distinguish no matches from an empty recent list, and sample-project preparation ignores repeated clicks.

- Clicking commit messages and graph details selects their row. Fast-forward suggestions now check ancestry rather than commit ordering.

- Worktrees start from the selected commit and use the correct folder separator. Pull-request creation pushes the selected branch without checking it out or pushing a different branch.

- Reset rejects unknown modes. Undo and redo of a hard reset protect newer edits, reset replay stays on its original branch and expected commit, and failed undo/redo remains available to retry. Annotated tags keep their annotation when restored through undo or redo.

- Merge previews, normal and squash merges, merge pulls, and Gitflow finishes now share the same rename detection settings. Personal Git settings no longer make the preview and merge use different rename rules.

## [0.1.7] - 2026-11-12

### Added

- Rename an existing commit with a “Write with AI” button that uses that commit’s saved changes. Review and edit the suggested message before saving, or restore the original.

- Hovering a commit expands its grouped branches and tags into an animated stack of colored labels. Each label supports selection and its own right-click actions, with keyboard navigation and reduced-motion support.

### Changed

- Commit menus now say “Rename commit”, keep common actions near the top, and group advanced history edits in a submenu. Menus use the darker background, rounded corners and full-width separators of the reference design.

### Fixed

- Repository summaries now count each new file inside a folder, honor the same rename threshold as the file lists, and count a staged rename or a file changed in both staging and the working folder once.

- Selecting a saved commit now keeps the working-directory file count and View Changes button visible. Files changed in both staging and the working folder are counted once.

- Agents now groups Codex CLI and Claude Code sessions under worktree cards, with file counts, agent selection, resume actions, and loading/error states. Saved activity no longer claims an agent is running. Starting or resuming a session uses the correct worktree and configured CLI folder, and opening a hidden terminal no longer loses the launch request.

## [0.1.6] - 2026-11-12

### Added

- Fetch model lists in AI Preferences and pick a default model or an override for each helper. Find and verify the installed Codex CLI or Claude Code without running an AI request.

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

- Opening files, folders and web links now asks Windows directly instead of starting Explorer, so a file is shown selected in its folder and a failure is reported ("Failed to open path").
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

[Unreleased]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.8...HEAD
[0.1.8]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/MystifiedSky/GitCraft/releases/tag/v0.1.1
