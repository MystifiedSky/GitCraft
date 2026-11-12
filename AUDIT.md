# GitCraft audit — 2026-11-09

## Follow-up feature, motion and visual audit — 2026-11-12

Opened all four reference/picture-1.png through picture-4.png again. Reviewed the current start screen, menu builders, graph and sidebar menu routing, staging/commit panel, diff/image view, shared menus/dialogs, API action types, action refresh/loading/authentication paths, Git tag/history/file-content implementations, and the prior audit's remaining gaps.

| Area | Finding and result |
| --- | --- |
| Tag actions | Added per-remote Push Tag and Delete Remote Tag menus, available from both graph labels and sidebar tags through the shared builder. Deletion requires confirmation, retains the local tag, and uses a fully qualified refspec so a same-named branch is unaffected. Push preserves annotated tag objects and never forces overwrites. Backend rejects missing remotes, invalid names and missing local push tags. |
| Merge commit actions | Cherry pick used to fail without a mainline; revert silently chose parent one. Both now offer each parent with SHA and available summary. A confirmation explains the baseline; the backend resolves the commit and validates the selected parent before mutation. Ordinary commit actions retain their direct behavior. Multi-commit cherry-pick planning still lacks per-merge parent choices. |
| Staged contents | Copy file contents incorrectly read the working folder. It now reads the index for staged files, including when later unstaged edits exist. |
| Comparison contents | Comparison file menus omitted the displayed revision. They now copy the destination revision, or the old revision for deleted files. Saved-commit deleted files use their first parent. This is text copying, not a complete historical external-editor workflow. |
| Commit/range file lists | Errors no longer look like an empty result: show the error and Retry. Successful empty results explain that no files changed. Loading has a status role. |
| Stash message | A failed stash no longer clears the typed title. |
| Staging motion | New grid-height and opacity transitions preserve folder collapse state across section toggles and keep closed controls inert. Commit options use the same transition; staging header chevrons rotate without replacing their glyph. No graph row translation or scaling was introduced. |
| Motion accessibility | Expansion immediately settles with reduced motion, using the existing global policy plus an explicit section override. Closed content is hidden from assistive technology and keyboard focus. Actual frame pacing and live original motion equivalence remain unverified. |
| Start screen | Preserves the wide two-column reference arrangement, stacks at 760 CSS px, allows the search input to shrink, distinguishes no matches from no history, uses the platform shortcut label, and disables repeated sample preparation clicks. |
| Diff controls | Toolbar wraps when space is narrow; toggles expose accessible names and pressed state; before/after image previews have text alternatives. |
| Keyboard/long labels | Added visible keyboard focus for buttons and links. Long contextual labels can wrap inside the viewport cap. Shared dialog viewport constraints and existing menu collision handling retained. |
| Reference fidelity | Kept existing sourced colors, graph lanes/node sizes, fonts, top-bar structure, file tree and commit controls. Added only modest text empty states and responsive behavior. |

New timing/layout values are explicitly estimates in mechanics.json: section expansion 160 ms, fade 120 ms, start-screen stacking at 760 CSS px. The originals' still images establish arrangement, not these motion timings.

Added meaningful regression cases: merge menu parent selection/confirmation, remote tag target/confirmation, index versus worktree copy; Rust local bare-repository tag lifecycle with an annotated tag and same-named branch, merge mainline semantics and invalid-choice no-op behavior, and distinct saved/index/working text. Standalone npm build, npm test and cargo test commands were refused by the session policy ("requires approval, but approval policy is never"). These regression assertions have not been executed; rebuild_check compiles the application, not test-only code.

The first fresh rebuild_check passed build, sample startup/drawing (1395×1004), normal startup/drawing and mechanics. Opened checks/round-0/screenshot.png and window.png and compared with all four references: dark surfaces, toolbar, graph/ref lane and staging/commit arrangement remain consistent. Automated likeness was approximately 68% sample / 63% normal against capture/window.png; repository contents, restored panel selection and physical window scale differ. Final check after all source edits also passed build, sample drawing at 1395×1004, normal drawing and mechanics (144 values; 126 carry a source description, 22 are explicitly estimated; categories overlap). Opened both newly regenerated screenshots and compared with the four originals again. Likeness remained approximately 68% sample / 63% normal against capture/window.png; no motion or full behavioral equivalence is claimed.

Remaining gaps: hosted cloud patches and synced teams/workspaces, CI/status badges, author/team graph filters, terminal autocomplete, localization, per-merge choices in batch history operations, guarding every historical undo type, cross-platform and large-repository testing, full keyboard/motion walkthroughs, and configured auth/signing/LFS/update integrations. Earlier audit entries retain their historical status; nothing here verifies cloud-service parity or live remote behavior.


## Feature, bug, and animation audit — 2026-11-12

### Scope and evidence

Reviewed the three new attachments (pictures 14–16), all four original reference pictures, the extracted original context-menu translations (en-us.json, especially lines 996–1166), original CSS motion rules, the existing feature inventory, and current source for menus, graph selection, ref stacks, tabs, toolbar loading, shared dialogs, staging/diffs, worktree creation, hosting/PR creation, reset/undo/tag actions, and related state/API paths. Earlier audits below remain historical evidence; their tests were not silently promoted to passing.

This audit distinguishes working source implementations, changes made here, confirmed gaps, and workflows still requiring behavioral testing. Screenshots cannot prove timing, smooth frame delivery, Git correctness, or service integration. No original cloud service, remote repository, account, AI provider, or updater was mutated for this audit.

### Original menu coverage

| Original action / surface | Result of audit |
| --- | --- |
| Reset current branch to a selected commit: soft / mixed / hard | Already implemented but absent from branch-label menus, and incorrectly disabled at HEAD. Shared submenu now appears on both commit and branch menus and stays available at HEAD. |
| Soft keeps changes; mixed resets index; hard discards changes | Backend modes remain explicit; invalid mode strings now fail instead of silently selecting mixed. Target resolves to a commit before execution; redo records the resolved SHA. |
| Hard-reset confirmation | Retained, with Cancel initially focused and explicit explanation that discarded work cannot be restored. |
| Undo/redo reset with newer edits | Fixed unsafe hard replay: requires a clean index/worktree, including untracked files. Original offers automatic stashing; GitCraft instead asks the user to stash/commit first. |
| Undo/redo after switching branches or external HEAD movement | Reset replay now verifies original branch and expected HEAD. A mismatch leaves the action available for retry. Other historic undo action types still need equivalent external-change checks. |
| Checkout submenu on a commit | Added local and remote branch choices on that commit plus explicit detached checkout. Current local branch is disabled in the submenu. |
| Create worktree from this commit | Added to commit menu with ref choices. New branches receive the selected SHA instead of defaulting to HEAD. |
| Create worktree from a branch/ref | Available from local and remote branch-label menus using that ref's commit as the new branch start. Existing-worktree opening remains in the Worktrees panel. |
| Rebase current branch onto selected commit | Added near the top of commit menu. Branch-label normal and interactive rebase were already implemented. |
| Create branch here | Present in both menus; retained. |
| Cherry pick / revert selected commit | Present; retained. Merge revert uses first parent; selectable mainline remains a gap. |
| Merge / squash merge into checked-out branch | Present on branch labels; retained. |
| Fast-forward branch | Already present; one menu incorrectly inferred ancestry from row ordering. It now proves ancestry from loaded parents, conservatively omitting an unproven suggestion. |
| Rename commit / history edits | Present; retained rename, reword, squash, drop, move, interactive rebase under existing history controls. |
| Push and start PR | Exposed in commit menu; existing branch-label action retained. Backend now pushes the explicitly selected local branch, without switching checkout. |
| Explain Branch Changes | Existing AI action retained and also exposed for a ref at the selected commit. Actual AI results require a configured provider and were not requested. |
| Delete local / remote branch | Existing branch-label actions retained with remote confirmation and current-local-branch protection. No remote deletion was exercised. |
| Hide / Solo / Pin to Left | Branch-label actions already existed. Commit menus now expose branch pin/solo actions for refs on that commit. |
| Copy branch name / SHA / remote link | Existing branch actions retained; branch name and visibility entries also accessible from commit menu. Links use compatible hosting URLs. |
| Create patch from commit | Existing local patch export retained with the pictured wording. |
| Share commit as Cloud Patch | GAP: GitCraft exports/applies local patch files; it does not implement GitKraken's cloud upload/share service. |
| Create tag here / annotated tag here | Existing optional annotation was hidden in a general tag prompt. Added an explicit annotated entry; annotation is required there and a missing-message retry retains the tag name. |
| Undo annotated tag creation/deletion | Fixed: stores/reuses the tag object rather than its peeled commit, preserving annotation and tagger metadata. |
| Push/delete tags on remotes | GAP: dedicated tag-menu remote actions are absent. |
| Multi-commit cherry pick / comparison / history planning | Present elsewhere in the graph/rebase views; source implementation, not a new end-to-end verification. |
| Multi-ref batch menus and remote/local combined delete | GAP: no full parity with all original grouped-ref batch operations. |

### Concrete bugs fixed

1. Reset missing from branch labels and disabled at the current commit.
2. Clicks on commit-message/details content could miss the overlay button and fail to select the row; selection now handles the row's bubbled click while ref buttons retain their own behavior.
3. Worktree creation lacked a selected start SHA and defaulted to the checked-out commit.
4. Worktree folder suggestions always used Windows separators; they now follow the repository path.
5. A PR created from a non-current branch could push the current branch; the new pushBranch action uses an explicit source ref and its configured remote/destination.
6. A PR result could open in whichever tab was active when submission started; it now locates the repository's tab after completion.
7. Hard-reset undo/redo could discard edits made after the original reset.
8. Reset undo/redo could run against a different branch or externally moved HEAD.
9. Failed undo/redo popped and lost the record. It now remains on the correct stack for retry.
10. Tag undo/redo recreated an annotated tag as a lightweight tag.
11. Unknown reset modes silently became mixed resets; they now fail.
12. Fast-forward eligibility used commit display order, which does not establish ancestry.
13. A single ref label was unnecessarily hidden/recreated in a hover portal, causing a visual replacement with no additional labels to reveal.
14. Ref expansion scaled text vertically; it now fades/translates without distorting letters.
15. Tab reorder animations accumulated on rapid reorders and ignored OS reduced motion; previous animations are cancelled, their current offset is retained, and reduced-motion slides are skipped.
16. Shared dialogs could extend beyond narrow windows; their width now has a viewport cap.

### Animation audit

| Surface | Original evidence / current behavior | Outcome |
| --- | --- | --- |
| Pull / Push / Stash / Pop loading | Original ToolbarButton circle-notch and .fa-spin use a 500 ms turn. Existing per-repository loading ownership is retained, including explicit branch pushes. | Retained sourced speed and visible labels. The earlier 500 ms minimum remains a documented GitCraft adaptation. |
| Repository tab reorder | Original sortable transition is 250 ms ease with 20 px drag activation. | Retained timing/threshold; improved interruption and reduced-motion handling. Runtime frame pacing is unmeasured. |
| Grouped branch/tag labels | Pictured colored stacked labels with individual context menus; prior GitCraft unfold was 160 ms with scaleY(.85). | 120 ms ease-out, 2 px travel, no text scaling. These timings are estimates, not recovered original constants. Single labels remain anchored. |
| Context/dropdown menus | Pictured dark rounded surfaces and full-width separators. No exact menu animation duration established by the still images. | Added quick opacity-only entry (100 ms) and exit (70 ms), retaining Radix keyboard navigation and collision handling. Timings are GitCraft choices. |
| Dialog entry | Original CSS slow-fade enter uses 200 ms opacity. | Adapted a 200 ms opacity-only entry for shared dialog/overlay; exit remains immediate because the wrapper unmounts. |
| Terminal focus dim | Original .xterm opacity transition is 150 ms; current terminal already uses transition-opacity and opacity-60. | Retained; no additional slide or zoom added. |
| Sidebar/file section chevrons | Current transition-transform already communicates expansion. | Retained and now covered by reduced-motion policy. Animated expansion of section contents is still absent. |
| Reduced motion | Previously only ref expansion/filter transitions were suppressed. | CSS animation and transition duration suppressed throughout; imperative tab motion explicitly skips animation. Loading labels and functional state remain visible. |
| Graph updates / staging lists | Current virtual graph and optimistic staging update immediately. | No animated graph relayout, staged-file travel, or cross-view panel expansion added; original equivalence remains unproven. |
| Full animation quality | Static captures cannot establish frame pacing or compare movement to a live original. | Needs a recorded motion walkthrough under normal/reduced motion, rapid hover/reorder, scrolling, and slower hardware. No blanket equivalence claim. |

### Broader feature inventory and remaining work

| Area reviewed against inventory / current surfaces | Current coverage and outstanding gaps |
| --- | --- |
| Repository open / clone / init / recent/favorite repos | Implemented; original start-screen layout preserved. Filesystem/network failure end-to-end coverage remains limited. |
| Tabs / aliases / resize / grouping | Implemented, with persistent mounted repository panels from earlier work. Rapid reorder source issues fixed here. |
| Graph / hidden refs / solo / pin / ghost refs / search / extra columns | Implemented. Dedicated author/team filters, complete batch-ref menus, animated layout changes and matching large-repository benchmarks remain absent/unverified. |
| Working directory / staging / hunk and line operations | Implemented, including index-only refresh and earlier conflict/diff fixes. Filtered diff operations and CRLF cases need the full regression run. |
| File history / blame / images / editor / external tools | Present in source. Cross-platform launcher behavior and all binary/encoding cases still need end-to-end verification. |
| Conflicts / rebase / history rewriting | Present in source. Automatic stash-before-reset-undo, arbitrary-merge mainline selection, and guarded replay for every undo type remain gaps. |
| Worktrees / submodules / Gitflow / sparse checkout | Implemented. Commit-start worktrees corrected here. Submodule update partial failures and platform paths need live filesystem walkthroughs. |
| Authentication / GCM / SSH / signing / LFS | Source implementations exist; real service login/token expiry, key prompts, locks and signing were not tested. |
| PR / issue / hosting integrations | Source implementations exist; selected-branch push fixed here. CI/build-status badges, provider-specific permissions and non-default/multiple-remote PR routing remain unverified or incomplete. |
| AI helpers / model discovery / local and CLI connections | Implemented in earlier work; this audit makes no live AI request. Models, authentication and generated-content quality require configured providers. |
| Agents / session history / terminals | Existing worktree grouping and session launch/resume retained. Full multi-tab terminal/session lifecycle automation is still needed. |
| Settings / themes / profiles / shortcuts / updates | Present. Dialog viewport and motion accessibility improved; localization, complete keyboard focus audits and real signed-update installation remain outstanding. |
| GitKraken account/cloud features | Cloud patches, synced profiles/workspaces, teams, Code Suggest, licenses/trials and hosted organization controls are not reproduced. Local analogues are not cloud parity. |
| Terminal productivity | Existing terminal and focus dim retained. Original-style autocomplete remains absent. |
| Platforms / scale | Windows build/startup checked. macOS/Linux/WSL login forwarding, very large graphs and low-end animation performance require separate platform/load testing. |

Suggested order for remaining work: execute the full test suites and automate the pictured menus; add remote tag actions and merge-mainline choice; guard the remaining undo types and expose partial submodule failures; validate real hosting/auth/signing/LFS/update workflows; then record and compare motion before adding graph or panel movement. Cloud-service features require a separately defined service scope.

### Validation for this pass

- Built-in rebuild_check passed the first source build, sample startup/drawing at 1395 × 1004 and normal startup/drawing. Fresh screenshot.png and window.png were opened alongside all four references and capture/window.png. Palette, top bars, graph and staging arrangement are preserved; automated similarity was about 67% sample / 63% normal against the older narrow capture.
- Added six frontend behavior cases covering reset discovery at HEAD, hard confirmation/target, checkout/tag menu exposure, exact worktree start/path, selected-branch PR push and ancestry eligibility. Added three Rust integration cases covering reset semantics and guarded retry, annotated-tag object preservation, and local-only branch push/selected-start worktrees.
- Direct npm build, npm test and cargo test were refused: "MCP tool call requires approval, but approval policy is never". Unit/integration assertions are therefore not claimed as executed. Built-in rebuild_check compiles the application but does not replace those suites.
- Final rebuild_check after all source edits: build passed; sample startup/drawing at 929 × 669 passed; normal startup/drawing passed; mechanics passed with 141 values (123 sourced, 19 estimated; categories overlap). Opened both newly generated checks/round-0 images and compared them with the four reference pictures already reviewed and the original narrow capture. Dark palette, toolbar, graph lanes/ref labels and staging/commit arrangement remain consistent. Automated likeness was approximately 67% sample / 61% normal; different repository contents, restored panel sizes and Agents selection limit pixel matching. Static comparison does not establish animation or full functional equivalence.


## Additional conflict/diff audit — 2026-11-10

Opened all four reference pictures and reviewed the conflict editor, diff actions, editor integration and backend patch paths. Fixed automatic staging of manual/AI output with remaining conflict markers. Untouched conflict blocks now retain diff3 base sections and marker labels; custom-width and incomplete markers are parsed without inventing closing markers. Output no longer forces a final newline or turns a fully deleted conflict into a blank line. Unresolved-output saves refresh the repository and surface write failures.

Commit hunk reversion is hidden when whitespace filtering is enabled, since filtered hunk indexes can differ from the backend's full diff. Commit lines can now be selected for reversion. Failed hunk-diff reads show their errors instead of an empty-change message. Added six regression tests in ui/src/lib/conflict.test.ts. No layout constants changed. The follow-up also surfaces conflict-load failures instead of leaving the editor blank.

Validation pending: standalone npm --prefix ui test and cargo build were denied by the approval policy. The first integrated rebuild_check failed because Windows would not replace target/debug/gitcraft.exe (access denied); the running GitCraft window must close before retrying. No new screenshots have been accepted as fresh validation yet.

## Requested Deep pass — 2026-11-10

Reopened the launcher's executable and actual app-12.6.2 main program, then the shipped nodegit.node/nsfw.node modules. Searched names, settings and messages, followed references, and awaited twelve Ghidra function pseudocode results. The current section of MECHANICS.md records addresses, formulas, evidence IDs and where native equivalence remains unproven; mechanics.json remains synchronized.

Corrected patch export so personal format.subjectPrefix, format.numbered and format.coverLetter settings cannot replace the recovered PATCH prefix, single-patch unnumbered subject or multi-patch 1/N numbering. Export passes explicit Git CLI defaults and adds series numbering itself. Moved recovered patch and merge defaults to src/metrics.rs. Added assertions to the existing patch integration test for a custom prefix, forced numbering and enabled cover letters.

Final integrated rebuild_check passed build, sample startup/drawing at 1395×1004, normal startup/drawing and mechanics (103 values, 92 sourced, 12 estimated; counts overlap). Opened fresh checks/round-0/window.png: graph, file tree and commit pane populated; likeness approximately 67% sample / 69% normal. Direct cargo test extras::tests::patches_and_conflict_check -- --exact was refused: “MCP tool call requires approval, but approval policy is never”. The regression assertions were not executed; no live integrations were exercised.

## Resume verification — 2026-11-10

Reviewed the saved checklist and core_unknowns_show: the previous native-rule work is complete and both recorded unknowns are resolved. Reopened all four reference pictures. Fresh build screenshots exposed unreadable commit messages when fixed side panels squeezed the graph at the default window size. The side panels now shrink to 200 / 300 CSS px while reserving up to 440 px for the centre, within the available width. Wide windows retain saved panel sizes; divider drags use the actual displayed width. These are GitCraft adaptations, recorded as estimates in MECHANICS.md and mechanics.json. Corrected stale watcher documentation to match the previously implemented refresh rules.

Final integrated rebuild_check passed: build, sample startup/drawing at 1395 × 1004, normal startup, and mechanics (101 values, 90 sourced, 12 estimated; categories overlap). Opened both fresh checks/round-0 screenshots and visually compared their dark palette, top bars, graph, file tree and commit pane with all four references. Messages and controls remain readable. The automated likeness against the older narrow capture/window.png is approximately 67% sample / 69% normal; reserving readable graph space deliberately changes that narrow capture's panel proportions. No standalone tests, remote integration flows or live AI requests were run in this resume pass. Earlier test-execution restrictions and feature gaps below still apply.

## Scope and evidence

Reviewed the existing feature inventory and source for the build, repository caching and refreshes, working directory and index actions, diffs/history/blame, worktree watching, background jobs, terminal sessions, keyboard commands, graph rendering, settings, and hosting integrations. Opened all four pictures in reference/ and compared the newly generated checks/round-0/screenshot.png with them.

This is a source audit plus the built-in build/screenshot/mechanics checks. It is not an assertion that every workflow or connected account was exercised.

## Fixes made

| Area | Problem found | Change |
| --- | --- | --- |
| Build | A failed frontend compile could silently embed an older interface. | Fail the desktop build when npm or its frontend build fails; watch lockfile and frontend configuration changes too. |
| Repository views | A previous graph view could initialize a different query, including different visibility/limit settings. | Use the full query key's cache; retain placeholder data only for the same repository. |
| Diffs and history | Saves and HEAD changes refreshed the graph without necessarily refreshing open file contents, history, or blame. | Invalidate related views explicitly; unchanged file status no longer leaves old contents cached. |
| Watcher bursts | Several status reads could run together and finish out of order. | Coalesce requests per repository with a trailing read when another change arrives. |
| External changes | A one-second window after the app's own actions could suppress unrelated external changes. | Remove time-based suppression. |
| Linked worktrees | Watching only the worktree's gitdir missed branches/tags in its common directory. | Watch both metadata directories. |
| Large watcher batches | Writing all check-ignore input before reading its output could deadlock on full pipes; ignored-path lookup was quadratic. | Drain output while writing input; use a hash set for ignored paths. |
| Background fetch | A slow fetch could overlap the next timer tick. | Allow one batch per timer and stop scheduling further repositories after cleanup. |
| Git diagnostics | CRLF output and final messages without a newline could be omitted from errors. | Preserve complete diagnostics while keeping only final progress updates. |
| Repository tabs | Switching tabs unmounted the repository screen and its terminal/editor state. | Keep open repository screens mounted and show the active one. |
| Terminal visibility | Hiding the terminal killed running shells. | Keep sessions alive while hidden; closing a terminal still terminates that session. |
| Terminal startup | Output could arrive before listeners were ready or before the session id was known. | Register listeners before opening the shell and replay early output for that session. |
| Terminal cleanup | Naturally exited shells retained session resources. | Release PTYs and reap exited children; cancel queued startup commands on disposal. |
| Keyboard | Repository shortcuts could interrupt editing or shell shortcuts, or run behind a modal. | Respect input ownership and handled/composition events; isolate terminal shortcuts and block repository actions behind dialogs. |
| Focus shortcuts | A global callback could point to an inactive panel; the start-screen search hint had no working focus target. | Scope focus to the active repository or start screen. |
| Graph | Highlighting a long branch repeatedly scanned all commits. | Build a SHA-to-row map and use direct parent lookups. |
| Rendering | Busy/progress/toast updates rerendered the application and repository wrappers. | Use the panel store subscription that excludes those transient values. |
| Preferences | An interrupted write could truncate the settings file. | Write complete JSON to a temporary file, then rename it over the settings file. |
| macOS | Open in File Manager used the Linux launcher. | Use macOS's open command on macOS. |

## Validation

- Craft Anything rebuild_check: **build passed**.
- Sample app launch and fresh screenshot: **passed**, 1395 × 1004; automated likeness approximately **76%** against capture/window.png.
- Visually compared the fresh screenshot with all four reference pictures: dark panels, top bars, branch graph, file tree, and commit pane remain consistent with the reference arrangement. Different repository contents and window dimensions limit pixel comparisons.
- Mechanics check: **passed**, 94 recorded values; the audit did not change design constants.
- Added six frontend regression cases for refresh invalidation/coalescing and shortcut ownership, plus two Rust cases for CRLF/progress diagnostics.
- Standalone cargo test and cargo build commands, and an additional normal-window capture, were denied by the session's approval policy. The built-in rebuild check successfully built the edited sources, but the new unit tests were **not executed** in this session.
- The built-in check could not capture the app's normal restored window. checks/round-0/window.png predates this run and must not be treated as fresh evidence.

## Feature gaps and further verification

The feature inventory describes implementations, not independent proof that all integrations work. Earlier sections of FEATURES.md predate Stage 8; providers listed there as future work are now present in the source.

- Browser/desktop end-to-end automation is still missing. Important scenarios include keeping shells alive across tab changes/hiding, multi-repository selection, external edits while a diff is open, linked-worktree branch updates, conflicts, and cancellation.
- Real-account integration flows, self-hosted servers, token expiry, remote merge permissions, SSH/GPG prompts, LFS servers, and signed update installation still need verification against configured services. No live requests or remote mutations were made for this audit.
- CI/build-status badges on pull requests, dedicated graph filtering by author/team, terminal autocomplete, localization, and WSL-specific sign-in forwarding remain absent.
- Original-only account/cloud features (synced profiles/workspaces, hosted teams, Code Suggest, trials/licenses) remain intentional differences. GitCraft provides local workspaces/patches and conflict prevention rather than the original's cloud service.
- macOS/Linux behavior and very large repositories need platform and load testing; the graph lookup and watcher changes above are source-level efficiency improvements, not new timing benchmarks.

The next verification step is to run cargo test and the UI's npm test in an execution-enabled session, then automate the multi-tab/terminal and worktree scenarios above.

## Local and compatible AI follow-up — 2026-11-09

Every AI helper now awaits the current shared connection settings before invoking Rust, including PR drafts, conflict suggestions and the connection test. Local/custom servers can omit authentication. A default model serves all features, with optional per-feature overrides. Provider switches remember connection details and local presets cover Ollama, LM Studio and llama.cpp. API bases and complete endpoints are normalized, preserving proxy paths and query parameters. Compatible servers can use strict schema, JSON object or prompt-only JSON; automatic mode retries only explicit format incompatibility at the same endpoint. Reply validation checks required fields and nested types. Error text and generated-answer attribution no longer assume Anthropic. Saving credentials reports errors correctly and saved keys can be removed.

Added five Rust connection regression tests (including loopback fake-server tests) and nine UI connection regression cases covering all eight AI entry points, settings ordering and failed settings sync. Standalone test commands were rejected by the session execution policy, so these test cases have not been run. No real local model or hosted provider was contacted. Build/screenshot/mechanics verification uses Craft Anything's built-in check; all four references were opened and fresh sample and normal-window screenshots were inspected.
