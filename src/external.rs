//! Preferences > External Tools: open a repository or file in the chosen editor, and a terminal in the chosen
//! terminal app (custom executables take arguments with $REPO_PATH and $FILE).

use std::process::Command;

fn spawn(mut c: Command) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map(|_| ()).map_err(|e| format!("Could not start {:?}: {e}", c.get_program()))
}

/// Split an argument string, keeping "quoted parts" together, then fill in $REPO_PATH and $FILE.
pub fn split_args(args: &str, repo: &str, file: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for ch in args.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter().map(|a| a.replace("$REPO_PATH", repo).replace("$FILE", file)).collect()
}

/// The command-line launcher of each known editor.
pub fn editor_command(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "vscode" => "code",
        "cursor" => "cursor",
        "windsurf" => "windsurf",
        "zed" => "zed",
        "sublime" => "subl",
        "notepadpp" => "notepad++",
        "idea" => "idea64",
        "webstorm" => "webstorm64",
        "rider" => "rider64",
        "vs" => "devenv",
        _ => return None,
    })
}

/// Open a repository (or one file in it) in the external editor.
pub fn open_editor(repo: &str, file: Option<&str>, tool: &str, exe: &str, file_args: &str, repo_args: &str) -> Result<(), String> {
    let full_file = file.map(|f| std::path::Path::new(repo).join(f).to_string_lossy().into_owned());
    if tool == "custom" {
        if exe.trim().is_empty() {
            return Err("Choose the editor executable in Preferences > External Tools.".into());
        }
        let template = if full_file.is_some() { if file_args.trim().is_empty() { "\"$FILE\"" } else { file_args } } else if repo_args.trim().is_empty() { "\"$REPO_PATH\"" } else { repo_args };
        let mut c = Command::new(exe.trim());
        c.args(split_args(template, repo, full_file.as_deref().unwrap_or(""))).current_dir(repo);
        return spawn(c);
    }
    let cmd = editor_command(tool).unwrap_or("code");
    let target = full_file.unwrap_or_else(|| ".".into());
    // Through cmd on Windows so launcher scripts (code.cmd, cursor.cmd) are found.
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", cmd, &target]);
        c
    } else {
        let mut c = Command::new(cmd);
        c.arg(&target);
        c
    };
    c.current_dir(repo);
    spawn(c)
}

/// Open a terminal app in the repository folder.
pub fn open_terminal(repo: &str, tool: &str, exe: &str, args: &str) -> Result<(), String> {
    if tool == "custom" {
        if exe.trim().is_empty() {
            return Err("Choose the terminal executable in Preferences > External Tools.".into());
        }
        let mut c = Command::new(exe.trim());
        c.args(split_args(args, repo, "")).current_dir(repo);
        return spawn(c);
    }
    if !cfg!(windows) {
        let mut c = Command::new(if cfg!(target_os = "macos") { "open" } else { "x-terminal-emulator" });
        if cfg!(target_os = "macos") {
            c.args(["-a", "Terminal", repo]);
        }
        c.current_dir(repo);
        return spawn(c);
    }
    let has_wt = Command::new("where").arg("wt").output().map(|o| o.status.success()).unwrap_or(false);
    let mut c = Command::new("cmd");
    match tool {
        "wt" => c.args(["/C", "start", "", "wt", "-d", repo]),
        "default" if has_wt => c.args(["/C", "start", "", "wt", "-d", repo]),
        "powershell" => c.args(["/C", "start", "", "powershell", "-NoExit"]),
        "pwsh" => c.args(["/C", "start", "", "pwsh", "-NoExit"]),
        "gitbash" => {
            let git = Command::new("where").arg("git").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or_default().trim().to_string()).unwrap_or_default();
            let gb = std::path::Path::new(&git).parent().and_then(|p| p.parent()).map(|p| p.join("git-bash.exe")).filter(|p| p.exists());
            match gb {
                Some(p) => {
                    let p = p.to_string_lossy().into_owned();
                    c.args(["/C", "start", "", &p, &format!("--cd={repo}")])
                }
                None => return Err("Git Bash wasn't found.".into()),
            }
        }
        _ => c.args(["/C", "start", "", "cmd"]),
    };
    c.current_dir(repo);
    spawn(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_tool_arguments() {
        assert_eq!(split_args("--new-window \"$REPO_PATH\" -g", r"C:\My Repo", ""), vec!["--new-window", r"C:\My Repo", "-g"]);
        assert_eq!(split_args("\"$FILE\"", "r", r"C:\a b\x.txt"), vec![r"C:\a b\x.txt"]);
        assert_eq!(split_args("", "r", ""), Vec::<String>::new());
        assert_eq!(editor_command("cursor"), Some("cursor"));
    }
}
