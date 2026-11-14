//! Built-in terminal: a real shell in a pseudo-terminal (ConPTY on Windows), streamed to the
//! window's xterm.js panel. Each terminal tab is one session.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

struct Session {
    writer: std::sync::mpsc::SyncSender<Vec<u8>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    output: Arc<OutputFlow>,
}

#[derive(Default)]
struct OutputFlow {
    state: Mutex<(usize, bool)>,
    changed: std::sync::Condvar,
}
impl OutputFlow {
    fn reserve(&self, bytes: usize) -> bool {
        let mut state = self.state.lock().unwrap();
        while state.0 > 0 && state.0 + bytes > 64 * 1024 && !state.1 { state = self.changed.wait(state).unwrap(); }
        if state.1 { return false; }
        state.0 += bytes;
        true
    }
    fn acknowledge(&self, bytes: usize) { let mut state = self.state.lock().unwrap(); state.0 = state.0.saturating_sub(bytes); self.changed.notify_all(); }
    fn close(&self) { self.state.lock().unwrap().1 = true; self.changed.notify_all(); }
    fn drain(&self) {
        let mut state = self.state.lock().unwrap();
        while state.0 > 0 && !state.1 { state = self.changed.wait(state).unwrap(); }
    }
}

#[derive(Default)]
pub struct Terminals {
    sessions: Arc<Mutex<HashMap<u32, Session>>>,
    next: AtomicU32,
}

/// Resolve the executable once; passing its full path also avoids PTY environment casing differences on Windows.
fn program_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|p| p.join(name)).find(|p| p.is_file()))
}

fn shell_command(name: &str) -> CommandBuilder {
    CommandBuilder::new(program_path(name).unwrap_or_else(|| name.into()))
}

/// Preferences > In-App Terminal > Default In-App Terminal: powershell, pwsh, cmd, gitbash, wsl (Windows);
/// default, bash, zsh, fish elsewhere.
pub fn shell_for(name: &str) -> CommandBuilder {
    match name {
        "powershell" if cfg!(windows) => shell_command("powershell.exe"),
        "pwsh" => shell_command(if cfg!(windows) { "pwsh.exe" } else { "pwsh" }),
        "cmd" if cfg!(windows) => shell_command("cmd.exe"),
        "wsl" if cfg!(windows) => shell_command("wsl.exe"),
        "gitbash" if cfg!(windows) => {
            // Git for Windows: <install>\cmd\git.exe -> <install>\bin\bash.exe
            let git = std::process::Command::new("where").arg("git").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or_default().trim().to_string()).unwrap_or_default();
            let bash = std::path::Path::new(&git).parent().and_then(|p| p.parent()).map(|p| p.join("bin").join("bash.exe")).filter(|p| p.exists());
            let mut c = CommandBuilder::new(bash.map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "bash.exe".into()));
            c.args(["--login", "-i"]);
            c
        }
        "bash" | "zsh" | "fish" => CommandBuilder::new(name),
        _ => default_shell(),
    }
}

/// The shell to start: PowerShell on Windows, the user's shell elsewhere.
fn default_shell() -> CommandBuilder {
    if cfg!(windows) {
        match program_path("pwsh.exe") {
            Some(path) => CommandBuilder::new(path),
            None => shell_command("powershell.exe"),
        }
    } else {
        CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()))
    }
}

/// Splits bytes into valid UTF-8 text, keeping an incomplete character for the next chunk.
pub fn take_utf8(buf: &mut Vec<u8>) -> String {
    match std::str::from_utf8(buf) {
        Ok(s) => {
            let s = s.to_string();
            buf.clear();
            s
        }
        Err(e) => {
            let good = e.valid_up_to();
            // An invalid byte (not just a cut-off character): replace it and move on.
            let cut = if e.error_len().is_some() { buf.len() } else { good };
            let s = String::from_utf8_lossy(&buf[..cut]).into_owned();
            buf.drain(..cut);
            s
        }
    }
}

impl Terminals {
    /// Start a shell in `cwd`. Output goes to `on_data`; `on_exit` runs when the shell ends.
    pub fn open(
        &self,
        shell: &str,
        cwd: &str,
        cols: u16,
        rows: u16,
        agent_env: Option<&HashMap<String, String>>,
        on_data: Arc<dyn Fn(u32, String) + Send + Sync>,
        on_exit: Arc<dyn Fn(u32) + Send + Sync>,
    ) -> Result<u32, String> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
        let mut cmd = shell_for(shell);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        // Keep agent config paths out of shell command text.
        if let Some(vars) = agent_env {
            for key in ["CODEX_HOME", "CLAUDE_CONFIG_DIR"] {
                if let Some(value) = vars.get(key) {
                    let expanded = value.strip_prefix("~/").or_else(|| value.strip_prefix("~\\"))
                        .and_then(|rest| std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|home| std::path::PathBuf::from(home).join(rest)));
                    cmd.env(key, expanded.unwrap_or_else(|| value.into()));
                }
            }
        }
        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let (input, queued) = std::sync::mpsc::sync_channel::<Vec<u8>>(8);
        std::thread::spawn(move || {
            for bytes in queued {
                if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() { break; }
            }
        });
        let output = Arc::new(OutputFlow::default());
        self.sessions.lock().unwrap().insert(id, Session { writer: input, master: Arc::new(Mutex::new(pair.master)), child, output: output.clone() });
        let sessions = self.sessions.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut pending = Vec::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        pending.extend_from_slice(&buf[..n]);
                        let text = take_utf8(&mut pending);
                        if !text.is_empty() {
                            if !output.reserve(text.len()) { break; }
                            on_data(id, text);
                        }
                    }
                }
            }
            if !pending.is_empty() {
                let text = String::from_utf8_lossy(&pending).into_owned();
                if output.reserve(text.len()) { on_data(id, text); }
            }
            output.drain();
            // EOF releases the PTY and reaps shells that exited without the user closing their tab.
            let session = sessions.lock().unwrap().remove(&id);
            if let Some(mut session) = session {
                let _ = session.child.wait();
            }
            on_exit(id);
        });
        Ok(id)
    }

    pub fn write(&self, id: u32, data: &str) -> Result<(), String> {
        if data.len() > 256 * 1024 { return Err("Paste at most 256 KB at a time into the terminal.".into()); }
        let writer = self.sessions.lock().unwrap().get(&id).ok_or("That terminal has closed")?.writer.clone();
        writer.try_send(data.as_bytes().to_vec()).map_err(|_| "Terminal input is busy or closed. Wait before sending more input.".into())
    }

    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<(), String> {
        let master = self.sessions.lock().unwrap().get(&id).ok_or("That terminal has closed")?.master.clone();
        let result = master.lock().unwrap().resize(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string());
        result
    }

    pub fn acknowledge(&self, id: u32, bytes: usize) {
        let output = self.sessions.lock().unwrap().get(&id).map(|s| s.output.clone());
        if let Some(output) = output { output.acknowledge(bytes); }
    }

    pub fn close(&self, id: u32) {
        let session = self.sessions.lock().unwrap().remove(&id);
        if let Some(mut sess) = session {
            sess.output.close();
            let _ = sess.child.kill();
            std::thread::spawn(move || { let _ = sess.child.wait(); });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_waits_for_renderer_acknowledgement_and_close_releases_waiters() {
        use std::time::Duration;
        let flow = Arc::new(OutputFlow::default());
        assert!(flow.reserve(64 * 1024));
        let other = flow.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || { tx.send(other.reserve(1)).unwrap(); });
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());
        flow.acknowledge(64 * 1024);
        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap()); worker.join().unwrap();
        let other = flow.clone(); let worker = std::thread::spawn(move || other.drain());
        flow.close(); worker.join().unwrap(); assert!(!flow.reserve(1));
    }

    #[test]
    fn keeps_cut_off_characters_for_the_next_chunk() {
        let mut b = "hé".as_bytes()[..2].to_vec(); // "h" + first byte of "é"
        assert_eq!(take_utf8(&mut b), "h");
        assert_eq!(b.len(), 1);
        b.extend_from_slice(&"é".as_bytes()[1..]);
        assert_eq!(take_utf8(&mut b), "é");
        assert!(b.is_empty());
    }

    #[test]
    fn runs_a_shell_command() {
        let t = Arc::new(Terminals::default());
        let out = Arc::new(Mutex::new(String::new()));
        let o2 = out.clone();
        let ack = t.clone();
        let id = t
            .open("", &std::env::temp_dir().to_string_lossy(), 80, 24, None, Arc::new(move |id, s| { o2.lock().unwrap().push_str(&s); ack.acknowledge(id, s.len()); }), Arc::new(|_| {}))
            .unwrap();
        // Like xterm.js does in the app: answer the console's "where is the cursor?" request.
        let mut answered = false;
        let mut sent = false;
        for _ in 0..200 {
            let got = out.lock().unwrap().clone();
            if !answered && got.contains("\x1b[6n") {
                t.write(id, "\x1b[1;1R").unwrap();
                answered = true;
            }
            if !sent && (answered || got.len() > 20) {
                std::thread::sleep(std::time::Duration::from_millis(300));
                t.write(id, "echo gitcraft-terminal-ok\r").unwrap();
                sent = true;
            }
            if got.matches("gitcraft-terminal-ok").count() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        t.close(id);
        let got = out.lock().unwrap().clone();
        assert!(got.contains("gitcraft-terminal-ok"), "terminal output was: {:?}", got.chars().take(600).collect::<String>());
    }
}
