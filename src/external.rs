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
    out.into_iter().map(|argument| {
        let mut rest = argument.as_str(); let mut expanded = String::new();
        while let Some(position) = rest.find('$') {
            expanded.push_str(&rest[..position]); rest = &rest[position..];
            if let Some(tail) = rest.strip_prefix("$REPO_PATH") { expanded.push_str(repo); rest = tail; }
            else if let Some(tail) = rest.strip_prefix("$FILE") { expanded.push_str(file); rest = tail; }
            else { expanded.push('$'); rest = &rest[1..]; }
        }
        expanded.push_str(rest); expanded
    }).collect()
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

/// Resolve native programs and launcher scripts without asking a command shell to parse a file path.
fn program_path(name: &str) -> std::path::PathBuf {
    let extensions: &[&str] = if cfg!(windows) { &["exe", "com", "cmd", "bat"] } else { &[""] };
    if let Some(paths) = std::env::var_os("PATH") {
        for folder in std::env::split_paths(&paths) {
            for ext in extensions {
                let candidate = folder.join(if ext.is_empty() { name.to_owned() } else { format!("{name}.{ext}") });
                if candidate.is_file() { return candidate; }
            }
        }
    }
    name.into()
}

fn editor_process(program: &std::path::Path, target: &str, repo: &str) -> Command {
    let mut command = Command::new(program);
    command.arg(target).current_dir(repo);
    command
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
    // Rust handles .cmd/.bat launchers with its own argument escaping.
    spawn(editor_process(&program_path(cmd), &target, repo))
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

    #[cfg(windows)]
    #[test]
    fn batch_editor_receives_shell_characters_as_one_literal_file_argument() {
        let folder = tempfile::tempdir().unwrap();
        let launcher = folder.path().join("editor.cmd");
        std::fs::write(&launcher, "@echo off\r\necho %1\r\n").unwrap();
        let target = r"C:\repo\file&ver&.txt";
        let output = editor_process(&launcher, target, &folder.path().to_string_lossy()).output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(target), "{text}");
        assert!(!text.contains("Microsoft Windows"), "Filename executed as a shell command: {text}");
    }

    #[test]
    fn custom_tool_arguments() {
        assert_eq!(split_args("--new-window \"$REPO_PATH\" -g", r"C:\My Repo", ""), vec!["--new-window", r"C:\My Repo", "-g"]);
        assert_eq!(split_args("\"$FILE\"", "r", r"C:\a b\x.txt"), vec![r"C:\a b\x.txt"]);
        assert_eq!(split_args("", "r", ""), Vec::<String>::new());
        assert_eq!(editor_command("cursor"), Some("cursor"));
    }
}


/// Parse quoted argument fields without running a shell or expanding variables.
pub fn argument_fields(text: &str) -> Result<Vec<String>, String> {
    let mut output=Vec::new();let mut current=String::new();let mut quote=None;let mut started=false;let mut chars=text.chars().peekable();
    while let Some(ch)=chars.next() {
        match ch {
            '\\' if quote != Some('\'') && chars.peek().is_some_and(|next| *next=='"' || *next=='\\' || (quote.is_none() && (next.is_whitespace() || *next=='\''))) => {
                current.push(chars.next().unwrap());started=true;
            },
            '\''|'"' if quote.is_none() => {quote=Some(ch);started=true;},
            '\''|'"' if quote==Some(ch) => {quote=None;},
            ch if ch.is_whitespace() && quote.is_none() => {if started {output.push(std::mem::take(&mut current));started=false;}},
            _ => {current.push(ch);started=true;},
        }
    }
    if quote.is_some() {return Err("Custom flags contain an unclosed quote.".into());}
    if started {output.push(current);}Ok(output)
}


#[cfg(test)]
mod argument_field_tests {
    use super::*;
    #[test]
    fn quotes_windows_paths_and_values_without_a_shell() {
        assert_eq!(argument_fields(r#"--reference "C:\repos\reference repo.git" -c "user.name=Full Name" --no-tags"#).unwrap(),vec!["--reference",r"C:\repos\reference repo.git","-c","user.name=Full Name","--no-tags"]);
        assert_eq!(argument_fields(r#"--config='key=spaced value' --config "key=escaped \"quotes\"""#).unwrap(),vec!["--config=key=spaced value","--config","key=escaped \"quotes\""]);
        assert!(argument_fields("--reference \"unclosed").is_err());
    }
}


#[cfg(test)]
mod placeholder_tests {
    use super::*;
    #[test]
    fn inserted_paths_are_never_expanded_a_second_time() {
        let repo=r"C:\projects\$FILE workspace";let file=r"C:\files\$REPO_PATH document";
        assert_eq!(split_args("\"$REPO_PATH\" --file \"$FILE\"",repo,file),vec![repo,"--file",file]);
        assert_eq!(split_args("\"$REPO_PATH\"",repo,""),vec![repo]);
    }
}
