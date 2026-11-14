# Contributing to GitCraft

Thanks for helping improve GitCraft. Small, focused changes are easiest to review.

## Report a bug or suggest a feature

Open an [issue](https://github.com/MystifiedSky/GitCraft/issues) with the behavior you expected, what happened, your GitCraft version, operating system, and steps to reproduce it. Use a disposable repository when sharing a reproduction. Remove credentials, private repository URLs, and personal data from logs and screenshots.

Report security vulnerabilities privately using the process in [SECURITY.md](SECURITY.md).

## Build and check a change

Install Git, stable Rust, and Node.js 22.12 or later. On Windows, also install Visual Studio Build Tools with the **Desktop development with C++** workload, a Windows SDK, and the Microsoft Edge WebView2 Runtime. See [TESTING.md](TESTING.md) for Linux and macOS prerequisites. From the directory containing `Cargo.toml`:

```powershell
npm --prefix ui ci
npm --prefix ui run typecheck
npm --prefix ui test
cargo test --locked
node --test scripts/changelog-release.test.mjs
```

Run the app with `cargo run --locked` and exercise the affected behavior in a disposable repository. For Git operations, check the resulting branch, index, and working tree as well as the visible result.

See [TESTING.md](TESTING.md) for Ubuntu/WSL and macOS setup and the desktop smoke-test checklist. Pull requests run the test workflow on Windows, Ubuntu, and macOS.

## Submit a pull request

- Explain the problem, the resulting behavior, and the checks you ran.
- Include screenshots for visible changes, using sample data.
- Add a regression test when it helps verify a behavior change.
- Document notable changes under **Unreleased** in [CHANGELOG.md](CHANGELOG.md). Use the existing categories and leave published releases and comparison links alone.
- Keep dependency lockfiles in sync and explain dependency changes.

Contributions are distributed under the project's [GPL-3.0-only license](LICENSE). Retain notices for third-party code and assets.

## Build from source and package

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

