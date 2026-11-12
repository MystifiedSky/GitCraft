# GitCraft

GitCraft is a desktop Git client for exploring repository history and managing everyday changes. Its workflow is inspired by GitKraken, with an independent implementation and its own branding. GitCraft is not affiliated with GitKraken.

<img width="3836" height="2086" alt="image" src="https://github.com/user-attachments/assets/8ee127cb-f041-4934-a80d-b8fa91852d09" />


[Features](#features) · [Under the hood](#under-the-hood) · [Get started](#get-started) · [Testing](#testing) · [Project structure](#project-structure) · [License](#license)

## Features

- **Explore history.** Follow branches and merges in an interactive commit graph, search by commit message, author, or hash, and inspect the files in a commit.
- **Review and commit changes.** View diffs as hunks, inline, or side by side; stage, unstage, or discard whole files, single hunks, or picked lines; edit a file in place; and create or amend a commit.
- **Dig into a file's past.** Open File History and Blame for any file, or compare two commits, or a commit and your working folder.
- **Shape the graph.** Show author, date, SHA, and change-count columns, show commit descriptions, hide or solo branches, and keep scrolling to load older history.
- **Manage branches, tags, and stashes.** Create or switch branches, merge or rebase, and use stashes to set work aside. GitCraft can undo and redo selected local operations.
- **Work across repositories.** Open, clone, or initialize a repository, keep several repositories open in tabs, and see changes reflected as files are edited.
- **Use your existing Git setup.** Fetch, pull, and push through the installed Git CLI and its configured credentials. The activity log shows the Git commands GitCraft runs.
- **Rewrite and resolve.** Interactive rebase (pick, reword, squash, drop, reorder), cherry pick, and a three-pane merge tool for conflicts.
- **Power tools.** Remotes, worktrees, submodules, Gitflow, Git LFS, sparse checkout, a command palette, a built-in terminal, workspaces, profiles, and four themes.
- **Pull requests and issues.** Connect GitHub, GitHub Enterprise, GitLab, Bitbucket, Azure DevOps, Jira or Trello (browser sign-in for GitHub, GitLab, Bitbucket and Azure DevOps, or a token) to list, view, create, comment on, and merge pull requests, work with issues, clone your repositories, and see everything on the Launchpad.
- **Preferences like you expect.** A full Preferences screen covers Git, SSH, signing, the editor, the terminal, external tools, AI providers, and per-repository settings.
- **AI helpers (optional).** Use a ChatGPT or Claude subscription through the installed Codex CLI or Claude Code, or connect an API provider or local server. The helpers write commit and stash messages, compose commits, explain changes, draft pull requests, and help resolve conflicts. One default model serves every helper, with optional per-feature overrides.
- **Manage many repositories.** Repository Management lists open, favorite, and recent repositories with their branch and uncommitted changes, and fetches or pulls them all at once.
- **Stays up to date.** Installed copies check GitHub Releases, download signed updates in the background, and install them on restart. See [RELEASING.md](RELEASING.md) to publish a version.
- **Share and stay ahead of conflicts.** Save or apply .patch files, see early whether your branch would conflict with the main branch, and list the Claude Code sessions that worked in a repository.

## Under the hood

Tauri 2 hosts the desktop window. Rust runs Git commands, watches repository files, and computes the graph layout. The interface uses React, TypeScript, Vite, and Tailwind CSS. A Canvas renderer draws the visible portion of the commit graph, and Monaco (bundled locally) shows diffs and file contents.

**Platform support:** Windows is the current tested target. macOS and Linux builds have not been validated.

## Get started

Download the [latest GitCraft installers](https://github.com/MystifiedSky/GitCraft/releases/latest). GitCraft needs Git available on `PATH`.

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

On Windows, the executable is `target/release/gitcraft.exe`. To package an installer (NSIS and MSI on Windows; deb, rpm, AppImage, or dmg on those systems):

```powershell
npm --prefix ui run installer
```

Installers are written to `target/release/bundle/`. Pushes to `main` build and publish signed updates through GitHub Actions; see [RELEASING.md](RELEASING.md).

## Testing

After installing the frontend dependencies, run these checks from the same directory:

```powershell
npm --prefix ui run typecheck
npm --prefix ui test
cargo test --locked
```

The Rust suite covers commit graph lanes, interactive rebase and conflicts, remotes, worktrees, Gitflow, the terminal, hosting API parsing, patches and the conflict check, file watcher behavior, partial (hunk and line) patches, and common Git operations in a temporary repository, including line staging, discard with undo, history, blame, comparisons, and hidden or soloed branches. For changes to the UI, launch the app and try the affected flow in a disposable repository.

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

## Local and compatible AI

Open **Preferences → AI**, choose a provider or local preset, and enter the exact model ID from that server as the **Default Model**. Start the local server and load that model first. Ollama defaults to `http://localhost:11434/v1`; LM Studio uses `http://localhost:1234/v1`; llama.cpp uses `http://localhost:8080/v1`. Addresses and models are editable. Server roots, API bases and full chat-completions URLs are accepted.

Local and custom connections can run without an API key. Save a key only when the server requires authentication; **Remove Key** deletes a previously saved key. Hosted Anthropic, OpenAI and Google connections require their provider key. Keys live in the operating system credential store. Provider endpoints, default models and feature overrides are remembered when switching providers.

**ChatGPT subscription:** Install the [Codex CLI](https://developers.openai.com/codex/cli), run `codex login` in a terminal, and choose **Sign in with ChatGPT**. In **Preferences → AI**, select **ChatGPT subscription (Codex CLI)**.

**Claude subscription:** Install [Claude Code](https://code.claude.com/docs/en/quickstart), run `claude auth login` in a terminal, and sign in with your Claude account. In **Preferences → AI**, select **Claude subscription (Claude Code)**. If you signed in with a separate `CLAUDE_CONFIG_DIR`, choose that folder in GitCraft.

Use **Test Connection** to verify the selected account. Leave **Program** empty to find the installed CLI automatically and **Default Model** empty to use its default. GitCraft runs the CLI outside the repository with tools disabled and passes the context for the helper you request. The CLI manages its own credentials and uses your account's allowance. API connections remain available separately.

**Test Connection** uses the default model. Every AI helper uses the same connection; feature model overrides are optional. **Automatic** response mode tries structured JSON, JSON mode, then prompt-only JSON if the selected server explicitly rejects that output format. You can select a mode directly for servers with limited compatibility. Replies are validated before use; authentication, rate-limit and missing-model errors are reported without format retries. Compatibility retries stay on the selected endpoint.

Model quality and context capacity depend on the model you run. Conflict suggestions remain editable and require review before saving; composed commits are reviewed before creation. External coding-agent CLIs in the Agents view keep their own provider configuration.
