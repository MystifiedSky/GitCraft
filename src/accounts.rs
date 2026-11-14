//! Stage 4: identity, credentials, SSH keys and GPG signing — all stored in Git's own config and
//! the user's ~/.ssh folder, so other Git tools see the same settings.

use crate::git;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Serialize, Default)]
pub struct SshKey {
    pub path: String,
    pub kind: String,
    pub public: String,
}

#[derive(Serialize, Default)]
pub struct GpgKey {
    pub id: String,
    pub fingerprint: String,
    pub uid: String,
}

#[derive(Serialize, Default)]
pub struct AccountInfo {
    /// Global identity (and this repository's override, if any).
    pub name: String,
    pub email: String,
    pub repo_name: String,
    pub repo_email: String,
    pub credential_helper: String,
    /// Git Credential Manager is installed.
    pub gcm: bool,
    pub ssh_dir: String,
    pub ssh_keys: Vec<SshKey>,
    pub ssh_keygen: bool,
    pub gpg: bool,
    pub gpg_keys: Vec<GpgKey>,
    pub signing_key: String,
    pub sign_commits: bool,
    pub signing_format: String,
}

fn cmd(program: &str) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    c.creation_flags(0x0800_0000);
    c
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default()
}



pub fn info(dir: &Path) -> AccountInfo {
    let ssh_dir = home().join(".ssh");
    let mut ssh_keys = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&ssh_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "pub").unwrap_or(false) {
                let public = std::fs::read_to_string(&p).unwrap_or_default().trim().to_string();
                let kind = public.split_whitespace().next().unwrap_or("").to_string();
                ssh_keys.push(SshKey { path: p.with_extension("").to_string_lossy().into_owned(), kind, public });
            }
        }
    }
    let keys: Vec<String> = ["user.name", "user.email", "credential.helper", "user.signingkey", "commit.gpgsign", "gpg.format", "gpg.program"].map(str::to_owned).to_vec();
    let global = crate::prefs::config_get(Some(dir), "global", &keys);
    let local = crate::prefs::config_get(Some(dir), "local", &keys);
    let effective = crate::prefs::config_get(Some(dir), "", &keys);
    let config = |scope: &str, key: &str| {
        let values = match scope { "--global" => &global, "--local" => &local, _ => &effective };
        values.get(key).cloned().unwrap_or_default()
    };
    let gpg_program = config("", "gpg.program");
    let gpg_out = cmd(if gpg_program.is_empty() { "gpg" } else { &gpg_program }).args(["--list-secret-keys", "--keyid-format", "LONG", "--with-colons"]).output();
    let gpg = gpg_out.as_ref().is_ok_and(|out| out.status.success());
    let gpg_keys = gpg_out.map(|out| parse_gpg_keys(&String::from_utf8_lossy(&out.stdout))).unwrap_or_default();
    AccountInfo {
        name: config("--global", "user.name"),
        email: config("--global", "user.email"),
        repo_name: config("--local", "user.name"),
        repo_email: config("--local", "user.email"),
        credential_helper: config("", "credential.helper"),
        gcm: git::run(dir, &["credential-manager", "--version"]).ok,
        ssh_dir: ssh_dir.to_string_lossy().into_owned(),
        ssh_keys,
        ssh_keygen: cmd("ssh-keygen").arg("-?").output().is_ok(),
        gpg,
        gpg_keys,
        signing_key: config("", "user.signingkey"),
        sign_commits: config("", "commit.gpgsign") == "true",
        signing_format: {
            let f = config("", "gpg.format");
            if f.is_empty() { "openpgp".into() } else { f }
        },
    }
}

/// Set (or with an empty value, remove) a Git setting. scope: "global" or "local".
pub fn set(dir: &Path, scope: &str, key: &str, value: &str) -> Result<(), String> {
    let s = format!("--{scope}");
    if value.is_empty() {
        // Unsetting a key that isn't there is fine (git exits with 5 and says nothing).
        let o = git::run(dir, &["config", &s, "--unset-all", key]);
        return if o.ok || o.message().is_empty() { Ok(()) } else { Err(o.message()) };
    }
    let o = git::run(dir, &["config", &s, key, value]);
    if o.ok {
        Ok(())
    } else {
        Err(o.message())
    }
}

/// Create a new ed25519 SSH key (no passphrase) and return its public half.
pub fn generate_ssh_key(email: &str, name: &str) -> Result<String, String> {
    let dir = home().join(".ssh");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(if name.trim().is_empty() { "id_ed25519" } else { name.trim() });
    if file.exists() {
        return Err(format!("{} already exists", file.display()));
    }
    let o = cmd("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-C", email, "-f"])
        .arg(&file)
        .output()
        .map_err(|e| format!("ssh-keygen is not available: {e}"))?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    std::fs::read_to_string(file.with_extension("pub")).map(|s| s.trim().to_string()).map_err(|e| e.to_string())
}

pub fn read_public_key(path: &str) -> Result<String, String> {
    let path = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")).map(|rest| home().join(rest)).unwrap_or_else(|| path.into());
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() < 2 || !(fields[0].starts_with("ssh-") || fields[0].starts_with("ecdsa-") || fields[0].starts_with("sk-")) {
        return Err("Choose an SSH public key file (.pub).".into());
    }
    Ok(text.trim().into())
}

pub fn parse_gpg_keys(text: &str) -> Vec<GpgKey> {
    let mut keys = Vec::new(); let mut id = String::new(); let mut fingerprint = String::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split(':').collect();
        match fields.first().copied() {
            Some("sec") if fields.len() > 4 => { id = fields[4].into(); fingerprint.clear(); },
            Some("fpr") if fields.len() > 9 && !id.is_empty() => fingerprint = fields[9].into(),
            Some("uid") if fields.len() > 9 && !id.is_empty() => { keys.push(GpgKey { id: std::mem::take(&mut id), fingerprint: std::mem::take(&mut fingerprint), uid: fields[9].into() }); },
            _ => {},
        }
    }
    keys
}
