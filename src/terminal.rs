//! Built-in terminal: a real shell in a pseudo-terminal (ConPTY on Windows), streamed to the
//! window's xterm.js panel. Each terminal tab is one session.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

struct Session {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct Terminals {
    sessions: Mutex<HashMap<u32, Session>>,
    next: AtomicU32,
}

/// The shell to start: PowerShell on Windows, the user's shell elsewhere.
fn default_shell() -> CommandBuilder {
    if cfg!(windows) {
        let pwsh = std::process::Command::new("where").arg("pwsh").output().map(|o| o.status.success()).unwrap_or(false);
        CommandBuilder::new(if pwsh { "pwsh.exe" } else { "powershell.exe" })
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
        cwd: &str,
        cols: u16,
        rows: u16,
        on_data: Arc<dyn Fn(u32, String) + Send + Sync>,
        on_exit: Arc<dyn Fn(u32) + Send + Sync>,
    ) -> Result<u32, String> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
        let mut cmd = default_shell();
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
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
                            on_data(id, text);
                        }
                    }
                }
            }
            on_exit(id);
        });
        self.sessions.lock().unwrap().insert(id, Session { writer, master: pair.master, child });
        Ok(id)
    }

    pub fn write(&self, id: u32, data: &str) -> Result<(), String> {
        let mut s = self.sessions.lock().unwrap();
        let sess = s.get_mut(&id).ok_or("That terminal has closed")?;
        sess.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        sess.writer.flush().map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<(), String> {
        let s = self.sessions.lock().unwrap();
        let sess = s.get(&id).ok_or("That terminal has closed")?;
        sess.master.resize(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string())
    }

    pub fn close(&self, id: u32) {
        if let Some(mut sess) = self.sessions.lock().unwrap().remove(&id) {
            let _ = sess.child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let t = Terminals::default();
        let out = Arc::new(Mutex::new(String::new()));
        let o2 = out.clone();
        let id = t
            .open(&std::env::temp_dir().to_string_lossy(), 80, 24, Arc::new(move |_, s| o2.lock().unwrap().push_str(&s)), Arc::new(|_| {}))
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
