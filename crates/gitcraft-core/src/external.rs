//! Preferences > External Tools: open a repository or file in the chosen editor, and a terminal in the chosen
//! terminal app (custom executables take arguments with $REPO_PATH and $FILE).

use std::{path::{Path, PathBuf}, process::Command};

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
        "notepadpp" if cfg!(windows) => "notepad++",
        "idea" => if cfg!(windows) { "idea64" } else { "idea" },
        "webstorm" => if cfg!(windows) { "webstorm64" } else { "webstorm" },
        "rider" => if cfg!(windows) { "rider64" } else { "rider" },
        "vs" if cfg!(windows) => "devenv",
        _ => return None,
    })
}

/// Resolve native programs and launcher scripts without asking a command shell to parse a file path.
pub(crate) fn find_program(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() { return path.is_file().then(|| path.to_path_buf()); }
    if path.components().count() != 1 { return None; }
    let extensions: &[&str] = if cfg!(windows) && path.extension().is_none() { &["exe", "com", "cmd", "bat"] } else { &[""] };
    let folders = std::env::var_os("PATH")?;
    for folder in std::env::split_paths(&folders).filter(|path| path.is_absolute()) {
        for ext in extensions {
            let candidate = folder.join(if ext.is_empty() { name.to_owned() } else { format!("{name}.{ext}") });
            if candidate.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if !candidate.metadata().is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0) { continue; }
                }
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
pub(crate) fn git_for_windows_program(relative: &str) -> Option<PathBuf> {
    let git = find_program(&crate::prefs::git_program())?;
    git.parent()?.ancestors().take(4).map(|root| root.join(relative)).find(|path| path.is_file())
}

/// Add missing package manager directories to PATH during single-threaded app startup.
#[cfg(target_os = "macos")]
pub fn add_macos_tool_paths() {
    // Finder uses a system-only PATH. Preserve the user's existing tool order when
    // launched from a shell and place missing package managers before the OS shims.
    let existing = std::env::var_os("PATH").filter(|path| !path.is_empty()).unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into());
    let mut folders: Vec<_> = std::env::split_paths(&existing).filter(|path| path.is_absolute()).collect();
    let package_managers = if cfg!(target_arch = "aarch64") { ["/opt/homebrew/bin", "/usr/local/bin"] } else { ["/usr/local/bin", "/opt/homebrew/bin"] };
    let missing = package_managers.into_iter().map(PathBuf::from).filter(|path| path.is_dir() && !folders.contains(path)).collect::<Vec<_>>();
    let system = folders.iter().position(|path| path == Path::new("/usr/bin")).unwrap_or(folders.len());
    folders.splice(system..system, missing);
    if let Ok(path) = std::env::join_paths(folders) { std::env::set_var("PATH", path); }
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
    let cmd = editor_command(tool).ok_or_else(|| "Choose a supported editor in Preferences > External Tools.".to_owned())?;
    let target = full_file.unwrap_or_else(|| repo.into());
    if let Some(program) = find_program(cmd) { return spawn(editor_process(&program, &target, repo)); }
    #[cfg(target_os = "macos")]
    {
        let app = match tool {
            "vscode" => "Visual Studio Code", "cursor" => "Cursor", "windsurf" => "Windsurf",
            "zed" => "Zed", "sublime" => "Sublime Text", "idea" => "IntelliJ IDEA",
            "webstorm" => "WebStorm", "rider" => "Rider", _ => return Err(format!("Editor launcher {cmd} wasn't found.")),
        };
        // Wait for Launch Services to confirm that the requested app was found.
        return open_mac_app(app, &target);
    }
    #[cfg(not(target_os = "macos"))]
    Err(format!("Editor launcher {cmd} wasn't found. Choose its executable in Preferences > External Tools."))
}

#[cfg(target_os = "macos")]
fn open_mac_app(app: &str, path: &str) -> Result<(), String> {
    let result = Command::new("/usr/bin/open").args(["-a", app, path]).output().map_err(|error| error.to_string())?;
    if result.status.success() { Ok(()) } else { Err(format!("Could not open {app}: {}", String::from_utf8_lossy(&result.stderr).trim())) }
}

fn spawn_terminal(mut command: Command) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE
    }
    command.spawn().map(|_| ()).map_err(|error| format!("Could not start {:?}: {error}", command.get_program()))
}

/// Open a terminal app in the repository folder.
pub fn open_terminal(repo: &str, tool: &str, exe: &str, args: &str) -> Result<(), String> {
    if tool == "custom" {
        if exe.trim().is_empty() {
            return Err("Choose the terminal executable in Preferences > External Tools.".into());
        }
        let mut command = Command::new(exe.trim());
        command.args(split_args(args, repo, "")).current_dir(repo);
        return spawn_terminal(command);
    }
    #[cfg(windows)]
    { return spawn_terminal(windows_terminal(repo, tool)?); }
    #[cfg(target_os = "macos")]
    {
        return match tool {
            "default" => open_mac_app("Terminal", repo),
            "iterm" => open_mac_app("iTerm", repo),
            "warp" => open_mac_app("Warp", repo),
            "ghostty" => {
                let result = Command::new("/usr/bin/open").args(["-n", "-a", "Ghostty", "--args", &format!("--working-directory={repo}")]).output().map_err(|error| error.to_string())?;
                if result.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&result.stderr).trim().into()) }
            }
            _ => Err("Choose a supported terminal in Preferences > External Tools.".into()),
        };
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if tool != "default" { return Err("Choose a supported terminal in Preferences > External Tools.".into()); }
        let configured = std::env::var("TERMINAL").ok();
        let program = configured.as_deref().and_then(find_program).or_else(|| {
            ["xdg-terminal-exec", "x-terminal-emulator", "gnome-terminal", "konsole", "xfce4-terminal", "kitty", "alacritty", "wezterm", "xterm"].into_iter().find_map(find_program)
        }).ok_or("No terminal launcher found. Choose one in Preferences > External Tools.")?;
        let mut command = Command::new(&program);
        match program.file_name().and_then(|name| name.to_str()) {
            Some("gnome-terminal" | "konsole" | "xfce4-terminal" | "alacritty") => { command.arg(format!("--working-directory={repo}")); }
            Some("kitty") => { command.args(["--directory", repo]); }
            Some("wezterm") => { command.args(["start", "--cwd", repo]); }
            _ => {}
        }
        command.current_dir(repo);
        spawn_terminal(command)
    }
}

/// Escape a value for Windows Terminal's own `;` subcommand separator.
#[cfg_attr(not(windows), allow(dead_code))]
fn wt_literal(value: &str) -> String {
    value.replace(';', "\\;")
}

#[cfg(windows)]
fn windows_terminal(repo: &str, tool: &str) -> Result<Command, String> {
    let mut command = match tool {
        "default" | "wt" if tool == "wt" || find_program("wt.exe").is_some() => {
            let mut command = Command::new("wt.exe");
            // Windows Terminal splits its own command line at `;` (new tab or pane), even inside
            // a quoted argument. `\;` is its escape for a literal semicolon in a folder name.
            command.args(["-d", &wt_literal(repo)]);
            command
        }
        "powershell" | "pwsh" => {
            let mut command = Command::new(if tool == "pwsh" { "pwsh.exe" } else { "powershell.exe" });
            command.arg("-NoExit");
            command
        }
        "gitbash" => {
            let program = git_for_windows_program("git-bash.exe").ok_or("Git Bash wasn't found next to the configured Git executable.")?;
            let mut command = Command::new(program);
            command.arg(format!("--cd={repo}"));
            command
        }
        "default" | "cmd" => Command::new("cmd.exe"),
        _ => return Err("Choose a supported terminal in Preferences > External Tools.".into()),
    };
    command.current_dir(repo);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_terminal_treats_the_repository_as_an_argument() {
        for folder in ["R&D", "a^b", "100%", "x|y", "has space", "x;calc"] {
            let path = format!(r"C:\repo\{folder}");
            let command = windows_terminal(&path, "wt").unwrap();
            assert_eq!(command.get_program(), "wt.exe");
            assert_eq!(command.get_args().collect::<Vec<_>>(), [std::ffi::OsStr::new("-d"), std::ffi::OsStr::new(&wt_literal(&path))]);
            assert_eq!(command.get_current_dir(), Some(Path::new(&path)));
        }
    }
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
    fn windows_terminal_semicolons_stay_in_the_folder_argument() {
        assert_eq!(wt_literal(r"C:\repo\x;calc"), r"C:\repo\x\;calc");
        assert_eq!(wt_literal(r"C:\repo\plain"), r"C:\repo\plain");
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
