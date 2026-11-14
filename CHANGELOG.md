# Changelog

This file records notable changes to GitCraft. It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- A maintained bug and optimization checklist tracks priorities, ownership, reproduction evidence, and verification before issues are marked fixed.

- A test workflow runs frontend and native regression suites on Windows, Ubuntu, and macOS for pull requests and pushes to main. Platform setup and desktop validation instructions document how to test all three targets.

- An optional strip of coloured ticks beside the graph's scrollbar (Preferences > UI Customization > Show markers beside the graph scrollbar) can mark branches, tags, remote branches, the checked-out commit and the selection. It is off by default, so the graph looks as before.

- Desktop notifications are off by default, sent only while GitCraft is in the background, several at once become "You have N new notifications", an unclicked one is dismissed after 20 seconds, clicking one brings GitCraft forward and opens the bell menu, reading or removing a notification closes its pop-up, and turning the setting on shows a "Desktop notifications enabled" pop-up.

- The graph draws date lines between commits ("20 hours ago", "yesterday", "a month ago", ...) while the date column is hidden or a search is active. The status bar gains a what's-new gift icon and a Support link.

- Keyboard: Shift+Down / Shift+J follows the first parent and Shift+Up / Shift+K goes to the only child (or the WIP row from the checked-out commit); Ctrl+Shift+Z also redoes. The status bar shows the first pull request's state sentence (for example "owner/repo#9 is in draft") once the repository's service is connected, and opens the Launchpad when clicked.

- The bell in the tab bar is now a notification center: unread badge (50+ from fifty), All / Read / Unread filter, newest first with Today / Yesterday / date labels, mark as read or unread, remove, and Mark all as read / unread. Failed actions and available updates appear there. File > Sign Out removes the saved service sign-ins and shows "Successfully signed out". The status bar says "Connect an integration" until a service is connected, then "Select an integration" for repositories on a service you have not connected. The start screen gains Intro Tutorials and Documentation links.

### Changed

- Load independent hosting services and repository summaries with bounded concurrency. Batch configuration and submodule reads, cache ignored-file classifications, and avoid retransmitting unchanged activity logs.
- Keep large file lists, folder counts, file-finder ranking, branch grouping, graph highlighting, and coding-session grouping responsive with bounded rendering and reusable indexes.
- Bound terminal input and output queues, let the renderer acknowledge consumed output, and cancel Git helpers together with their parent process.
- Automatically excerpt large AI contexts, prioritizing manifests and local changes and clearly summarizing omitted contents instead of collecting an unlimited request.

- Branch operations refresh their first history page promptly and restore older pages in the background. Session discovery reuses unchanged logs, Git object and reflog activity no longer triggers unnecessary graph refreshes, and AI context generation omits bulk dependency source before building patches.

- AI helpers summarize bulk vendored dependency imports and removals while retaining manifests, licenses, patch notes, and edits to existing dependency files. Large changes are no longer rejected by a fixed local byte limit; the selected provider enforces its context capacity.

- The repository landing page now showcases the desktop with an updated screenshot, feature highlights, platform downloads, and quality badges. Detailed setup and contributor guidance have their own pages.

- Graph labels combine local branches with their matching tracking refs, count additional grouped labels in a separate badge, and expand into one clean label stack with both local and remote actions available.

- The Windows terminal resolves the shell executable directly, avoiding an extra discovery process and launch failures when inherited environment names differ in casing.

- Startup loads the code editor and terminal only when needed. Repository tabs reuse their first read, append history in bounded pages, preserve graph lanes between pages, and release cached data when closed. Hidden tabs defer refresh work, graph calculations avoid unrelated updates, and checkout/merge watcher events are combined with the final action refresh.

- File and history previews automatically load files up to 2 MB, offer an explicit preview up to 20 MB, and keep larger files out of the in-app editor.

- The commit graph uses softer branch colours, connected labels, author pictures inside lane rings, and a translucent band linking each commit to its message. Message rows no longer have heavy filled backgrounds.

- When a remembered repository tab points at a folder that no longer exists, the error screen shows the folder and offers Try again, Locate folder and Close tab.

- Hovering a tag or branch label no longer tints every commit below it. Only the commit of a tag (or a branch's own stretch of unlabelled commits, down to the next labelled commit) stays bright while the other rows fade, and labels show a tooltip such as "v0.1.3 (Tag)". The selected row uses the selection colour, rows and buttons ease their hover colours, and menus, dialogs and notifications slide in gently. The tag icon now follows the tag name and the graph's column titles use a small monospace style.

- Cloning follows Git's usual order of options and offers more control: Shallow Clone can limit history by commit depth or by a since date (checked for the format and against future dates), clone a single branch, and pass extra flags; Sparse Checkout takes a list of paths (one per line, or only top-level files when empty) and an optional blobless clone.
- Creating a repository always makes an "Initial commit" (empty if there is nothing to add) and sets up Git LFS afterwards, so an LFS problem no longer stops the repository from being created. The optional README box now starts unticked.

- Initialize a Repository and Clone a Repository are now two-pane dialogs with a source list on the left (Local Only or Clone with URL, then GitHub, GitLab, Bitbucket and Azure DevOps). New repositories can be set up with Git LFS, and clones can use Sparse Checkout. The Profiles page shows a My Profiles card with a "keep .gitconfig updated" option, an Add a Profile button, and a table of profiles; the Add Profile dialog lets you pick a picture, name, author and whether to copy the current settings.

- Preferences now start with an Exit Preferences button at the top of the section list, followed by the current profile and the Preferences and Repo-Specific Preferences headings. A staged file's edit button reads "Edit in Working Directory".

- Update checks now repeat every 10 hours instead of every hour. The File menu shows the update lines that fit the situation: "Checking for updates...", "Restart and Install Update", "Check for Update (Last checked 5 minutes ago)", "Update Errored (2 minutes ago)" and "Downloading Update...". A failed check clears the "Last checked" time, and the wording of times follows the usual "a few seconds ago / an hour ago / 2 days ago" steps.

- After every release is published, GitHub Actions now moves the shipped entries from Unreleased into a dated version section of this changelog, updates the comparison links, and uses those entries as the release notes, so the changelog no longer falls behind. The missing 0.1.6, 0.1.7 and 0.1.8 sections were restored.

- Source comments, documentation and test names describe GitCraft on its own terms and no longer refer to other software.

### Fixed

- Protect existing files during repository initialization and submodule removal. Stash actions follow the selected stash, and discard Redo preserves its original file selection and refuses later edits.
- Reject stale hunk selections and history-edit plans. Internal patches use consistent file paths, and partial operations refuse non-UTF-8 patches rather than changing their bytes.
- Preserve complete interactive replay plans through conflicts and allow interrupted Gitflow finishes to resume without recreating release tags. Conflict recovery uses the correct Git operation.
- Keep profile identities, selected SSH keys, signing fingerprints, and case-sensitive repository paths consistent across settings changes.
- Check out the exact pull-request source commit, create pull requests from the configured push destination, and load all result pages. Server-side rebases finish before their merge is sent.
- Preserve new commit drafts while an earlier commit finishes. File editors retain layout and wrapping when switching files, and read or removed notifications dismiss their desktop pop-ups.
- Show untracked stash files with the correct previews and copy actions. Support SHA-256 blame, Unicode file names, spaced remote URLs, custom LFS hooks, file-based sparse clones, and unstaging or exporting changes before the first commit.
- Save pending preferences and tabs before closing or installing an update, and report incomplete LFS initialization while keeping the created repository available. Merge-backup cleanup waits for completion and lets you review new backup files before deletion.
- Keep complete shipped release notes when pending notes are reworded. Commit-message control characters remain message content instead of creating extra history entries.
- Preserve quoted custom arguments and literal repository/file placeholders. Windows file reveal uses the selected item's correct shell identifier.

- Discard preserves existing stashes when there is nothing new to discard. Partially completed composed commits remain undoable, and repository mutations run in order.
- File and conflict-editor saves refuse to overwrite newer disk changes. AI conflict suggestions cannot replace output after its file or edits have changed.
- Renamed files retain their original contents in previews, status errors are reported instead of showing a clean repository, and large ref lists no longer block history recovery.
- Conflict warnings refresh when local target branches move. AI helpers include untracked text files, and release publication waits for all platform test jobs.

- Repository refreshes explicitly sort ref signatures with Unicode-aware comparison and a deterministic tie-breaker, preserving loaded history when refs arrive in a different order.

- Undo and redo refuse to move a different branch or overwrite commits made after the recorded action. Undoing branch creation also preserves branches that have since changed.
- Applying a single stashed file refuses to overwrite current staged, unstaged, untracked, or ignored contents. Clearing a remote's custom push URL now restores the pull URL as its push destination.
- Switching repository tabs preserves file-editor drafts, conflict-resolution selections, and commit-panel input. Full refreshes preserve loaded history and older commit selections, and delayed status reads cannot replace a newer repository snapshot.
- AI commit-message generation preserves drafts edited while it runs and offers the generated result for explicit review instead of replacing newer text.
- Conflict readers respect the configured repository encoding. Interactive rewording works when a repository path contains apostrophes.
- Automatic update checks run at startup and hourly while GitCraft remains open.
- Commit-row selection uses its native button, the agent-session filter focuses only when opened, and changelog/remote-link parsing handles long whitespace or slash runs without excessive backtracking.

- The graph's date lines ("1 hour ago", "yesterday") and the optional scrollbar markers now only appear when the list is long enough to scroll, instead of always. When you hover a label, the other rows' text fades to a dim colour after a short pause rather than turning translucent.

- An outdated test of the AI commit-message request was corrected; the frontend (61) and Rust (63) test suites now run and pass.

### Security

- Keep AI connection settings isolated per request so concurrent calls use their selected provider and account.
- Block authenticated hosting redirects and redact credentials from service errors. Send GPG passphrases through private input and quote selected SSH key paths literally.

- Release recording validates version strings and matches existing release headings literally, removing command-line input from regular-expression construction.
- Updated the embedded browser runtime to Wry 0.57.2, the latest compatible stable patch release.
- Linux builds use the upstream GLib string-iterator soundness fix backported to the released version compatible with stable Tauri. A Linux release check exercises the affected iterator methods; the dependency upgrade is blocked by Tauri's GTK3 version constraint.

- Windows external-editor launches pass file paths directly to resolved executables or safely escaped launcher scripts, preventing filenames from being interpreted as additional shell commands.

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
