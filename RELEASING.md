# Releasing GitCraft

Each push to `main` starts the **Release** workflow. It builds the tip of that push and adds the workflow run number to the patch component of the base version in `tauri.conf.json`. For example, base `0.1.0` plus run 1 yields `0.1.1`. The workflow updates the version in its checkout only. It creates a version tag at the source commit when it publishes, so no version bump commits or manually pushed tags are needed.

The workflow builds Windows x64 (NSIS and MSI), macOS (universal DMG), and Linux x64 (AppImage, DEB, and RPM). It signs the updater bundles, checks that all expected assets are present, and publishes them together to [GitHub Releases](https://github.com/MystifiedSky/GitCraft/releases/latest). Until that succeeds, installed copies continue to see the previous release.

Use **Re-run failed jobs** in Actions to retry a failed release with the same version. Use **Run workflow** on `main` if you need a fresh build. To change the base version, set it with `npm --prefix ui run version:set -- <version>` and commit the changed manifests and lockfiles. Keep the next calculated version higher than the current release.

## Updating installed copies

GitCraft checks GitHub Releases shortly after startup and periodically after that. When it finds a newer version, it downloads the update and verifies the signature using the public key built into the app. Choose **Update Ready (Restart GitCraft)** to install it, or use **Help → Check for Update** to check manually. Background downloads can be turned off in Preferences.

Install a published release once if your current build predates the signing key. Linux AppImage builds can update in place; DEB and RPM installations should be upgraded using the matching package from GitHub.

Update signing is separate from Windows Authenticode signing and Apple notarization. First-time installations may still show platform security prompts.

## Signing key

The repository already has the public key in `tauri.conf.json` and the corresponding `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Actions secrets. Keep a backup of that same private key and its password in a secure location. Replacing the key would require users to install a new build manually before receiving more updates.

For a new repository or restoration, run `npm --prefix ui ci` followed by `powershell -ExecutionPolicy Bypass -File scripts/setup-updates.ps1` from the app checkout. Use the existing key and its password when restoring this repository.
