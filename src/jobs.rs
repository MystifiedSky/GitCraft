//! Long Git jobs (clone, fetch, pull, push) with live progress and Cancel.
//! git writes progress to stderr with carriage returns ("Receiving objects:  45% (450/1000)");
//! each line is passed to a listener (the window shows it in the status bar).

use crate::git::{self, GitOut};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};

pub type Listener = Arc<dyn Fn(&str, &str, Option<u32>) + Send + Sync>;

#[derive(Default)]
pub struct Jobs {
    running: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    listener: Mutex<Option<Listener>>,
}

/// The percentage at the end of a git progress line, if any ("...  45% (450/1000)").
pub fn percent(line: &str) -> Option<u32> {
    let i = line.rfind('%')?;
    let digits: String = line[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().ok().filter(|p: &u32| *p <= 100)
}

/// Git uses CR for progress updates, CRLF on some platforms, and may omit the last newline.
#[derive(Default)]
struct ProgressOutput {
    pending: Vec<u8>,
    progress: Option<String>,
    stderr: String,
}

impl ProgressOutput {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut updates = Vec::new();
        for &b in bytes {
            if b == b'\r' || b == b'\n' {
                if !self.pending.is_empty() {
                    let text = String::from_utf8_lossy(&self.pending).trim().to_string();
                    self.pending.clear();
                    updates.push(text.clone());
                    if b == b'\r' {
                        self.progress = Some(text);
                    } else {
                        self.stderr.push_str(&text);
                        self.stderr.push('\n');
                        self.progress = None;
                    }
                } else if b == b'\n' {
                    if let Some(text) = self.progress.take() {
                        self.stderr.push_str(&text);
                        self.stderr.push('\n');
                    }
                }
            } else {
                self.pending.push(b);
            }
        }
        updates
    }

    fn finish(mut self) -> String {
        if !self.pending.is_empty() {
            self.stderr.push_str(&String::from_utf8_lossy(&self.pending));
            self.stderr.push('\n');
        } else if let Some(text) = self.progress.take() {
            self.stderr.push_str(&text);
            self.stderr.push('\n');
        }
        self.stderr
    }
}

impl Jobs {
    pub fn set_listener(&self, l: Listener) {
        *self.listener.lock().unwrap() = Some(l);
    }

    /// Runs git, reporting each progress line as (job id, text, percent). Can be cancelled by id.
    pub fn run(&self, id: &str, dir: &Path, args: &[&str]) -> GitOut {
        let mut cmd = git::git_cmd(dir);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return GitOut { ok: false, stdout: String::new(), stderr: format!("Could not start git: {e}") },
        };
        let mut err_pipe = child.stderr.take().expect("stderr");
        let mut out_pipe = child.stdout.take().expect("stdout");
        let child = Arc::new(Mutex::new(child));
        self.running.lock().unwrap().insert(id.to_string(), child.clone());
        let out_thread = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out_pipe.read_to_string(&mut s);
            s
        });
        let listener = self.listener.lock().unwrap().clone();
        let mut output = ProgressOutput::default();
        let mut buf = [0u8; 1024];
        loop {
            let n = match err_pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for text in output.push(&buf[..n]) {
                if let Some(l) = &listener {
                    l(id, &text, percent(&text));
                }
            }
        }
        let stderr = output.finish();
        let status = child.lock().unwrap().wait();
        self.running.lock().unwrap().remove(id);
        let stdout = out_thread.join().unwrap_or_default();
        let ok = status.map(|s| s.success()).unwrap_or(false);
        if let Some(l) = &listener {
            l(id, "", None); // finished
        }
        GitOut { ok, stdout, stderr }
    }

    /// Stop a running job. Returns false if it already finished.
    pub fn cancel(&self, id: &str) -> bool {
        let job = self.running.lock().unwrap().get(id).cloned();
        match job {
            Some(c) => c.lock().map(|mut c| c.kill().is_ok()).unwrap_or(false),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_git_progress_percentages() {
        assert_eq!(percent("Receiving objects:  45% (450/1000), 1.2 MiB | 3 MiB/s"), Some(45));
        assert_eq!(percent("Resolving deltas: 100% (20/20), done."), Some(100));
        assert_eq!(percent("remote: Enumerating objects: 5, done."), None);
    }

    #[test]
    fn preserves_crlf_and_unterminated_errors() {
        let mut output = ProgressOutput::default();
        output.push(b"remote: message\r");
        output.push(b"\nfatal: authentication failed");
        assert_eq!(output.finish(), "remote: message\nfatal: authentication failed\n");
    }

    #[test]
    fn keeps_only_the_final_progress_update() {
        let mut output = ProgressOutput::default();
        assert_eq!(output.push(b"Receiving: 10%\rReceiving: 20%\r"), vec!["Receiving: 10%", "Receiving: 20%"]);
        output.push(b"\n");
        assert_eq!(output.finish(), "Receiving: 20%\n");
        let mut output = ProgressOutput::default();
        output.push(b"Resolving: 100%\r");
        assert_eq!(output.finish(), "Resolving: 100%\n");
    }

    #[test]
    fn runs_and_reports() {
        let jobs = Jobs::default();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let s2 = seen.clone();
        jobs.set_listener(Arc::new(move |_id, t, _p| s2.lock().unwrap().push(t.to_string())));
        let o = jobs.run("v", &std::env::temp_dir(), &["--version"]);
        assert!(o.ok && o.stdout.starts_with("git version"));
        assert!(!jobs.cancel("v"));
        assert!(seen.lock().unwrap().last().map(|s| s.is_empty()).unwrap_or(false));
    }
}
