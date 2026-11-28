# Testing GitCraft across desktop platforms

Use automated tests on each OS, then test the installed app in a real desktop session. The release workflow builds packages for all three platforms; the test workflow runs the suites on each OS. Neither replaces interactive validation.

## Ubuntu on a Windows development machine

For quick Linux development, use a separate Ubuntu 24.04 WSL 2 distribution with WSLg. From an administrator PowerShell terminal:

```powershell
wsl --install -d Ubuntu-24.04
```

Complete Ubuntu's first-run account setup. Follow Microsoft's [WSL GUI setup](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) if Linux windows do not open. Keep the checkout inside the Linux filesystem, such as `~/src/GitCraft`, rather than `/mnt/c`, for Linux filesystem behavior and file-watch performance. Install tools within Ubuntu; Windows installations of Rust do not replace Linux installations.

In Ubuntu:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl git \
  libgtk-3-dev libxkbcommon-dev libwayland-dev libdbus-1-dev
mkdir -p ~/src
cd ~/src
git clone https://github.com/MystifiedSky/GitCraft.git
cd GitCraft
```

Use Git 2.38 or later. Install stable Rust using the [official Rust installer](https://www.rust-lang.org/tools/install), and Node.js 22.12 or later if you work on the release scripts. Reopen the terminal, verify `cargo --version` and `git --version`, then run the commands below from the cloned repository root. GitCraft draws with the GPU through Vulkan, Metal, DirectX 12 or OpenGL; under WSLg it uses OpenGL.

WSLg is useful for everyday development. Also test packaged releases in a full Ubuntu Desktop VM or physical Linux desktop to check menus, file dialogs, notifications, credential storage, terminal behavior, and desktop installation. Use a disposable VM with a snapshot for package and updater testing. CI currently uses Ubuntu 22.04 as the Linux release build baseline; testing 24.04 additionally checks a newer desktop environment.

## macOS

Use a Mac, a remote Mac you can access interactively, or a macOS VM hosted on Apple hardware. GitHub's macOS runners provide automated build/test feedback; an interactive Mac session is still needed for desktop checks.

Install Apple's command-line developer tools:

```bash
xcode-select --install
```

Install stable Rust, clone the repository, and use the same build/test commands below. Verify Apple silicon and Intel separately before declaring both fully validated. The release pipeline compiles a universal app, while the test job runs on the runner's native architecture.

## Build and run the checks

From the directory containing `Cargo.toml` on each OS:

```bash
cargo test --locked --workspace -- --test-threads=1
cargo test --locked -p gitcraft headless_ -- --test-threads=1
node --test scripts/changelog-release.test.mjs
cargo run --locked
```

The `headless_` filter runs the native UI regressions used by the required GitHub check named
`Headless browser regressions`. On Linux that job covers the headless cases and the rest of the
Linux suite skips them. Windows and macOS CI use `cargo nextest run --locked --workspace --test-threads 4`
to give each test its own process, then run Rust documentation tests separately.

The app is in [crates/gitcraft](crates/gitcraft) and its back end in [crates/gitcraft-core](crates/gitcraft-core). Take a picture of the window without touching your preferences with `cargo run --locked -- --screenshot shot.png --theme light`; `--shot-view` shows a particular view first (see `--help`).

The Rust suite uses disposable repositories. Run it serially because some tests change process-wide runtime preferences. Live AI tests are ignored by default and require explicit local setup. Fixtures must not depend on your Git configuration: to check, run the suite with `GIT_CONFIG_GLOBAL` pointing at a file that sets `core.autocrlf = true`, as many Windows machines do.

To smoke-test GitHub Copilot, install the current Copilot CLI, select **GitHub Copilot subscription** under **Preferences → AI**, and use **Sign in with GitHub**. Fetch the model list, then run **Test Connection**. With the signed-in CLI on `PATH`, `cargo test --locked -p gitcraft-core live_copilot_connection -- --ignored` checks discovery, model listing, and a small structured request that uses your Copilot allowance.

Only one GitCraft window runs per user: a second launch hands its folder or `gitcraft://clone?url=` link to the running one and exits. To try that without touching your own preferences, build first (`cargo build --locked`), then start both launches of the built program with `HOME` (on Windows, `LOCALAPPDATA`) pointing at an empty folder, for example `HOME=/tmp/gc target/debug/gitcraft repo-a`, then `HOME=/tmp/gc target/debug/gitcraft repo-b`. Don't use `cargo run` for them: with `HOME` changed, rustup can't find the installed toolchain (or keep `RUSTUP_HOME` and `CARGO_HOME` at their original values for both launches). The second launch exits at once and `repo-b` opens as a tab in the first window. `--new-window` opens a separate window instead.

To package the app locally, install the packager once with `cargo install cargo-packager --locked --version 0.11.8`, then run `cargo build --release --locked -p gitcraft` and `cargo packager --release -p gitcraft --formats <formats>` (after a `--target` build, pass the same `--target` to the packager: 0.11.8 ignores `--binaries-dir`, and a relative `--out-dir` is read from `crates/gitcraft`): `nsis` or `wix` on Windows, `app` or `dmg` on macOS, `deb` or `appimage` on Linux. For an RPM, install `cargo install cargo-generate-rpm --locked --version 0.21.1` and run `cargo generate-rpm -p crates/gitcraft`. Published updater artifacts are built separately by the signed release pipeline; a local build alone does not verify the published update flow.

On Windows, run `./scripts/check-windows-icon.ps1 -Executable target/debug/gitcraft.exe` after a
debug build to verify that Windows can extract the executable icon used by shortcuts and Explorer.
The release workflow checks this for both Windows architectures.

On Windows, run `./scripts/check-msi-upgrade.ps1 -Directory <msi-output-directory>` to verify
that a newly packaged MSI recognizes the existing GitCraft MSI. In a disposable Windows VM,
install the previous public MSI, then run the new MSI with a higher version and verify that
GitCraft launches and Windows lists one installation.

### Benchmarks

`GITCRAFT_BENCH=<file>` turns on a probe that writes when the first populated graph is drawn, and `GITCRAFT_BENCH_SCROLL=<frames>` then scrolls the graph and writes frame times ([bench.rs](crates/gitcraft/src/app/bench.rs)). [scripts/benchmark.py](scripts/benchmark.py) runs a build against a repository made with [scripts/benchmark-repo.py](scripts/benchmark-repo.py), each launch with an empty `HOME`:

```sh
python3 scripts/benchmark-repo.py /tmp/gc-large 50000
cargo build --release --locked -p gitcraft
python3 scripts/benchmark.py native 7 /tmp/gc-large -- target/release/gitcraft
python3 scripts/benchmark.py scroll native 1500 /tmp/gc-large -- target/release/gitcraft
```

To compare with another build, give the runner that build's command; it needs the same probe lines. Compare builds on the same machine, with the same repository, after a warm-up launch.

## Choose the right checks

| Change | Start with | Evidence to include in the PR |
| --- | --- | --- |
| Git reads or writes | An existing Rust fixture near the operation or in [crates/gitcraft-core/src/regressions.rs](crates/gitcraft-core/src/regressions.rs) | The starting repo state, the action, and the resulting refs, staged changes, or working files that matter to the fix |
| Interface logic or layout | The nearest test in [crates/gitcraft](crates/gitcraft), and a `--screenshot` picture in the dark and light themes | The test and outcome, with before and after screenshots for visible changes |
| Native windows, file dialogs, launchers, credential storage, or updater behavior | A [desktop smoke test](#desktop-smoke-test) on the affected platform | The OS, architecture, package or build tested, and repeatable steps |
| Documentation or translations | Follow the steps or open the changed screen in the affected language | The command, screen, or links you checked |

For a bug fix, reproduce the original failure when practical and keep the fixture as small as possible. Prefer a disposable repository with synthetic files and a local remote. For example, the [stash regression](crates/gitcraft-core/src/regressions.rs) checks the live index and unselected work after a stash. If you cannot reproduce a platform-specific report, state the gap in the PR so a reviewer can verify it.

## Desktop smoke test

Use a disposable repository with a local bare remote. Record the app version, OS version, architecture, package format, and results.

- Edit a tracked file with CRLF line endings and repeated identifiers. Fold a function, find text inside it, edit repeated selections, and undo once. Switch among File, Inline, and Split views, then save and check that the file contains only the intended edits.
- In a split diff with insertions and deletions, scroll from each pane and drag or keyboard-navigate the overview ruler. Repeat with a file over 512 KiB, Word Wrap enabled, and the divider off center. In edit mode, use Ctrl+U (Command+U on macOS) after a selection change, then verify regular text undo still works.
- Turn on Word Wrap in Hunk View for a long changed line. Verify its continuation is visible and clicking it selects the same logical line for staging. For a patch beyond the hunk limit, confirm that the full comparison opens and file-level actions are available.
- In a read-only split diff, drag across several original lines and copy them. Use Ctrl+A and Ctrl+C in the modified pane and verify that each copy preserves its side's line endings and Unicode characters.
- Open a UTF-8 file in File, Inline, and Split views with a Cyrillic lookalike (U+0430 in a Latin identifier), a zero-width space (U+200B), and a bidirectional control (U+202E). Check the markers and hover labels. Verify that café, decomposed accents, and emoji such as 👩🏽‍💻 and 1️⃣ have no markers. Repeat with syntax colors off and a file over 512 KiB.
- Type a misspelling in the commit summary and description. Check the underlines, apply a suggestion, and undo it. Toggle spellcheck off in Preferences and verify that the fields continue to accept edits.

- Install, launch, close, and reopen the packaged app. Launch it again with a folder while it runs: the folder opens as a tab.
- Open a `gitcraft://clone?url=` link and "Open in GitCraft" from Explorer or the Finder.
- Open, clone, and initialize repositories, including paths with spaces and Unicode.
- On Windows, open a repository from a Git-reported path (with forward slashes). Use **Open in File Manager** from the File menu, toolbar Actions, and Repository Management, and verify Explorer opens the repository root. Use the same action from a nested changed file's context menu and verify Explorer selects the file.
- Stage a file, hunk, and line; commit; amend; verify the index and working tree.
- On Windows in a disposable repository, start an untracked executable and edit a tracked file. Choose **Discard All** and verify the error names the executable, the tracked edit remains, and no stash was created. Close the executable, retry, and use Undo to confirm both files are restored.
- Create and switch branches; merge, resolve a conflict, rebase, and exercise guarded undo/redo.
- Stash and restore work; fetch, pull, and push against the local remote.
- Verify graph scrolling, author images, diffs, file watching, and multiple repository tabs.
- Open the terminal, external editor, and native file dialogs. Verify preferences persist.
- Edit a file without saving, then open another file, close the tab, close the window, and restart for an update: each asks to Save, Discard, or Cancel, and Cancel keeps the text. Do the same with conflict picks.
- In the terminal, drag, double-click, and triple-click to select, copy with Ctrl+Shift+C (Cmd+C on macOS), and check that wide characters such as `漢字` and combined accents keep the caret and selection on their columns.
- Check credential storage, notifications, and optional AI integrations in a real signed-in desktop session.
- Install a previous published version, check for updates, restart into the new version, and verify the displayed version. On Windows x64 do this with the NSIS installer and with the MSI: each must update in place (one entry in Installed apps) and reopen with the earlier window size and position. Test Linux package upgrades separately from AppImage updates.

Report failures with reproduction steps and sanitized logs. Keep platform validation marked as in progress until the packaged desktop flows have been exercised.
