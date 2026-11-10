//! Back end for the Preferences screen: settings that change how Git runs (executable, SSH key, logging),
//! global / repository Git config values, GPG key tools, and stored-credential clean-up.
//! Defaults follow the original's getDefaultSettings (main.bundle.js), see MECHANICS.md.

use crate::git;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// main.bundle.js maxCommitCountMinimum = 500 ("Initial Commits in Graph: Minimum 500").
pub const MIN_INITIAL_COMMITS: usize = 500;

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeSettings {
    /// Experimental > Git Executable: an explicit git.exe (empty = the one on PATH).
    pub git_path: String,
    /// SSH > Use local SSH agent (and the system's SSH config). When off, `ssh_private_key` is used.
    pub ssh_use_agent: bool,
    pub ssh_private_key: String,
    /// General > Use extended logging in activity log.
    pub extended_log: bool,
    /// Repo-Specific > Encoding: repository path -> encoding label (WHATWG names such as windows-1252).
    pub encodings: HashMap<String, String>,
}

fn same_path(a: &Path, b: &str) -> bool {
    let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(&a.to_string_lossy()) == n(b)
}

/// The encoding chosen for files in this repository (None = UTF-8).
pub fn encoding_for(dir: &Path) -> Option<&'static encoding_rs::Encoding> {
    let r = runtime();
    let label = r.encodings.iter().find(|(p, _)| same_path(dir, p)).map(|(_, e)| e.clone())?;
    encoding_rs::Encoding::for_label(label.trim().as_bytes()).filter(|e| *e != encoding_rs::UTF_8)
}

/// Text from file bytes in the repository's encoding (BOMs win, like editors do).
pub fn decode(bytes: &[u8], enc: &'static encoding_rs::Encoding) -> String {
    enc.decode(bytes).0.into_owned()
}

/// Bytes for saving text in the repository's encoding. UTF-16 is written by hand (encoding_rs only decodes it).
pub fn encode(text: &str, enc: &'static encoding_rs::Encoding, had_bom: bool) -> Vec<u8> {
    if enc == encoding_rs::UTF_16LE || enc == encoding_rs::UTF_16BE {
        let le = enc == encoding_rs::UTF_16LE;
        let mut out = Vec::new();
        let units = if had_bom { std::iter::once(0xFEFF_u16).chain(text.encode_utf16()).collect::<Vec<_>>() } else { text.encode_utf16().collect() };
        for u in units {
            out.extend(if le { u.to_le_bytes() } else { u.to_be_bytes() });
        }
        return out;
    }
    enc.encode(text).0.into_owned()
}

static RUNTIME: RwLock<Option<RuntimeSettings>> = RwLock::new(None);

pub fn set_runtime(s: RuntimeSettings) {
    if let Ok(mut r) = RUNTIME.write() {
        *r = Some(s);
    }
}

pub fn runtime() -> RuntimeSettings {
    RUNTIME.read().ok().and_then(|r| r.clone()).unwrap_or(RuntimeSettings { ssh_use_agent: true, ..Default::default() })
}

/// The git program to start.
pub fn git_program() -> String {
    let r = runtime();
    if r.git_path.trim().is_empty() {
        "git".into()
    } else {
        r.git_path.trim().to_string()
    }
}

/// GIT_SSH_COMMAND for a chosen private key (forward slashes: the value goes through Git's sh).
pub fn ssh_command(r: &RuntimeSettings) -> Option<String> {
    if r.ssh_use_agent || r.ssh_private_key.trim().is_empty() {
        return None;
    }
    let key = r.ssh_private_key.trim().replace('\\', "/");
    Some(format!("ssh -i \"{key}\" -o IdentitiesOnly=yes"))
}

#[derive(Serialize, Default)]
pub struct Versions {
    pub git: String,
    pub git_path: String,
    pub ssh: String,
    pub gpg: String,
}

fn first_line(program: &str, args: &[&str]) -> String {
    let mut c = std::process::Command::new(program);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    match c.output() {
        Ok(o) => {
            let s = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&s).lines().next().unwrap_or_default().trim().to_string()
        }
        Err(_) => String::new(),
    }
}

/// "Selected Git executable" and "Detected OpenSSH version" in Experimental.
pub fn versions(gpg_program: &str) -> Versions {
    let program = git_program();
    let path = if program == "git" { first_line(if cfg!(windows) { "where" } else { "which" }, &["git"]) } else { program.clone() };
    Versions {
        git: first_line(&program, &["--version"]),
        git_path: path,
        ssh: first_line("ssh", &["-V"]),
        gpg: first_line(if gpg_program.trim().is_empty() { "gpg" } else { gpg_program.trim() }, &["--version"]),
    }
}

/// Read several Git config values at once (scope "global" or "local").
pub fn config_get(dir: Option<&Path>, scope: &str, keys: &[String]) -> HashMap<String, String> {
    let d = dir.map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
    keys.iter()
        .map(|k| {
            let mut a = vec!["config"];
            if !scope.is_empty() {
                a.push(if scope == "local" { "--local" } else { "--global" });
            }
            a.extend(["--get", k.as_str()]);
            (k.clone(), git::run(&d, &a).stdout.trim().to_string())
        })
        .collect()
}

/// gpg --quick-gen-key: "a 4096 bit key with the name and email in your current profile that expires in 2 years"
/// (GPGPreferences-GpgGenerateKeyHelp). Returns the new key's fingerprint.
pub fn gpg_generate(program: &str, name: &str, email: &str, passphrase: &str) -> Result<String, String> {
    if name.trim().is_empty() || email.trim().is_empty() {
        return Err("Set your name and email (Profiles or Accounts) first.".into());
    }
    let program = if program.trim().is_empty() { "gpg" } else { program.trim() };
    let uid = format!("{} <{}>", name.trim(), email.trim());
    let mut c = std::process::Command::new(program);
    c.args(["--batch", "--pinentry-mode", "loopback", "--passphrase", passphrase, "--quick-gen-key", &uid, "rsa4096", "sign", "2y"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let o = c.output().map_err(|e| format!("Could not start {program}: {e}"))?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    let mut l = std::process::Command::new(program);
    l.args(["--list-secret-keys", "--with-colons", &uid]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        l.creation_flags(0x0800_0000);
    }
    let out = l.output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let fpr = text.lines().filter_map(|x| x.strip_prefix("fpr:::::::::")).last().map(|f| f.trim_end_matches(':').to_string());
    fpr.ok_or_else(|| "The key was created but could not be found afterwards.".into())
}

pub fn gpg_export(program: &str, key: &str) -> Result<String, String> {
    let program = if program.trim().is_empty() { "gpg" } else { program.trim() };
    let mut c = std::process::Command::new(program);
    c.args(["--armor", "--export", key]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let o = c.output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if s.is_empty() {
        Err(format!("No public key found for {key}"))
    } else {
        Ok(s)
    }
}

/// "Forget all Usernames and Passwords": ask Git's credential helper to drop what it saved for these hosts.
pub fn forget_credentials(hosts: &[String]) -> usize {
    use std::io::Write;
    let mut n = 0;
    for h in hosts {
        let mut c = git::git_cmd(&std::env::temp_dir());
        c.args(["credential", "reject"]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        if let Ok(mut ch) = c.spawn() {
            if let Some(mut si) = ch.stdin.take() {
                let _ = si.write_all(format!("protocol=https\nhost={h}\n\n").as_bytes());
            }
            if ch.wait().map(|s| s.success()).unwrap_or(false) {
                n += 1;
            }
        }
    }
    n
}

/// General > Delete ".orig" files after merging: the backup copies merge tools leave behind.
pub fn delete_orig_files(dir: &Path) -> Vec<String> {
    let o = git::run(dir, &["ls-files", "--others", "-z", "--", "*.orig"]);
    let mut gone = Vec::new();
    for f in o.stdout.split('\0').filter(|s| s.ends_with(".orig")) {
        let p: PathBuf = dir.join(f);
        if p.is_file() && std::fs::remove_file(&p).is_ok() {
            gone.push(f.to_string());
        }
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn repository_encodings_round_trip() {
        let w1252 = encoding_rs::Encoding::for_label(b"windows-1252").unwrap();
        assert_eq!(decode(&[0x63, 0x61, 0x66, 0xE9], w1252), "café");
        assert_eq!(encode("café", w1252, false), vec![0x63, 0x61, 0x66, 0xE9]);
        let le = encoding_rs::UTF_16LE;
        let bytes = encode("hi", le, true);
        assert_eq!(bytes, vec![0xFF, 0xFE, b'h', 0, b'i', 0]);
        assert_eq!(decode(&bytes, le), "hi");
        set_runtime(RuntimeSettings { encodings: [(r"C:\Repo".to_string(), "windows-1252".to_string())].into(), ssh_use_agent: true, ..Default::default() });
        assert_eq!(encoding_for(Path::new("c:/repo/")), Some(w1252));
        assert_eq!(encoding_for(Path::new("c:/other")), None);
        set_runtime(RuntimeSettings { ssh_use_agent: true, ..Default::default() });
    }

    #[test]
    fn chosen_ssh_key_becomes_ssh_command() {
        let mut r = RuntimeSettings { ssh_use_agent: false, ssh_private_key: r"C:\Users\Sam\.ssh\id_ed25519".into(), ..Default::default() };
        assert_eq!(ssh_command(&r).as_deref(), Some("ssh -i \"C:/Users/Sam/.ssh/id_ed25519\" -o IdentitiesOnly=yes"));
        r.ssh_use_agent = true;
        assert_eq!(ssh_command(&r), None);
    }

    #[test]
    fn orig_files_are_deleted_and_nothing_else() {
        let d = std::env::temp_dir().join(format!("gitcraft-orig-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("src")).unwrap();
        assert!(git::run(&d, &["init", "-q"]).ok);
        fs::write(d.join("a.txt"), "a").unwrap();
        fs::write(d.join("a.txt.orig"), "backup").unwrap();
        fs::write(d.join("src").join("b.rs.orig"), "backup").unwrap();
        let mut gone = delete_orig_files(&d);
        gone.sort();
        assert_eq!(gone, vec!["a.txt.orig".to_string(), "src/b.rs.orig".to_string()]);
        assert!(d.join("a.txt").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn reads_several_config_values() {
        let d = std::env::temp_dir().join(format!("gitcraft-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        assert!(git::run(&d, &["init", "-q"]).ok);
        assert!(git::run(&d, &["config", "core.hooksPath", ".githooks"]).ok);
        let m = config_get(Some(&d), "local", &["core.hooksPath".into(), "core.missing".into()]);
        assert_eq!(m["core.hooksPath"], ".githooks");
        assert_eq!(m["core.missing"], "");
        let _ = fs::remove_dir_all(&d);
    }
}
