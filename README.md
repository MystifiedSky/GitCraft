# GitCraft

GitCraft is a desktop Git client for exploring repository history and managing everyday changes. Its workflow is inspired by GitKraken, with an independent implementation and its own branding. GitCraft is not affiliated with GitKraken.

[Features](#features) · [Under the hood](#under-the-hood) · [Get started](#get-started) · [Testing](#testing) · [Project structure](#project-structure) · [License](#license)

## Features

- **Explore history.** Follow branches and merges in an interactive commit graph, search by commit message, author, or hash, and inspect the files in a commit.
- **Review and commit changes.** View diffs as hunks, inline, or side by side; stage, unstage, or discard whole files, single hunks, or picked lines; edit a file in place; and create or amend a commit.
- **Dig into a file's past.** Open File History and Blame for any file, or compare two commits, or a commit and your working folder.
- **Shape the graph.** Show author, date, SHA, and change-count columns, show commit descriptions, hide or solo branches, and keep scrolling to load older history.
- **Manage branches, tags, and stashes.** Create or switch branches, merge or rebase, and use stashes to set work aside. GitCraft can undo and redo selected local operations.
- **Work across repositories.** Open, clone, or initialize a repository, keep several repositories open in tabs, and see changes reflected as files are edited.
- **Use your existing Git setup.** Fetch, pull, and push through the installed Git CLI and its configured credentials. The activity log shows the Git commands GitCraft runs.

## Under the hood

Tauri 2 hosts the desktop window. Rust runs Git commands, watches repository files, and computes the graph layout. The interface uses React, TypeScript, Vite, and Tailwind CSS. A Canvas renderer draws the visible portion of the commit graph, and Monaco (bundled locally) shows diffs and file contents.

**Platform support:** Windows is the current tested target. macOS and Linux builds have not been validated.

## Get started

On Windows, have these tools available before building:

- Git for Windows on `PATH`. GitCraft also needs `git` at runtime.
- A recent stable Rust toolchain with Cargo and the Windows MSVC target.
- Visual Studio Build Tools with the **Desktop development with C++** workload and a Windows SDK, plus the Microsoft Edge WebView2 Runtime.
- Node.js 22.12 or later and npm.

From the directory containing `Cargo.toml`, install the frontend dependencies and launch the desktop app:

```powershell
npm --prefix ui ci
cargo run --locked
```

Use **Open a repository** for an existing checkout, or open the sample repository to explore the graph and file panel. The Rust build runs the frontend build after the npm dependencies are available. Restart `cargo run` to pick up UI changes.

To compile a release executable:

```powershell
npm --prefix ui run build
cargo build --release --locked
```

On Windows, the executable is `target/release/gitcraft.exe`. These commands build an executable; they do not package an installer.

## Testing

After installing the frontend dependencies, run these checks from the same directory:

```powershell
npm --prefix ui run typecheck
cargo test --locked
```

The Rust suite covers commit graph lanes, file watcher behavior, partial (hunk and line) patches, and common Git operations in a temporary repository, including line staging, discard with undo, history, blame, comparisons, and hidden or soloed branches. For changes to the UI, launch the app and try the affected flow in a disposable repository.

## Project structure

| Path | Purpose |
| --- | --- |
| `src/` | Rust backend, Git operations, graph layout, and file watching |
| `ui/src/` | React components, styles, and application state |
| `capabilities/` | Tauri permissions for the desktop window |
| `icons/` | GitCraft application icons |
| `build.rs`, `tauri.conf.json` | Frontend build integration and Tauri configuration |
| `Cargo.lock`, `ui/package-lock.json` | Locked Rust and frontend dependencies |

## Contributing

Describe the behavior you want to change and how to reproduce it. For a patch, include the checks you ran and, for visible changes, a screenshot using a sample repository.

## License

Copyright © 2026 the GitCraft contributors.

GitCraft is licensed under the GNU General Public License, version 3 only (`GPL-3.0-only`). See [LICENSE](LICENSE) for the full terms. Third-party dependencies and bundled fonts retain their respective licenses.
