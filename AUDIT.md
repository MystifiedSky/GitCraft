# GitCraft audit — 2026-11-09

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
