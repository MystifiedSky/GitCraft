//! Updates from GitHub Releases: read `latest.json` from the newest stable release, download the
//! installer for this platform, check its minisign signature against the public key built into
//! GitCraft, then install it and start the new version.
//!
//! The feed, the signatures and the installers keep the format earlier GitCraft releases read, so
//! those installed copies update to this one.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where installed copies look for the newest release.
pub const ENDPOINT: &str = "https://github.com/MystifiedSky/GitCraft/releases/latest/download/latest.json";

/// The minisign public key the release workflow signs with (base64 of the key file). Installed
/// copies only accept updates signed with its private key; see RELEASING.md before changing it.
pub const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEY1REM1OUYyQzlEQTg1NTkKUldSWmhkcko4bG5jOVcrNUlmaTdYdURnOHlFMkMxYTdGK2h3TnphbWpQMitXU3dDMWNtYjFPS00K";

const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// No installer is anywhere near this size.
const MAX_DOWNLOAD: u64 = 600 * 1024 * 1024;
/// The feed is a few kilobytes.
const MAX_FEED: usize = 1024 * 1024;

#[derive(Deserialize, Debug, Clone)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    pub platforms: HashMap<String, PlatformAsset>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlatformAsset {
    pub url: String,
    pub signature: String,
}

/// A newer release for this platform.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Update {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub signature: String,
}

/// `major.minor.patch`, with an optional leading `v`; anything after `-` or `+` is ignored.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

pub fn is_newer(current: &str, candidate: &str) -> bool {
    matches!((parse_version(current), parse_version(candidate)), (Some(a), Some(b)) if b > a)
}

/// How a Windows copy was installed, so an update comes as the same kind of installer and
/// replaces it instead of installing a second copy beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsInstaller {
    Nsis,
    Msi,
}

/// The NSIS installer leaves `uninstall.exe` beside GitCraft (cargo-packager's and the earlier
/// web-view release's alike); the MSI installs none.
pub fn windows_installer_in(dir: &Path) -> WindowsInstaller {
    if dir.join("uninstall.exe").is_file() { WindowsInstaller::Nsis } else { WindowsInstaller::Msi }
}

fn this_windows_installer() -> WindowsInstaller {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(windows_installer_in)).unwrap_or(WindowsInstaller::Nsis)
}

/// The feed keys for this build, most specific first.
pub fn platform_keys() -> Vec<&'static str> {
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    let installer = if cfg!(windows) { this_windows_installer() } else { WindowsInstaller::Nsis };
    keys_for(std::env::consts::OS, arch, installer)
}

fn keys_for(os: &str, arch: &str, installer: WindowsInstaller) -> Vec<&'static str> {
    match (os, arch) {
        // Only x64 ships an MSI; an ARM64 copy is always an NSIS install.
        ("windows", "aarch64") => vec!["windows-aarch64-nsis", "windows-aarch64"],
        ("windows", _) if installer == WindowsInstaller::Msi => vec!["windows-x86_64-msi"],
        ("windows", _) => vec!["windows-x86_64-nsis", "windows-x86_64"],
        ("macos", "aarch64") => vec!["darwin-aarch64"],
        ("macos", _) => vec!["darwin-x86_64"],
        ("linux", "aarch64") => vec!["linux-aarch64"],
        ("linux", _) => vec!["linux-x86_64"],
        _ => Vec::new(),
    }
}

/// The update `manifest` offers this build, if it is newer than `current`.
pub fn pick(manifest: &Manifest, current: &str, keys: &[&str]) -> Result<Option<Update>, String> {
    if parse_version(&manifest.version).is_none() {
        return Err("The update feed names an invalid version.".into());
    }
    if !is_newer(current, &manifest.version) {
        return Ok(None);
    }
    let asset = keys.iter().find_map(|k| manifest.platforms.get(*k)).ok_or("The newest release has no update for this platform.")?;
    let url = url::Url::parse(&asset.url).map_err(|_| "The update feed has an invalid download address.")?;
    if url.scheme() != "https" {
        return Err("The update feed has an insecure download address.".into());
    }
    Ok(Some(Update { version: manifest.version.clone(), notes: manifest.notes.clone(), url: asset.url.clone(), signature: asset.signature.clone() }))
}

fn decode_text(b64: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| "The update signature is malformed.")?;
    String::from_utf8(bytes).map_err(|_| "The update signature is malformed.".to_owned())
}

/// Check `data` against a release signature (base64 of a minisign signature file) and the
/// public key (base64 of a minisign public key file). A version in the signed comment has to be
/// the announced one.
pub fn verify(data: &[u8], signature: &str, public_key: &str, version: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(&decode_text(public_key)?).map_err(|_| "The update public key is invalid.")?;
    let sig = minisign_verify::Signature::decode(&decode_text(signature)?).map_err(|_| "The update signature is malformed.")?;
    key.verify(data, &sig, true).map_err(|_| "The update's signature doesn't match. It was not installed.")?;
    let signed = sig.trusted_comment().split('\t').find_map(|kv| kv.strip_prefix("version:"));
    match signed {
        Some(signed) if parse_version(signed) != parse_version(version) => Err(format!("The update is signed for version {signed}, not {version}.")),
        _ => Ok(()),
    }
}

fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    // Release downloads redirect to GitHub's file host; stay on HTTPS while following them.
    let policy = reqwest::redirect::Policy::custom(|attempt| if attempt.previous().len() > 8 || attempt.url().scheme() != "https" { attempt.stop() } else { attempt.follow() });
    reqwest::Client::builder().redirect(policy).timeout(timeout).user_agent(concat!("GitCraft/", env!("CARGO_PKG_VERSION"))).build().map_err(|e| e.to_string())
}

/// Whether this copy can install updates by itself, and why not.
pub fn can_install() -> Result<(), String> {
    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_none() {
        return Err("This copy was installed from a package. Install the newer DEB or RPM package from GitHub Releases.".into());
    }
    if !(cfg!(windows) || cfg!(target_os = "linux") || cfg!(target_os = "macos")) {
        return Err("Updates aren't available on this platform.".into());
    }
    Ok(())
}

/// Read the feed and return a newer update for this platform, if there is one.
pub fn check(current: &str) -> Result<Option<Update>, String> {
    crate::runtime().block_on(async {
        let mut response = client(CHECK_TIMEOUT)?.get(ENDPOINT).send().await.map_err(crate::hosting::transport_error)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("The update feed answered {}.", response.status()));
        }
        // Read piece by piece and stop at the limit, whether or not the size was announced.
        if response.content_length().is_some_and(|n| n > MAX_FEED as u64) {
            return Err("The update feed is too large.".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(crate::hosting::transport_error)? {
            if bytes.len() + chunk.len() > MAX_FEED {
                return Err("The update feed is too large.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| "The update feed is not valid.")?;
        pick(&manifest, current, &platform_keys())
    })
}

/// A new folder with an unpredictable name that only this user can open, for a downloaded update to wait in
/// until it is installed. It is kept: on Windows the installer runs after GitCraft has quit.
fn download_dir() -> Result<PathBuf, String> {
    tempfile::Builder::new().prefix("gitcraft-update-").tempdir().map(tempfile::TempDir::keep).map_err(|e| format!("Could not save the update: {e}"))
}

fn file_name(url: &str) -> String {
    let name = url.rsplit('/').next().unwrap_or("update");
    let name: String = url::form_urlencoded::parse(name.as_bytes()).map(|(k, v)| if v.is_empty() { k.into_owned() } else { format!("{k}={v}") }).collect();
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
    if safe.is_empty() || safe.starts_with('.') { "update".into() } else { safe }
}

/// Download `update`, report progress in percent, and check its signature. Returns the file.
pub fn download(update: &Update, progress: impl Fn(Option<u32>) + Send) -> Result<PathBuf, String> {
    let bytes = crate::runtime().block_on(async {
        let mut response = client(DOWNLOAD_TIMEOUT)?.get(&update.url).send().await.map_err(crate::hosting::transport_error)?;
        if !response.status().is_success() {
            return Err(format!("The download answered {}.", response.status()));
        }
        let total = response.content_length().filter(|n| *n > 0);
        if total.is_some_and(|n| n > MAX_DOWNLOAD) {
            return Err("The update is too large.".into());
        }
        let mut data = Vec::with_capacity(total.unwrap_or(0) as usize);
        progress(total.map(|_| 0));
        while let Some(chunk) = response.chunk().await.map_err(crate::hosting::transport_error)? {
            data.extend_from_slice(&chunk);
            if data.len() as u64 > MAX_DOWNLOAD {
                return Err("The update is too large.".into());
            }
            progress(total.map(|t| ((data.len() as u64 * 100) / t).min(100) as u32));
        }
        Ok::<_, String>(data)
    })?;
    verify(&bytes, &update.signature, PUBLIC_KEY, &update.version)?;
    let dir = download_dir()?;
    let path = dir.join(file_name(&update.url));
    std::fs::write(&path, &bytes).map_err(|e| format!("Could not save the update: {e}"))?;
    Ok(path)
}

/// Install a downloaded update. On Windows the installer is scheduled after this
/// process exits (`Ok(false)`); elsewhere the caller starts [`relaunch_target`] (`Ok(true)`).
pub fn install(file: &Path) -> Result<bool, String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let exe = std::env::current_exe().map_err(|e| format!("Could not find GitCraft: {e}"))?;
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle for this process.
        let creation_time = process_creation_time(unsafe { GetCurrentProcess() })
            .map_err(|e| format!("Could not identify this GitCraft process: {e}"))?;
        let script = windows_update_script(file, &exe, std::process::id(), creation_time);
        windows_update_command(&script)?.spawn().map_err(|e| format!("Could not schedule the installer: {e}"))?;
        Ok(false)
    }
    #[cfg(target_os = "linux")]
    {
        let target = PathBuf::from(std::env::var_os("APPIMAGE").ok_or("This copy is not an AppImage.")?);
        install_appimage(file, &target)?;
        Ok(true)
    }
    #[cfg(target_os = "macos")]
    {
        let bundle = app_bundle().ok_or("Could not find GitCraft's application bundle.")?;
        install_bundle(file, &bundle)?;
        Ok(true)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = file;
        Err("Updates aren't available on this platform.".into())
    }
}

/// A PowerShell quoted string.
#[cfg(any(windows, test))]
fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

#[cfg(windows)]
fn process_creation_time(handle: windows_sys::Win32::Foundation::HANDLE) -> std::io::Result<u64> {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetProcessTimes};
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut creation, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: The handle refers to a process and each output points to valid, distinct storage.
    if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(u64::from(creation.dwHighDateTime) << 32 | u64::from(creation.dwLowDateTime))
}

#[cfg(windows)]
fn windows_update_command(script: &str) -> Result<std::process::Command, String> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
    let windows = std::env::var_os("SystemRoot").ok_or("Could not locate Windows PowerShell.")?;
    let powershell = PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let mut command = std::process::Command::new(powershell);
    command.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

/// Pin the process with a handle before checking its creation time. An exited launcher
/// or reused PID must not stop an update or make it wait for an unrelated process.
#[cfg(any(windows, test))]
fn wait_for_windows_exit(pid: u32, creation_time: u64) -> String {
    format!(r#"
$app = Get-Process -Id {pid} -ErrorAction SilentlyContinue
if ($null -ne $app) {{
    $sameProcess = $false
    try {{
        $null = $app.Handle
        $sameProcess = $app.StartTime.ToFileTimeUtc() -eq {creation_time}
    }} catch {{
        if (-not $app.HasExited) {{ throw }}
    }}
    if ($sameProcess -and -not $app.WaitForExit(60000)) {{
        throw 'GitCraft did not finish closing. Close it, then reopen GitCraft and try the update again.'
    }}
    $app.Dispose()
}}
"#)
}

#[cfg(any(windows, test))]
fn windows_update_script(file: &Path, exe: &Path, pid: u32, creation_time: u64) -> String {
    let wait = wait_for_windows_exit(pid, creation_time);
    let install = if file.extension().is_some_and(|e| e.eq_ignore_ascii_case("msi")) {
        msi_script(file, exe)
    } else {
        format!(
            "$p = Start-Process -FilePath {} -ArgumentList '/P','/R' -PassThru; $p.WaitForExit(); if ($p.ExitCode -ne 0) {{ throw \"GitCraft's installer exited with code $($p.ExitCode).\" }}",
            ps_quote(&file.display().to_string())
        )
    };
    format!(r#"
$ErrorActionPreference = 'Stop'
try {{
    {wait}
    while (Get-Process -Name 'gitcraft' -ErrorAction SilentlyContinue) {{
        Add-Type -AssemblyName System.Windows.Forms
        $choice = [Windows.Forms.MessageBox]::Show('Close the other GitCraft windows, then choose Retry to continue updating.', 'GitCraft Update', 'RetryCancel', 'Information')
        if ($choice -ne 'Retry') {{ exit 1 }}
    }}
    {install}
}} catch {{
    Add-Type -AssemblyName System.Windows.Forms
    [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'GitCraft Update', 'OK', 'Error')
    exit 1
}}
"#)
}

/// MSI does not relaunch GitCraft. Start it after a successful install.
#[cfg(any(windows, test))]
fn msi_script(file: &Path, exe: &Path) -> String {
    let package = ps_quote(&format!("\"{}\"", file.display()));
    format!(
        r#"$p = Start-Process -FilePath "$env:SystemRoot\System32\msiexec.exe" -ArgumentList '/i',{package},'/passive','/norestart' -PassThru; $p.WaitForExit(); if ($p.ExitCode -ne 0 -and $p.ExitCode -ne 3010) {{ throw "Windows Installer exited with code $($p.ExitCode)." }}; Start-Process -FilePath {}"#,
        ps_quote(&exe.display().to_string())
    )
}

/// What to start after an update was installed.
pub fn relaunch_target() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        return std::env::var_os("APPIMAGE").map(PathBuf::from);
    }
    if cfg!(target_os = "macos") {
        return app_bundle();
    }
    std::env::current_exe().ok()
}

/// Start the installed version, detached from this process.
pub fn relaunch() -> Result<(), String> {
    let target = relaunch_target().ok_or("Could not find the installed GitCraft.")?;
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg("-n").arg(&target);
        c
    } else {
        std::process::Command::new(&target)
    };
    cmd.spawn().map(|_| ()).map_err(|e| format!("Could not start GitCraft again: {e}"))
}

/// `/Applications/GitCraft.app` for `/Applications/GitCraft.app/Contents/MacOS/gitcraft`.
fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?.to_path_buf();
    (bundle.extension().is_some_and(|e| e == "app")).then_some(bundle)
}

/// Replace the AppImage at `target` with `file`, keeping its permissions; the old one is put back
/// if anything fails.
#[cfg(unix)]
pub fn install_appimage(file: &Path, target: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let data = std::fs::read(file).map_err(|e| e.to_string())?;
    if !data.starts_with(b"\x7fELF") {
        return Err("The update is not an AppImage.".into());
    }
    let dir = target.parent().ok_or("The AppImage has no folder.")?;
    let mode = std::fs::metadata(target).map(|m| m.permissions().mode()).unwrap_or(0o755) | 0o111;
    let staged = dir.join(format!(".gitcraft-update-{}", std::process::id()));
    std::fs::write(&staged, &data).map_err(|e| format!("Could not write the update next to the AppImage: {e}"))?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())?;
    // One rename: the AppImage is never missing, and a running copy keeps its open file.
    std::fs::rename(&staged, target).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        format!("Could not replace the AppImage: {e}")
    })
}

/// Unpack a signed `.app.tar.gz` and put its bundle in place of `bundle`.
#[cfg(target_os = "macos")]
fn install_bundle(file: &Path, bundle: &Path) -> Result<(), String> {
    let parent = bundle.parent().ok_or("The application has no folder.")?;
    let stamp = std::process::id();
    let staging = parent.join(format!(".gitcraft-update-{stamp}"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("Could not unpack the update next to GitCraft: {e}"))?;
    let unpacked = (|| {
        let archive = std::fs::File::open(file).map_err(|e| e.to_string())?;
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
        tar.unpack(&staging).map_err(|e| format!("Could not unpack the update: {e}"))?;
        std::fs::read_dir(&staging).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "app")).ok_or_else(|| "The update holds no application.".to_owned())
    })();
    let new = match unpacked {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let backup = parent.join(format!(".gitcraft-previous-{stamp}"));
    std::fs::rename(bundle, &backup).map_err(|e| format!("Could not move the current GitCraft aside: {e}"))?;
    if let Err(e) = std::fs::rename(&new, bundle) {
        let _ = std::fs::rename(&backup, bundle);
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Could not install the update: {e}"));
    }
    let _ = std::fs::remove_dir_all(&backup);
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_copy_updates_with_the_installer_it_came_from() {
        let nsis = tempfile::tempdir().unwrap();
        std::fs::write(nsis.path().join("uninstall.exe"), b"").unwrap();
        let msi = tempfile::tempdir().unwrap();
        assert_eq!(windows_installer_in(nsis.path()), WindowsInstaller::Nsis);
        assert_eq!(windows_installer_in(msi.path()), WindowsInstaller::Msi);
        assert_eq!(keys_for("windows", "x86_64", WindowsInstaller::Msi), ["windows-x86_64-msi"]);
        assert_eq!(keys_for("windows", "x86_64", WindowsInstaller::Nsis), ["windows-x86_64-nsis", "windows-x86_64"]);
        // ARM64 ships no MSI.
        assert_eq!(keys_for("windows", "aarch64", WindowsInstaller::Msi), ["windows-aarch64-nsis", "windows-aarch64"]);
        assert_eq!(keys_for("linux", "x86_64", WindowsInstaller::Msi), ["linux-x86_64"]);
    }

    #[test]
    fn the_msi_script_quotes_its_paths() {
        let script = msi_script(Path::new("C:/Temp/it's/GitCraft.msi"), Path::new("C:/Program Files/GitCraft/gitcraft.exe"));
        assert!(script.contains("'\"C:/Temp/it''s/GitCraft.msi\"'"), "{script}");
        assert!(script.contains("Start-Process -FilePath 'C:/Program Files/GitCraft/gitcraft.exe'"), "{script}");
        assert!(script.contains("'/passive','/norestart'"));
    }

    #[test]
    fn both_windows_installers_start_after_the_process_wait() {
        let exe = Path::new("C:/Program Files/GitCraft/gitcraft.exe");
        for (file, args) in [("GitCraft.exe", "'/P','/R'"), ("GitCraft.msi", "'/passive','/norestart'")] {
            let script = windows_update_script(Path::new(file), exe, 1234, 5678);
            assert!(script.contains("Get-Process -Id 1234"));
            assert!(script.contains("ToFileTimeUtc() -eq 5678"));
            assert!(script.find("WaitForExit(60000)").unwrap() < script.find("Start-Process").unwrap());
            assert!(script.contains(args));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_update_handoff_waits_for_the_exact_process() {
        use std::{os::windows::io::AsRawHandle, process::Stdio, time::Instant};
        let mut app = windows_update_command("[void][Console]::ReadLine()").unwrap()
            .stdin(Stdio::piped()).spawn().unwrap();
        let stdin = app.stdin.take().unwrap();
        let creation = process_creation_time(app.as_raw_handle()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("ready");
        let installed = dir.path().join("installer started");
        let mark = |path: &Path| format!("Set-Content -LiteralPath {} -Value ready", ps_quote(&path.display().to_string()));
        let mut helper = windows_update_command(&format!("$ErrorActionPreference = 'Stop'\n{}\n{}\n{}",
            mark(&ready), wait_for_windows_exit(app.id(), creation), mark(&installed),
        )).unwrap().spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !ready.is_file() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(ready.is_file(), "the Windows helper did not reach the wait");

        // A stale creation time must not wait for this still-running process.
        let status = windows_update_command(&format!("$ErrorActionPreference = 'Stop'\n{}", wait_for_windows_exit(app.id(), creation + 1)))
            .unwrap().status().unwrap();
        assert!(status.success());
        assert!(app.try_wait().unwrap().is_none());
        assert!(!installed.exists(), "the installer ran while GitCraft was still open");

        drop(stdin);
        assert!(app.wait().unwrap().success());
        assert!(helper.wait().unwrap().success());
        assert!(installed.is_file(), "the installer did not run after GitCraft exited");
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.1.9", "0.1.10"));
        assert!(is_newer("0.1.0", "v0.2.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.1.0", "garbage"));
        assert_eq!(parse_version("1.2.3-beta"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
    }

    fn manifest(version: &str, url: &str) -> Manifest {
        serde_json::from_value(serde_json::json!({
            "version": version, "notes": "n",
            "platforms": {"linux-x86_64": {"url": url, "signature": "sig"}, "windows-x86_64-nsis": {"url": url, "signature": "nsis"}, "windows-x86_64": {"url": url, "signature": "plain"}}
        }))
        .unwrap()
    }

    #[test]
    fn the_feed_offers_only_newer_secure_downloads_for_this_platform() {
        let m = manifest("0.2.0", "https://example.invalid/a.AppImage");
        assert_eq!(pick(&m, "0.1.0", &["linux-x86_64"]).unwrap().unwrap().signature, "sig");
        assert_eq!(pick(&m, "0.1.0", &["windows-x86_64-nsis", "windows-x86_64"]).unwrap().unwrap().signature, "nsis");
        assert!(pick(&m, "0.2.0", &["linux-x86_64"]).unwrap().is_none());
        assert!(pick(&m, "0.1.0", &["darwin-aarch64"]).is_err());
        assert!(pick(&manifest("0.2.0", "http://example.invalid/a"), "0.1.0", &["linux-x86_64"]).is_err());
    }

    #[test]
    fn the_built_in_key_is_a_minisign_key() {
        assert!(minisign_verify::PublicKey::decode(&decode_text(PUBLIC_KEY).unwrap()).is_ok());
    }

    #[test]
    fn a_wrong_signature_is_refused() {
        // A real signature of other data under a different key.
        let other_key = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
        let sig = "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==\n";
        let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
        let key = b64(&format!("untrusted comment: minisign public key\n{other_key}\n"));
        assert!(verify(b"test", &b64(sig), &key, "1.0.0").is_ok());
        assert!(verify(b"tampered", &b64(sig), &key, "1.0.0").is_err());
        assert!(verify(b"test", &b64(sig), PUBLIC_KEY, "1.0.0").is_err());
        assert!(verify(b"test", "not base64!", PUBLIC_KEY, "1.0.0").is_err());
    }

    #[test]
    fn download_names_are_plain() {
        assert_eq!(file_name("https://x.invalid/v1/GitCraft_0.1.5_x64-setup.exe"), "GitCraft_0.1.5_x64-setup.exe");
        assert_eq!(file_name("https://x.invalid/a/..%2F..%2Fevil"), "update");
        assert_eq!(file_name("https://x.invalid/a/x%20y.AppImage"), "x_y.AppImage");
    }

    #[cfg(unix)]
    #[test]
    fn an_appimage_is_replaced_in_one_step() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("GitCraft.AppImage");
        std::fs::write(&target, b"\x7fELF old").unwrap();
        let new = dir.path().join("new");
        std::fs::write(&new, b"\x7fELF new").unwrap();
        install_appimage(&new, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"\x7fELF new");
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o111, 0);
        std::fs::write(&new, b"not an elf").unwrap();
        assert!(install_appimage(&new, &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"\x7fELF new");
    }
}
