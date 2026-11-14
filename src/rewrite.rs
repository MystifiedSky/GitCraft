//! Stage 3: interactive rebase / cherry pick and merge-conflict information.
//!
//! An interactive rebase writes its own todo list and points
//! GIT_SEQUENCE_EDITOR at a small shell loop that copies that list over git's `git-rebase-todo`.
//! The actions are four: pick, reword, squash, drop.

use crate::git::{self, Commit};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// GIT_SEQUENCE_EDITOR: copy our todo file line by line into git's todo file.
const SEQUENCE_EDITOR: &str = r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done < "$GITCRAFT_REBASE_TODO_PATH" >"#;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Pick,
    Reword,
    Squash,
    Drop,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TodoItem {
    pub action: Step,
    pub sha: String,
    /// New message for reword (and the combined message for a squash target, optional).
    pub message: Option<String>,
}

fn git_dir(dir: &Path) -> PathBuf {
    let o = git::run(dir, &["rev-parse", "--absolute-git-dir"]);
    PathBuf::from(o.stdout.trim())
}

fn parse_commits(text: &str) -> Vec<Commit> {
    text.split('\x1e')
        .filter_map(|rec| {
            let rec = rec.trim_start_matches(['\n', '\r']);
            let f: Vec<&str> = rec.splitn(7, '\x1f').collect();
            (f.len() == 7).then(|| Commit {
                sha: f[0].to_string(),
                parents: f[1].split_whitespace().map(|s| s.to_string()).collect(),
                author: f[2].to_string(),
                email: f[3].to_string(),
                time: f[4].parse().unwrap_or(0),
                summary: f[5].to_string(),
                body: f[6].trim().to_string(),
            })
        })
        .collect()
}

/// Commits an interactive rebase onto `base` would replay (oldest first). `base` "--root" = all.
pub fn rebase_commits(dir: &Path, base: &str) -> Result<Vec<Commit>, String> {
    let range = if base == "--root" { "HEAD".to_string() } else { format!("{base}..HEAD") };
    let o = git::run(dir, &["log", "--reverse", "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1e", &range]);
    if !o.ok {
        return Err(o.message());
    }
    let commits = parse_commits(&o.stdout);
    if commits.iter().any(|c| c.parents.len() > 1) {
        return Err("These commits include a merge. Interactive rebase over merge commits isn't supported.".into());
    }
    Ok(commits)
}

/// Runs `git rebase -i` with our own todo list. Uncommitted changes are stashed and restored
/// around it ("Auto stash before rebase").
pub fn interactive_rebase(dir: &Path, base: &str, items: &[TodoItem]) -> git::GitOut {
    let todo = match write_todo(dir, items) {
        Ok(t) => t,
        Err(e) => return git::GitOut { ok: false, stdout: String::new(), stderr: e },
    };
    let todo_s = todo.to_string_lossy().replace('\\', "/");
    let mut args = vec!["-c", "core.editor=true", "rebase", "-i", "--autostash"];
    if base == "--root" {
        args.push("--root");
    } else {
        args.push(base);
    }
    git::run_env(dir, &args, &[("GIT_SEQUENCE_EDITOR", SEQUENCE_EDITOR), ("GITCRAFT_REBASE_TODO_PATH", &todo_s)])
}

/// Writes our todo list (plus a message file per reword) and returns its path.
/// Squash = "meld into the previous commit and keep both messages": git's own `squash` asks an
/// editor for the combined message; with core.editor=true it keeps git's combined text, whose
/// comment lines are cleaned up. A squash with nothing before it is a pick. A reword replays the
/// commit, then an `exec` line amends only its message from the file.
fn write_todo(dir: &Path, items: &[TodoItem]) -> Result<PathBuf, String> {
    let gd = git_dir(dir);
    let todo = gd.join("gitcraft-rebase-todo");
    let mut text = String::new();
    let mut picked_any = false;
    for (i, it) in items.iter().enumerate() {
        match it.action {
            Step::Drop => text.push_str(&format!("drop {}\n", it.sha)),
            Step::Squash if picked_any => text.push_str(&format!("squash {}\n", it.sha)),
            _ => {
                text.push_str(&format!("pick {}\n", it.sha));
                picked_any = true;
            }
        }
        if it.action == Step::Reword {
            let msg = gd.join(format!("gitcraft-msg-{i}"));
            std::fs::write(&msg, it.message.clone().unwrap_or_default()).map_err(|e| e.to_string())?;
            let p = msg.to_string_lossy().replace('\\', "/");
            text.push_str(&format!("exec git commit --amend --only --allow-empty --no-verify -F '{p}'\n"));
        }
    }
    std::fs::write(&todo, text).map_err(|e| e.to_string())?;
    Ok(todo)
}

/// Interactive cherry pick: replay the listed commits onto HEAD in order.
pub fn interactive_cherry_pick(dir: &Path, items: &[TodoItem], log: &dyn Fn(&[&str]) -> git::GitOut) -> git::GitOut {
    let mut done_any = false;
    for it in items {
        let o = match it.action {
            Step::Drop => continue,
            Step::Squash if done_any => {
                let o = log(&["cherry-pick", "--no-commit", &it.sha]);
                if !o.ok {
                    return o;
                }
                // Keep both messages, like git's squash.
                let prev = git::run(dir, &["log", "-1", "--format=%B", "HEAD"]).stdout;
                let this = git::run(dir, &["log", "-1", "--format=%B", &it.sha]).stdout;
                let msg = format!("{}\n\n{}", prev.trim_end(), this.trim_end());
                let f = git_dir(dir).join("gitcraft-squash-msg");
                if let Err(e) = std::fs::write(&f, msg) {
                    return git::GitOut { ok: false, stdout: String::new(), stderr: e.to_string() };
                }
                log(&["commit", "--amend", "--no-verify", "-F", &f.to_string_lossy()])
            }
            _ => {
                let o = log(&["cherry-pick", &it.sha]);
                if o.ok && it.action == Step::Reword {
                    let f = git_dir(dir).join("gitcraft-reword-msg");
                    if let Err(e) = std::fs::write(&f, it.message.clone().unwrap_or_default()) {
                        return git::GitOut { ok: false, stdout: String::new(), stderr: e.to_string() };
                    }
                    log(&["commit", "--amend", "--only", "--no-verify", "-F", &f.to_string_lossy()])
                } else {
                    o
                }
            }
        };
        if !o.ok {
            return o;
        }
        done_any = true;
    }
    git::GitOut { ok: true, stdout: String::new(), stderr: String::new() }
}

#[derive(Serialize, Default)]
pub struct ConflictInfo {
    /// The working-folder file with conflict markers.
    pub working: Option<String>,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    /// Labels for A (current) and B (incoming), e.g. "master" / "feature/x".
    pub ours_label: String,
    pub theirs_label: String,
    pub binary: bool,
}

fn stage(dir: &Path, n: u8, file: &str) -> Option<String> {
    crate::files::contents(dir, &format!(":{n}"), file).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The names shown for "current" and "incoming": during a merge HEAD's branch and MERGE_HEAD's
/// name; during a rebase the branch being rebased onto and the commit being replayed.
pub fn conflict_labels(dir: &Path) -> (String, String) {
    let gd = git_dir(dir);
    let head = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
    if gd.join("MERGE_HEAD").exists() {
        let msg = std::fs::read_to_string(gd.join("MERGE_MSG")).unwrap_or_default();
        let incoming = msg
            .lines()
            .next()
            .and_then(|l| l.split('\'').nth(1))
            .map(|s| s.to_string())
            .unwrap_or_else(|| git::run(dir, &["rev-parse", "--short", "MERGE_HEAD"]).stdout.trim().to_string());
        return (if head.is_empty() { "HEAD".into() } else { head }, incoming);
    }
    for d in ["rebase-merge", "rebase-apply"] {
        let p = gd.join(d);
        if p.exists() {
            let onto = std::fs::read_to_string(p.join("onto")).unwrap_or_default();
            let onto_name = git::run(dir, &["name-rev", "--name-only", "--no-undefined", onto.trim()]).stdout.trim().to_string();
            let head_name = std::fs::read_to_string(p.join("head-name")).unwrap_or_default().trim().trim_start_matches("refs/heads/").to_string();
            return (if onto_name.is_empty() { onto.trim().chars().take(7).collect() } else { onto_name }, head_name);
        }
    }
    if gd.join("CHERRY_PICK_HEAD").exists() {
        let s = git::run(dir, &["rev-parse", "--short", "CHERRY_PICK_HEAD"]).stdout.trim().to_string();
        return (if head.is_empty() { "HEAD".into() } else { head }, s);
    }
    (if head.is_empty() { "HEAD".into() } else { head }, "incoming".into())
}

pub fn conflict_info(dir: &Path, file: &str) -> ConflictInfo {
    let working = std::fs::read(dir.join(file)).ok();
    let binary = working.as_deref().map(crate::files::is_binary).unwrap_or(false);
    let (ours_label, theirs_label) = conflict_labels(dir);
    ConflictInfo {
        working: working.map(|b| String::from_utf8_lossy(&b).into_owned()),
        base: stage(dir, 1, file),
        ours: stage(dir, 2, file),
        theirs: stage(dir, 3, file),
        ours_label,
        theirs_label,
        binary,
    }
}

/// The message git prepared for a merge commit (MERGE_MSG without comment lines).
pub fn merge_message(dir: &Path) -> Option<String> {
    let gd = git_dir(dir);
    let s = std::fs::read_to_string(gd.join("MERGE_MSG")).ok()?;
    let msg: Vec<&str> = s.lines().filter(|l| !l.starts_with('#')).collect();
    Some(msg.join("\n").trim().to_string()).filter(|m| !m.is_empty())
}
