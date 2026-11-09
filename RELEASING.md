# Releasing GitCraft and updating installed copies

Installed copies of GitCraft update themselves from this repository's **GitHub Releases**:

1. About 10 seconds after start-up, and every 4 hours after that, GitCraft downloads
   `https://github.com/MystifiedSky/GitCraft/releases/latest/download/latest.json`.
2. If that file names a newer version, GitCraft downloads the installer in the background and checks its
   **signature** against the public key built into the app. A file that wasn't signed with your private key
   is rejected, so nobody else can push an update to your users, even if they could change a release.
3. The status bar shows **Update Ready (Restart GitCraft)**. Clicking it (or Help → Restart and Install
   Update) installs the new version and reopens GitCraft. On Windows the installer runs in "passive" mode
   (a small progress window, no questions).

People can turn off background downloads in Preferences, or check by hand with Help → Check for Update.

## One-time setup

1. Install the front-end packages: `npm --prefix ui ci`
2. Run the setup script from the app folder:

   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\setup-updates.ps1
   ```

   It creates the signing key pair at `%USERPROFILE%\.tauri\gitcraft-updater.key` (outside the project, so it
   can never be committed), writes the public key into `tauri.conf.json`, and saves the private key and its
   password as the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets on GitHub
   (with the GitHub CLI, `gh`; otherwise it tells you what to paste into
   *Settings → Secrets and variables → Actions*).
3. Commit the `tauri.conf.json` change.
4. **Back up the key file and its password** (a password manager is ideal). If they are lost, copies that
   are already installed can't verify new updates and people would have to reinstall by hand.

Builds made before the public key is added don't check for updates (Help → Check for Update says so).

## Each release

```powershell
npm --prefix ui run version:set -- 0.2.0     # writes the version into tauri.conf.json, Cargo.toml and ui/package.json
cargo build                                  # updates Cargo.lock
git commit -am "Release 0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

Pushing the tag starts the **Release** workflow (`.github/workflows/release.yml`). It checks the tag matches
the version, builds Windows (NSIS + MSI), macOS (universal .dmg / .app) and Linux (.deb, .rpm, .AppImage),
signs the update files and uploads everything plus `latest.json` to a **draft** release.

Open the draft on GitHub, write the release notes (they appear in GitCraft's update notice), then press
**Publish release**. Installed copies pick it up on their next check. Keeping the release as a draft until
you publish lets you try the installers first; nobody is offered an update before then.

## Good to know

- Version numbers must go up (0.1.0 → 0.2.0 → 0.2.1). GitCraft never offers an older version.
- macOS and Linux builds come from the same workflow but haven't been tested by hand yet; if one of them
  fails, the Windows build is still uploaded.
- **Windows code signing** (a separate thing from update signing) removes the "Windows protected your PC"
  SmartScreen warning on first install. It needs a code-signing certificate from a certificate authority;
  once you have one, add it under `bundle.windows` in `tauri.conf.json`
  ([Tauri guide](https://v2.tauri.app/distribute/sign/windows/)). macOS needs an Apple Developer ID for the same.
