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


/// Commits an interactive rebase onto `base` would replay (oldest first). `base` "--root" = all.
pub fn rebase_commits(dir: &Path, base: &str) -> Result<Vec<Commit>, String> {
    let range = if base == "--root" { "HEAD".to_string() } else { format!("{base}..HEAD") };
    let o = git::run(dir, &["log", "--reverse", "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%b%x00", &range]);
    if !o.ok {
        return Err(o.message());
    }
    let commits = git::parse_commits(&o.stdout);
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
            let quoted = format!("'{}'", p.replace('\'', "'\\''"));
            text.push_str(&format!("exec git commit --amend --only --allow-empty --no-verify -F {quoted}\n"));
        }
    }
    std::fs::write(&todo, text).map_err(|e| e.to_string())?;
    Ok(todo)
}

/// Interactive cherry pick: replay the listed commits onto HEAD in order.
pub fn interactive_cherry_pick(dir: &Path, items: &[TodoItem], _log: &dyn Fn(&[&str]) -> git::GitOut) -> git::GitOut {
    if items.iter().all(|item| item.action == Step::Drop) { return git::GitOut { ok: true, stdout: String::new(), stderr: String::new() }; }
    let gd = git_dir(dir);
    if gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists() || gd.join("CHERRY_PICK_HEAD").exists() {
        return git::GitOut::failed("Finish or abort the current replay before starting another plan.");
    }
    let head = git::run(dir, &["rev-parse", "--verify", "HEAD"]);
    if !head.ok { return head; }
    let todo = match write_todo(dir, items) { Ok(todo) => todo, Err(error) => return git::GitOut::failed(&error) };
    let todo = todo.to_string_lossy().replace('\\', "/");
    // Git owns the complete plan, including the remaining steps after a conflict.
    // Replaying the selected commits onto the current tip appends them to this branch.
    git::run_env(dir, &["-c", "core.editor=true", "rebase", "-i", "--autostash", "--reapply-cherry-picks", "--onto", head.stdout.trim(), "--root"],
        &[("GIT_SEQUENCE_EDITOR", SEQUENCE_EDITOR), ("GITCRAFT_REBASE_TODO_PATH", &todo)])
}


#[derive(Serialize, Default)]
pub struct ConflictInfo {
    pub working_token: String,
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

fn decode_text(dir: &Path, bytes: &[u8]) -> String {
    match crate::prefs::encoding_for(dir) {
        Some(encoding) => crate::prefs::decode(bytes, encoding),
        None => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn stage(dir: &Path, n: u8, file: &str) -> Option<String> {
    crate::files::contents(dir, &format!(":{n}"), file).map(|b| decode_text(dir, &b))
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
    let encoding = crate::prefs::encoding_for(dir);
    let utf16 = encoding.is_some_and(|e| e == encoding_rs::UTF_16LE || e == encoding_rs::UTF_16BE);
    let binary = !utf16 && working.as_deref().map(crate::files::is_binary).unwrap_or(false);
    let (ours_label, theirs_label) = conflict_labels(dir);
    ConflictInfo {
        working_token: crate::files::content_token(working.as_deref()),
        working: working.map(|b| decode_text(dir, &b)),
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PlanState { pub head: String, pub branch: String, pub base: String }
pub fn plan_state(dir: &Path, base: &str) -> Result<PlanState, String> {
    let head = git::run(dir, &["rev-parse", "--verify", "HEAD"]);
    if !head.ok { return Err(head.message()); }
    let branch = git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_owned();
    let base = if base == "--root" { base.into() } else {
        let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &format!("{base}^{{commit}}")]);
        if !resolved.ok { return Err(resolved.message()); } resolved.stdout.trim().to_owned()
    };
    Ok(PlanState { head: head.stdout.trim().into(), branch, base })
}
