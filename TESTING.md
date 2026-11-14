# Testing GitCraft across desktop platforms

Use automated tests on each OS, then test the installed app in a real desktop session. The release workflow builds packages for all three platforms; the test workflow runs the suites on each OS. Neither replaces interactive validation.

## Ubuntu on a Windows development machine

For quick Linux development, use a separate Ubuntu 24.04 WSL 2 distribution with WSLg. From an administrator PowerShell terminal:

```powershell
wsl --install -d Ubuntu-24.04
```

Complete Ubuntu's first-run account setup. Follow Microsoft's [WSL GUI setup](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) if Linux windows do not open. Keep the checkout inside the Linux filesystem, such as `~/src/GitCraft`, rather than `/mnt/c`, for Linux filesystem behavior and file-watch performance. Install tools within Ubuntu; Windows installations of Rust and Node do not replace Linux installations.

In Ubuntu:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl git libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
mkdir -p ~/src
cd ~/src
git clone https://github.com/MystifiedSky/GitCraft.git
cd GitCraft
```

Install stable Rust using the [official Rust installer](https://www.rust-lang.org/tools/install) and Node.js 22.12 or later using the [Node.js installation instructions](https://nodejs.org/en/download). Reopen the terminal, verify `cargo --version`, `node --version`, and `git --version`, then run the commands below from the cloned repository root.

WSLg is useful for everyday development. Also test packaged releases in a full Ubuntu Desktop VM or physical Linux desktop to check menus, file dialogs, notifications, credential storage, terminal behavior, and desktop installation. Use a disposable VM with a snapshot for package and updater testing. CI currently uses Ubuntu 22.04 as the Linux release build baseline; testing 24.04 additionally checks a newer desktop environment.

## macOS

Use a Mac, a remote Mac you can access interactively, or a macOS VM hosted on Apple hardware. GitHub's macOS runners provide automated build/test feedback; an interactive Mac session is still needed for desktop checks.

Install Apple's command-line developer tools:

```bash
xcode-select --install
```

Install stable Rust and Node.js 22.12 or later, clone the repository, and use the same build/test commands below. See [Tauri's platform prerequisites](https://v2.tauri.app/start/prerequisites/) for current setup requirements. Verify Apple silicon and Intel separately before declaring both fully validated. The release pipeline compiles a universal app, while the test job runs on the runner's native architecture.

## Build and run the checks

From the directory containing `Cargo.toml` on each OS:

```bash
npm --prefix ui ci
npm --prefix ui run build
npm --prefix ui test
node --test scripts/changelog-release.test.mjs
cargo test --locked -- --test-threads=1
cargo run --locked
```

The Rust suite uses disposable repositories. Run it serially because some tests change process-wide runtime preferences. Live AI tests are ignored by default and require explicit local setup. Linux runs the GLib security regression as part of the full native suite.

To package the app locally, run `npm --prefix ui run installer`. Published updater artifacts are built separately by the signed release pipeline; a local build alone does not verify the published update flow.

## Desktop smoke test

Use a disposable repository with a local bare remote. Record the app version, OS version, architecture, package format, and results.

- Install, launch, close, and reopen the packaged app.
- Open, clone, and initialize repositories, including paths with spaces and Unicode.
- Stage a file, hunk, and line; commit; amend; verify the index and working tree.
- Create and switch branches; merge, resolve a conflict, rebase, and exercise guarded undo/redo.
- Stash and restore work; fetch, pull, and push against the local remote.
- Verify graph scrolling, author images, diffs, file watching, and multiple repository tabs.
- Open the terminal, external editor, and native file dialogs. Verify preferences persist.
- Check credential storage, notifications, and optional AI integrations in a real signed-in desktop session.
- Install a previous published version, check for updates, restart into the new version, and verify the displayed version. Test Linux package upgrades separately from AppImage updates.

Report failures with reproduction steps and sanitized logs. Keep platform validation marked as in progress until the packaged desktop flows have been exercised.
