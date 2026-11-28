# Changelog

This file records notable changes to GitCraft. It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- The File, Edit, View and Help menus share the window title bar, as in VS Code, saving a row of space. Turn it off in Preferences → UI Customization. ([#65](https://github.com/MystifiedSky/GitCraft/pull/65))
- Force-remove a worktree with local changes after a separate confirmation explains which files and branches will be deleted.
- Create a repository on a linked hosting service, choose its destination and visibility, and open the new local copy. Connect an account from the Create Repository dialog and return to the same form.
- Use your GitHub Copilot subscription for GitCraft’s AI helpers. Sign in with GitHub from Preferences → AI or reuse your Copilot CLI sign-in.
- Move the commit details panel below the commit graph or back to the right from the panel header or the View menu, and resize it in either layout. ([#57](https://github.com/MystifiedSky/GitCraft/pull/57))
- Live agent status: set up Claude Code or Codex in Preferences > External Tools, and the Agents view shows whether each session is running, waiting for you or done. Claude Code's permission requests appear on the worktree card with the command it wants to run, to allow or deny there; an unanswered request is asked in the terminal as before. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- The graph marks the commit each other worktree has checked out with a count of its uncommitted changes; click it to open that worktree. Turn it off in Preferences > Agents. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Drag a branch label onto another in the graph and pick an action from the list: merge, rebase, fast-forward, push, or start a pull request, each with a one-line explanation. Merge and rebase check out the branch they act on, and fast-forward moves the branch that is behind. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56)) ([#60](https://github.com/MystifiedSky/GitCraft/pull/60))
- Stacked pull requests: starting a pull request from a branch made on another open pull request's branch targets that branch, and the form names the pull request it stacks on. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Pull requests from a repository without its own template use the GitHub organization's default template. Commits made with a GitHub no-reply address show that account's GitHub picture. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Recompose commits with AI: from a commit's menu, or for a run of selected commits that ends at the checked-out commit, the commits are undone and their changes composed again into new commits for you to review. Undo brings the originals back. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Auto-resolve with AI explains each conflict it resolved and how confident it is (high, medium or low), shown above its suggestion in the conflict editor. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- A "Merged #N" pill shows when a branch's pull request was merged: beside the checked-out branch in the toolbar, on worktree cards in the Agents view, and in Commit Details for the commit it ended at. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Select a run of consecutive commits to see their combined changes in the right panel, below the actions for the selection. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Remove a worktree and delete its branch (locally, or locally and on the remote) in one step. A branch checked out in another worktree offers this instead of Delete. Sort the Agents worktree list by latest activity or branch name, and jump to the terminal an agent is running in. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Start an agent session in a new worktree with one click, or from an issue's menu with a branch named after it. Cursor CLI is available as a coding agent. New worktrees keep the hidden, soloed and pinned branches of the repository they came from. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Explain your working changes with AI from the commit panel. The last five commit and branch explanations are kept, so opening one again needs no new request (Explain Again asks anew). ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Restore a file from any commit by right-clicking it in the commit. Its current changes are kept as a backup you can restore from Actions. File View shows Markdown files rendered, with a button to switch between the preview and the code. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- GitCraft tells you when the system stops sending file change events, since changes made outside it may then not show until you refresh. Long choices in dialog lists show their full text on hover. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- The command palette offers "Open Repo in …" for every editor installed on this computer. Repository Management has resizable Repository, Location and Upstream columns that stay as you leave them. Choose a zoom level from the status bar. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Terminal tab shortcuts (Ctrl+Shift+N, Ctrl+Shift+W, Ctrl+PageUp/PageDown and Ctrl+Shift+1–9; Command+Option on macOS), listed in Keyboard Shortcuts. Minimize the terminal to its tab bar, end a session from the trash button, and drop files on the terminal to type their paths. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Hover an annotated tag in the left panel to read its message. With a branch pinned, a pin in the Graph column header jumps to it. Checking out a branch or commit shows its working changes. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Rename repository tabs, reopen individual recently closed repositories from the tabs list, and delete saved workspaces without touching their repositories. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- Find and navigate matches in File View, including while editing a file. Show syntax errors when editing JSON.
- Fold code, edit multiple selections, match brackets, and copy code with syntax colors. Find and Replace supports case, whole-word, and regular-expression matching. Folding and multiple selections also work in large files.
- Select and copy text from a large read-only file preview or either side of a full diff.
- Highlight invisible Unicode characters and lookalike letters in the file editor and full-file diffs. Accented text and regular emoji sequences keep their normal appearance.
- Undo cursor and selection moves with Ctrl+U or Command+U. The editor menu also lets you redo those moves.
- See additions and deletions in the split diff overview ruler. Click or drag it to navigate the file.
- Edit the same draft from File, Split, and Inline views. Use the split divider to resize the original and modified panes while editing.
- Check the English or Spanish spelling of commit summaries and descriptions, with corrections available from the right-click menu.
- Publish preview installers for volunteer testing and promote tested changes through a separate stable release run.

- Show stashes in the commit graph, each as a dashed box just above the commit it was made on and joined to it by a dashed line. Select one to see its changes, or right-click it for the stash actions.
- Double-clicking a remote branch whose local branch is behind it now fast-forwards that local branch after switching to it. Diverged branches are switched to but never merged, and local edits are never overwritten.
- When switching branches with uncommitted changes, choose to bring them along, stash them, commit first, or discard them after confirmation.
- Join the GitCraft community from the Help menu.
- Copy diagnostics from Help, open a repository in another window, and open clone URLs in a prefilled dialog.
- Bisect from the graph. Stash selected changes, retain staged changes, or stash only untracked files.
- Keep Undo and Redo across restarts. Review squash-merged, rebase-merged, and gone-upstream branches during branch cleanup.
- Edit and fix up commits during an interactive rebase, autosquash matching commits, and run a shell command after a selected commit.
- View CI checks and filter pull requests by state, author, or review request. Review changes and comment on diff lines in GitHub and GitLab.
- Render hosted descriptions and comments as Markdown. Edit squash messages and remove merged source branches after review.
- Publish to organizations and groups, and connect to Gitea, Forgejo, Codeberg, or Azure DevOps Server 2022.
- Lock and unlock Git LFS files, show who owns a lock, and keep lockable files read-only until locked.
- Search and follow links in the terminal, copy and paste from its context menu, and use the shell's Tab completion.
- Follow the system theme and high-contrast settings. Use Spanish for the main navigation and start screen.

- Build Windows and Linux ARM64 packages alongside the existing releases.

- Recover commits from the reflog, view signature verification in commit details, and reopen GitCraft with the previous window size and position.
- Keep new commit drafts across restarts, honor Git commit templates, and sign off commits from Commit options.
- Fetch earlier commits in shallow clones or download filtered objects in partial clones. Enable reuse of recorded conflict resolutions per repository.

- Filter repository history by author, path, added or removed text, or changed lines matching a regular expression. Open a file's containing folder history and page farther back in file history.
- Compare two versions of a commit series with range-diff. Run garbage collection, write the commit graph, or fetch and prune remotes from Repository maintenance.
- Choose a pull request template when the repository has multiple templates. Set pull mode, rebase autostash, and recursive submodule updates per repository.
- Highlight changed words in hunk diffs. Blame can use or bypass ignored revisions, and commit history shows canonical names from the repository's mailmap.

### Changed

- Auto-fetch now defaults to one minute for profiles without a saved interval.
- Repository tabs now widen to fit their name and only shorten it with an ellipsis when the strip is full, sharing the space with the other tabs. Dragging a tab's edge still sets a fixed width, and double-clicking it fits the name again. Dragging a tab to reorder it lifts it under the pointer, leaving a gap where it will land. ([#61](https://github.com/MystifiedSky/GitCraft/pull/61))
- Use consistent buttons in the branch drag-and-drop dialog, with a scrollable action list for smaller windows.
- The collapsed left panel lists Patches instead of Cloud Patches, matching the expanded panel. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Speed up release builds by reusing passing tests for the release commit and compiling macOS architectures in parallel. Verify update signatures before publishing. ([#55](https://github.com/MystifiedSky/GitCraft/pull/55))

- Open repositories faster: the first one is read while the window opens, and the first page of a long history no longer waits for Git to sort the whole history. On a Linux test machine a 50,000-commit repository showed its graph in about 110 ms instead of 340 ms. GitCraft also uses less memory, draws on the power-saving graphics card of a laptop with two, and falls back to OpenGL when Vulkan, Metal or DirectX 12 can't be used.
- Release builds are optimized across all crates, so the program is about a quarter smaller and the release build takes longer.
- In the commit graph, a branch's line runs straight down its own lane to the commit it grew from and joins that commit from the side, instead of bending into the other line just below the branch.
- GitCraft is now a native app drawn with egui instead of a web view, so it no longer needs WebView2 on Windows or WebKitGTK on Linux. It keeps your preferences, open tabs, commit drafts, profiles and window size and position, and installed copies update to it as usual, a copy installed from the MSI with the MSI. Installers are now built with cargo-packager; the base version moved to `Cargo.toml`. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- GitHub release notes group new changes, link to their pull requests, and credit contributors when GitHub identifies them. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- The GitHub bug form asks which operating system is affected and labels reports by platform. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- Document source setup with dependency installation scripts disabled.
- Build the desktop interface with a faster bundler.
- Update the diff and file editor while preserving offline language highlighting and JSON diagnostics.
- Update the libraries for native file dialogs, file watching, and terminal links.
- Pin the actions used to build, test, and publish release artifacts.
- Move the Rust backend into its own crate, `crates/gitcraft-core`, which no longer depends on Tauri. `cargo build` and `cargo test` cover the whole workspace.

### Fixed

- Generate an overall commit message for large change sets using a file summary and selected excerpts.
- Keep AI commit-message errors visible beside the commit draft, and report an empty AI answer without replacing the draft.
- Refresh remote branches in the graph when repositories open and on their own fetch schedules, even when another branch is checked out.
- Wait for GitCraft to close before running Windows updates, and prompt to close other GitCraft windows without forcing them to quit during the update.
- Remove stale worktrees whose Git link is missing while preserving their files. Delete selected branches only after the worktree has been removed.
- If a file is locked on Windows, Discard All names it and stops before stashing or removing changes.
- Open repositories and reveal files in Windows Explorer when Git reports paths with forward slashes.
- Keep dialog buttons inside the dialog box: wrap them onto new rows, shorten long labels with a tooltip, and truncate long titles. ([#60](https://github.com/MystifiedSky/GitCraft/pull/60))
- Drag and drop every branch in the list a commit's +N badge opens, not only the first label. ([#60](https://github.com/MystifiedSky/GitCraft/pull/60))
- Restore the GitCraft icon on Windows taskbar pins, shortcuts, and the executable in Explorer.
- Keep the Create Repository and Clone forms readable beside their source list, and stack the panels in narrow windows.
- Open branch cleanup in a larger, resizable dialog with scrollable results and visible Cancel and Delete buttons on smaller screens.
- Build Linux AppImage installers on x64 and ARM64 with their automatic update signatures. ([#59](https://github.com/MystifiedSky/GitCraft/pull/59))
- Keep commit, stash, and patch controls reachable when the changes panel is short, and give the bottom panel room for those controls when space permits. Keep the divider aligned with the pointer while resizing. ([#57](https://github.com/MystifiedSky/GitCraft/pull/57))
- Open the existing worktree when checking out a branch that is already checked out there. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Offer detached checkout directly in the commit context menu. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Apply Conflict Prevention preference changes immediately in Teams. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Keep the worktree-group colors visible along the bottom of repository tabs. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Recover staged, unstaged, and untracked edits after restoring a file from an older commit. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Recognize terminal number shortcuts when the operating system reports the shifted punctuation key. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Record the full keyboard shortcut when modifier keys are pressed before the final key. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Scroll terminal tabs into view when they overflow, leaving the toolbar controls accessible. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Keep Repository Management columns clear of branch information and show the current zoom level with a check mark. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Restore Windows placement in physical pixels and return to the primary display when the saved display is gone and the window is offscreen. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Respect modifier keys held at the time of a commit graph click, even if they are released in the same frame. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Put selection actions in the commit panel header, leaving more space for commits and combined changes. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Offer Recompose with AI when comparing two consecutive commits that end at the checked-out commit. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))
- Preserve the original file contents and modes when recomposing commits, including repositories with line-ending conversion. ([#58](https://github.com/MystifiedSky/GitCraft/pull/58))

- "Automatically group worktree tabs together" now works: tabs of worktrees of the same repository sit side by side and share a colored line. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Conflict Prevention now uses the repository's base branches and skips the excluded branches set in Preferences. "Delete .orig files after merging" and the submodule update preferences now take effect. ([#56](https://github.com/MystifiedSky/GitCraft/pull/56))
- Reject Fetch and Pull actions while their repository is busy, including from open menus, shortcuts and Repository Management. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Disable Pull and Fetch for repositories involved in automatic or bulk fetch and pull operations. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Avoid a crash when saving a Pull default with malformed repository preferences. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Identify each Pull menu default selector by its operation in its tooltip and accessibility description. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Choose the default Pull action independently from running a command in the Pull menu. The selected row spans the menu, and the toolbar shows Fetch while a fetch runs and prevents duplicate actions while the repository is busy. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Keep the Pull menu compact and explain its default selection; dim the Pull or Fetch button while the repository is busy. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Translate the Pull menu's default-selection guidance into Spanish and give its action buttons consistent widths. ([#54](https://github.com/MystifiedSky/GitCraft/pull/54))
- Restore RPM packages in Linux x64 and ARM64 releases. ([#53](https://github.com/MystifiedSky/GitCraft/pull/53))

- Fix a macOS build error in the native app's startup code. ([#50](https://github.com/MystifiedSky/GitCraft/pull/50))
- Stack the Start screen sections in narrow windows and apply the selected language to the Start screen, menus, and toolbar labels. Prevent duplicate sample-project requests. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- Keep failure messages in the notification center, limit bulk actions to the selected filter, and honor the notification position preference. Dismiss notifications or copy an error directly from its toast. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- Keep staged and unstaged files in the same scrolling area so an empty staged section leaves more room for changed files. ([#44](https://github.com/MystifiedSky/GitCraft/pull/44))
- Draw joined emoji and keycaps in the Windows editor using the installed emoji font.
- Open the full comparison automatically when a file exceeds the hunk diff limit.
- Keep the original and modified panes in sync when scrolling large or wrapped split diffs.
- Wait for the comparison to finish before taking a diff screenshot.
- Wrap long lines in Hunk View while keeping line-level staging available.
- Preserve the Windows MSI upgrade identity when moving from the web-view build to the native app.
- Keep ignored files safe when checking out a newly created branch or a pull request, or undoing or redoing a checkout.
- Keep history loading when a previous page request finishes late. Give commit rows accessible names and a visible keyboard focus indicator.
- Restore the Windows title-bar icon and toolbar spacing. Keep the empty Staged Files section visible while unstaged files scroll.
- Apply installed font choices in the file editor and terminal.
- Ask before discarding an unsaved file draft from the editor's Cancel button.
- Fix the Linux RPM package failing to install on Fedora and RHEL with "nothing provides git >= 2.38". The package now depends on `git`, and GitCraft still checks for Git 2.38 or newer when it starts. ([#42](https://github.com/MystifiedSky/GitCraft/pull/42))
- In the commit graph, a branch that shares a parent with a neighboring commit now connects to that parent instead of appearing to branch off the neighbor.
- Give the working-changes line its own lane above the checked-out commit so it stays clear of other commits.
- Show the first bad bisect commit on newer Git versions.
- Close repository windows independently, save their drafts, and stop their terminals. Stop managed subprocesses on app exit.
- Find build tools in the active or default installation when a desktop editor omits them from PATH.
- Preview merge conflicts with the minimum supported Git version and show filenames as they appear in the repository.
- Report an empty stash without changing the staging area.
- Limit concurrent DNS lookups for hosting integrations and retry briefly when the signing executable is busy.
- Let the newer release continue when GitHub blocks tagging a queued build that used an older workflow.
- Explain unknown and changed SSH host keys, and give install guidance before checking out files that require Git LFS.
- Poll file changes on network folders that lack reliable file notifications.

- Release notes list only changes since the previous published version, even when several releases are queued before the changelog is archived. ([#5](https://github.com/MystifiedSky/GitCraft/pull/5))

- Ask whether to save, discard or keep editing before leaving an unsaved file or conflict resolution.

- Launch external editors and terminals with platform-appropriate tools, use native window controls and select files in supported Linux file managers.

- Preserve server policy errors when a push is blocked, time out stalled Git commands, and clean up interactive replay files after completion. Rewording uses the configured Git executable and runs hooks by default.

- Preserve staged and unstaged changes separately when creating and restoring a stash.
- Recover recently discarded changes after a restart. Warn before removing a dirty worktree or amending an already-pushed commit.
- Recover corrupt settings from a backup and open secondary launches in the existing window.
- Name the clone folder, initialize submodules, and request older history from graph search.
- Validate branch and tag names as you type, make branch rename and pull undoable, and protect the default branch during cleanup.
- Report integration rate limits and network errors, and show provider-supported pull-request merge methods and blocking checks.
- Generate SSH keys with an optional passphrase and add them to the SSH agent. Hide unusable GPG signing keys.
- Preserve non-UTF-8 patch contents, report unreadable patches, and recognize nested Git LFS tracking patterns.
- Distinguish Control and Command shortcuts, show secondary shortcut clashes, sort release tags by version, and redraw the graph when display scaling changes.
- Time out background fetches without opening a sign-in prompt, skip refreshing unchanged refs, and help set an upstream before pulling.
- Use the upstream remote to suggest a conflict comparison branch and pull-request target. Escape Jira project filters and explain credential-store errors.
- Refresh browser-managed integration sign-ins when credentials expire. Check for Git 2.38 or newer and run lint and dependency checks in CI.
- Blame falls back to an ordinary blame when the repository's ignored-revisions list contains an invalid entry.
- Keep executable permissions when staging or committing in repositories stored inside WSL.
- Open a drive root from the Explorer "Open in GitCraft" menu.

### Security

- Update the Rust cookie and URL libraries to address two new security advisories.
- Limit GitCraft hosting credentials to explicit network commands and refuse editor saves that resolve outside the repository.

- Review untracked AI context before sending it, omit known credential files and redact common token formats.

- Apply the renderer content policy to Windows release builds.
- Cancel in-flight AI requests, keep AI credentials on the configured endpoint, and send Trello credentials in the authorization header.
- Open Windows Terminal in folders whose names contain a semicolon without running part of the name as a command.
- Refuse editor saves through symbolic links into a repository's Git metadata, and never read a blame revision as a Git option.
- Pin GitHub Actions to commit hashes and wait seven days before Dependabot proposes new dependency releases.

## [0.1.29] - 2026-11-18

### Security

- Pass desktop verification paths as browser-protocol arguments so they cannot become executable JavaScript.

## [0.1.27] - 2026-11-18

### Added

- An opt-in renderer content-policy configuration and source inventory support packaged security testing before the policy is enabled by default.

- Headless browser regressions now run in CI with downloadable reports and deterministic row, concurrency, and cache budgets.

### Changed

- Blame previews enforce input/output limits and render measured visible rows, including wrapped lines. Saved agent sessions initially show five recent entries per worktree; expanded archives remain virtualized and searchable.

- Worktree status reads prioritize visible cards and share four IPC slots. Session catalogs use a bounded incremental index with event updates and periodic recovery, and wide commit graphs transfer straight lanes as compact runs.

- Git reads have deadlines, process-tree cancellation and a shared concurrency budget. Long Git jobs keep diagnostic tails with truncation notices and wait for renderer acknowledgements before sending further progress events.

- Hosting requests reuse connections with request-specific credentials and interrupt stalled responses when their reader cancels. Pull requests, issues, comments, clone pickers and Launchpad load more results on demand. Launchpad providers arrive independently; failed continuations preserve displayed results and remain retryable. Azure Boards collections continue beyond fifty items.

- Load fewer editor features while retaining basic language grammars, diff editing, search, folding and clipboard actions. Expensive panels and repository layouts subscribe to the store fields they use, avoiding unrelated draft and preference updates.

### Fixed

- Keep agent session catalogs current when the configured directory uses an aliased path, including macOS temporary directories.

- Restore JSON language validation and detach editor models before disposal to avoid errors during rapid editor switches.

- Cancel auxiliary repository reads and queued read locks; cancelled history loads report failure instead of returning an empty repository.

- Preserve worktree identity and opening/actions when the worktree list is filtered. Restore branch-list row context menus by forwarding their trigger events and refs.

- Report file-history and related collection read failures explicitly, with history retry instead of an empty-history message.

- Retry temporarily unavailable avatars after a delay or reconnection, using normalized bounded caches.

- Retry failed repository watcher registrations and serialize close/reopen operations. Watcher startup and shutdown run outside the shared map lock.

- Publish editor saves from complete same-directory temporary files, retaining the original after write/replacement failure and preserving existing permissions and symlinks.

### Security

- Restrict desktop verification to local debug/release builds and safe report names, refuse artifact paths outside the checkout, and use explicit Windows tool paths. Pin the browser setup action to an immutable commit.

## [0.1.25] - 2026-11-17

### Fixed

- Make the mock AI server's accepted sockets blocking before timed reads, preventing intermittent macOS test failures on fragmented requests.

### Security

- Validate browser-debugging response IDs and registered callback types before dispatch. Review and document the vendored GLib scanning false positives without excluding them from future scans.

## [0.1.23] - 2026-11-17

### Added

- Commit-list column dividers can be dragged or adjusted with the keyboard. Widths are remembered, wide layouts scroll horizontally, and individual or all column widths can be reset.

### Changed

- The main README screenshot shows the current desktop layout and commit history.

### Fixed

- Development builds no longer warn about obsolete file-save helpers; file safety regressions exercise the production save path directly.

## [0.1.21] - 2026-11-15

### Added

- Add Remote now has URL, GitHub, GitLab, Bitbucket and Azure DevOps tabs. Connected accounts can create a hosted repository and push the current branch, with repository details, access selection, workspace/project selection where needed, and recovery after a partial failure.

## [0.1.20] - 2026-11-15

### Added

- Push now offers to add a remote when a local repository has none, opening the remote setup form directly from a cancellable prompt.

## [0.1.19] - 2026-11-15

### Fixed

- Made empty diff spacer rows inert so they stay out of keyboard navigation and assistive technology.

## [0.1.18] - 2026-11-15

### Fixed

- Keep virtual diff spacer rows decorative for assistive technology and wait for the reviewed branch and destination before completing rejected-push handling.

## [0.1.17] - 2026-11-14

### Fixed

- Fixed a garbled character in the loading message for historical blame.

## [0.1.16] - 2026-11-14

### Added

- A maintained bug and optimization checklist tracks priorities, ownership, reproduction evidence, and verification before issues are marked fixed.

### Changed

- Bound structured diff payloads and render visible hunk rows only. Large and densely wrapped comparisons use the virtualized editor with safe full-file actions; oversized partial patches are refused.

- Reject oversized clipboard file copies before loading their contents, with bounded reads for working files and Git blobs.

- Watch working directories selectively so ignored dependency and build trees do not generate unnecessary file events, while preserving tracked files and reacting to changed ignore rules.

- Load independent hosting services and repository summaries with bounded concurrency. Batch configuration and submodule reads, cache ignored-file classifications, and avoid retransmitting unchanged activity logs.

- Keep large file lists, folder counts, file-finder ranking, branch grouping, graph highlighting, and coding-session grouping responsive with bounded rendering and reusable indexes.

- Bound terminal input and output queues, let the renderer acknowledge consumed output, and cancel Git helpers together with their parent process.

- Automatically excerpt large AI contexts, prioritizing manifests and local changes and clearly summarizing omitted contents instead of collecting an unlimited request.

- Branch operations refresh their first history page promptly and restore older pages in the background. Session discovery reuses unchanged logs, Git object and reflog activity no longer triggers unnecessary graph refreshes, and AI context generation omits bulk dependency source before building patches.

### Removed

- Hide the unavailable worktree WIP graph control until its data and rendering support are implemented.

### Fixed

- Detect undecodable file and conflict previews, refuse ordinary saves that would replace original bytes, and require a correct encoding or explicit UTF-8 conversion.

- Give shared preference controls accessible captions and help text, with distinct names for path fields and Browse buttons.

- Redact remote-URL credentials, authentication headers and token-bearing query fields before storing or displaying Git diagnostics.

- Preserve unresolved files when taking a conflict side fails, and delete only when that side is genuinely absent.

- Record the exact created branch for Undo when a same-named tag exists.

- Include every requested repository in bulk fetch/pull even while summaries are filtered or loading, and report failures in the final count.

- Preserve existing ignore-file bytes, including non-UTF-8 rules and UTF-8 BOMs, and report read failures before untracking files.

- Use each revision’s historical filename when showing blame across renames.

- Keep topological navigation on visible commits at unloaded or filtered boundaries and explain how to load or reveal the parent.

- Refuse unsupported historical external comparisons instead of opening an unrelated working-tree diff. Named-file history and external-tool paths use literal filenames.

- Keep activity logs bounded across background jobs and hunk operations, and notify the viewer immediately when those entries arrive. Very large command/output entries now show a truncation notice.

- Restore upstream, push destination and other branch settings when undoing branch deletion, and refuse to overwrite externally changed branch settings.

- Refuse stashing during unresolved operations, and block every Commit All route before it can stage unresolved conflict markers.

- Apply the commit-message comment-cleanup preference to ordinary commits and amendments.

- Bind force-push confirmations to the reviewed branch, commit, destination and remote tip; reject stale confirmations and push an explicit commit with an atomic remote lease.

- Refuse remote-branch checkout when an existing local branch tracks a different remote, preserving both branch relationships and local work.

- Report failed automatic submodule updates and offer a safe retry while preserving the successful repository action.

- Pass HTTPS credential usernames as literal data so apostrophes and shell characters cannot break authentication.

- Read Git signing booleans consistently and write explicit Off values so included signing defaults can be disabled.

- Preserve unselected end-of-file newline changes when staging, unstaging or discarding selected lines.

- Treat bracketed and other special filenames literally in hunk views and named-file actions, and refuse mixed-file partial patches.

- Allow Undo and Redo for the first ordinary commit in an empty repository while preserving staged and working files and refusing intervening branch changes.

- Keep Git text-conversion output out of hunk staging, unstaging, discards and working-change patch exports so these operations preserve the original file content.

- Clearly use the configured server default for Bitbucket Server merges instead of offering merge strategies that were ignored.

- Prevent read-only filesystem notifications from causing repeated repository refreshes on Linux.

- Detect potential merge conflicts in both SHA-1 and SHA-256 repositories.

- Keep repository creation filesystem checks asynchronous, and share consistent loading and error states across history file lists.

- Correct Unix regression fixtures and keep upstream dependency duplication separate from GitCraft source duplication in quality analysis.

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

### Security

- Keep AI connection settings isolated per request so concurrent calls use their selected provider and account.

- Block authenticated hosting redirects and redact credentials from service errors. Send GPG passphrases through private input and quote selected SSH key paths literally.

## [0.1.14] - 2026-11-14

### Added

- A test workflow runs frontend and native regression suites on Windows, Ubuntu, and macOS for pull requests and pushes to main. Platform setup and desktop validation instructions document how to test all three targets.

### Changed

- AI helpers summarize bulk vendored dependency imports and removals while retaining manifests, licenses, patch notes, and edits to existing dependency files. Large changes are no longer rejected by a fixed local byte limit; the selected provider enforces its context capacity.

- The repository landing page now showcases the desktop with an updated screenshot, feature highlights, platform downloads, and quality badges. Detailed setup and contributor guidance have their own pages.

### Fixed

- Repository refreshes explicitly sort ref signatures with Unicode-aware comparison and a deterministic tie-breaker, preserving loaded history when refs arrive in a different order.

### Security

- Release recording validates version strings and matches existing release headings literally, removing command-line input from regular-expression construction.

- Updated the embedded browser runtime to Wry 0.57.2, the latest compatible stable patch release.

- Linux builds use the upstream GLib string-iterator soundness fix backported to the released version compatible with stable Tauri. A Linux release check exercises the affected iterator methods; the dependency upgrade is blocked by Tauri's GTK3 version constraint.

## [0.1.13] - 2026-11-14

### Security

- Updated the frontend test runner and its mocking dependency.

## [0.1.12] - 2026-11-14

### Fixed

- Undo and redo refuse to move a different branch or overwrite commits made after the recorded action. Undoing branch creation also preserves branches that have since changed.

- Applying a single stashed file refuses to overwrite current staged, unstaged, untracked, or ignored contents. Clearing a remote's custom push URL now restores the pull URL as its push destination.

- Switching repository tabs preserves file-editor drafts, conflict-resolution selections, and commit-panel input. Full refreshes preserve loaded history and older commit selections, and delayed status reads cannot replace a newer repository snapshot.

- AI commit-message generation preserves drafts edited while it runs and offers the generated result for explicit review instead of replacing newer text.

- Conflict readers respect the configured repository encoding. Interactive rewording works when a repository path contains apostrophes.

- Automatic update checks run at startup and hourly while GitCraft remains open.

- Commit-row selection uses its native button, the agent-session filter focuses only when opened, and changelog/remote-link parsing handles long whitespace or slash runs without excessive backtracking.

### Security

- Windows external-editor launches pass file paths directly to resolved executables or safely escaped launcher scripts, preventing filenames from being interpreted as additional shell commands.

## [0.1.11] - 2026-11-14

### Changed

- Graph labels combine local branches with their matching tracking refs, count additional grouped labels in a separate badge, and expand into one clean label stack with both local and remote actions available.

- The Windows terminal resolves the shell executable directly, avoiding an extra discovery process and launch failures when inherited environment names differ in casing.

- Startup loads the code editor and terminal only when needed. Repository tabs reuse their first read, append history in bounded pages, preserve graph lanes between pages, and release cached data when closed. Hidden tabs defer refresh work, graph calculations avoid unrelated updates, and checkout/merge watcher events are combined with the final action refresh.

- File and history previews automatically load files up to 2 MB, offer an explicit preview up to 20 MB, and keep larger files out of the in-app editor.

- The commit graph uses softer branch colours, connected labels, author pictures inside lane rings, and a translucent band linking each commit to its message. Message rows no longer have heavy filled backgrounds.

## [0.1.10] - 2026-11-14

### Added

- An optional strip of coloured ticks beside the graph's scrollbar (Preferences > UI Customization > Show markers beside the graph scrollbar) can mark branches, tags, remote branches, the checked-out commit and the selection. It is off by default, so the graph looks as before.

### Changed

- When a remembered repository tab points at a folder that no longer exists, the error screen shows the folder and offers Try again, Locate folder and Close tab.

- Hovering a tag or branch label no longer tints every commit below it. Only the commit of a tag (or a branch's own stretch of unlabelled commits, down to the next labelled commit) stays bright while the other rows fade, and labels show a tooltip such as "v0.1.3 (Tag)". The selected row uses the selection colour, rows and buttons ease their hover colours, and menus, dialogs and notifications slide in gently. The tag icon now follows the tag name and the graph's column titles use a small monospace style.

- Cloning follows Git's usual order of options and offers more control: Shallow Clone can limit history by commit depth or by a since date (checked for the format and against future dates), clone a single branch, and pass extra flags; Sparse Checkout takes a list of paths (one per line, or only top-level files when empty) and an optional blobless clone.

- Creating a repository always makes an "Initial commit" (empty if there is nothing to add) and sets up Git LFS afterwards, so an LFS problem no longer stops the repository from being created. The optional README box now starts unticked.

- Initialize a Repository and Clone a Repository are now two-pane dialogs with a source list on the left (Local Only or Clone with URL, then GitHub, GitLab, Bitbucket and Azure DevOps). New repositories can be set up with Git LFS, and clones can use Sparse Checkout. The Profiles page shows a My Profiles card with a "keep .gitconfig updated" option, an Add a Profile button, and a table of profiles; the Add Profile dialog lets you pick a picture, name, author and whether to copy the current settings.

### Fixed

- The graph's date lines ("1 hour ago", "yesterday") and the optional scrollbar markers now only appear when the list is long enough to scroll, instead of always. When you hover a label, the other rows' text fades to a dim colour after a short pause rather than turning translucent.

## [0.1.9] - 2026-11-14

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

[Unreleased]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.29...HEAD
[0.1.29]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.27...v0.1.29
[0.1.27]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.25...v0.1.27
[0.1.25]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.23...v0.1.25
[0.1.23]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.21...v0.1.23
[0.1.21]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.20...v0.1.21
[0.1.20]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.19...v0.1.20
[0.1.19]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.18...v0.1.19
[0.1.18]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.17...v0.1.18
[0.1.17]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.16...v0.1.17
[0.1.16]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.14...v0.1.16
[0.1.14]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/MystifiedSky/GitCraft/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/MystifiedSky/GitCraft/releases/tag/v0.1.1
