//! Stage 6 local stand-ins for the original's cloud features:
//! - Patches: share changes as a .patch file (instead of GitKraken's hosted Cloud Patches).
//! - Conflict prevention: warn early when your branch would conflict with its target
//!   (the original's "proactive conflict detection with target branch"), computed locally
//!   with `git merge-tree` — nothing leaves the computer.
//! - Agents: Claude Code sessions that worked in this repository.

use crate::git;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A patch of uncommitted changes (`wip`) or of the given commits (oldest first, mailbox format).
pub fn make_patch(dir: &Path, shas: &[String]) -> Result<String, String> {
    if shas.is_empty() {
        let o = git::run(dir, &["diff", "HEAD", "--binary", "--no-color", "--no-ext-diff"]);
        if !o.ok {
            return Err(o.message());
        }
        if o.stdout.trim().is_empty() {
            return Err("There are no uncommitted changes to put in a patch (new files must be staged first).".into());
        }
        return Ok(o.stdout);
    }
    let mut out = String::new();
    let total = shas.len();
    for (i, sha) in shas.iter().enumerate() {
        let o = git::run(dir, &["format-patch", "-1", "--stdout", "--binary", sha]);
        if !o.ok {
            return Err(o.message());
        }
        out.push_str(&number_subject(&o.stdout, i + 1, total));
    }
    Ok(out)
}

/// libgit2's email subject (nodegit.node 0x18028d000): "Subject: [PATCH] …" for one patch,
/// "[PATCH n/total]" when there are several (start number 1).
pub fn number_subject(patch: &str, n: usize, total: usize) -> String {
    if total <= 1 {
        return patch.to_string();
    }
    patch.replacen("Subject: [PATCH] ", &format!("Subject: [PATCH {n}/{total}] "), 1)
}

/// libgit2's merge defaults (nodegit.node 0x180296e60): find renames on, similarity threshold 50 %,
/// rename limit from merge.renamelimit, else diff.renamelimit, else 1000.
const MERGE_RENAME_THRESHOLD: u32 = 50;
const MERGE_RENAME_LIMIT: u32 = 1000;

fn rename_limit(dir: &Path) -> u32 {
    for key in ["merge.renamelimit", "diff.renamelimit"] {
        let o = git::run(dir, &["config", "--get", key]);
        if let Ok(n) = o.stdout.trim().parse::<u32>() {
            if n > 0 {
                return n;
            }
        }
    }
    MERGE_RENAME_LIMIT
}

/// Apply a patch file: mailbox patches become commits (`git am`), plain diffs change the working folder.
pub fn apply_patch_file(dir: &Path, file: &Path, run: &dyn Fn(&[&str]) -> git::GitOut) -> git::GitOut {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let f = file.to_string_lossy().into_owned();
    if text.starts_with("From ") {
        let o = run(&["am", "--3way", &f]);
        if !o.ok {
            let _ = git::run(dir, &["am", "--abort"]);
        }
        o
    } else {
        run(&["apply", "--3way", &f])
    }
}

#[derive(Serialize, Default)]
pub struct RepoSummary {
    /// False when the folder is gone or isn't a repository ("Repo is unreachable and may have been deleted").
    pub exists: bool,
    pub branch: Option<String>,
    /// Changed files in the working folder (the "WIP summary").
    pub changes: usize,
    pub ahead: u32,
    pub behind: u32,
}

/// Branch, uncommitted-change count and ahead / behind for the Repository Management list.
pub fn repo_summary(dir: &Path) -> RepoSummary {
    if !dir.is_dir() {
        return RepoSummary::default();
    }
    let o = git::run(dir, &["status", "--porcelain=v2", "--branch"]);
    if !o.ok {
        return RepoSummary::default();
    }
    let mut s = RepoSummary { exists: true, ..Default::default() };
    for line in o.stdout.lines() {
        if let Some(b) = line.strip_prefix("# branch.head ") {
            s.branch = (b != "(detached)").then(|| b.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut it = ab.split_whitespace();
            s.ahead = it.next().and_then(|x| x.trim_start_matches('+').parse().ok()).unwrap_or(0);
            s.behind = it.next().and_then(|x| x.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if !line.starts_with('#') && !line.is_empty() {
            s.changes += 1;
        }
    }
    s
}

/// The repository's pull / merge request template (GitHub or GitLab locations), if it has one.
pub fn pr_template(dir: &Path) -> Option<String> {
    let mut candidates: Vec<PathBuf> = [
        ".github/pull_request_template.md",
        ".github/PULL_REQUEST_TEMPLATE.md",
        "pull_request_template.md",
        "PULL_REQUEST_TEMPLATE.md",
        "docs/pull_request_template.md",
        "docs/PULL_REQUEST_TEMPLATE.md",
        ".gitlab/merge_request_templates/Default.md",
    ]
    .iter()
    .map(|p| dir.join(p))
    .collect();
    // Several templates: GitHub uses .github/PULL_REQUEST_TEMPLATE/*.md; take the first.
    if let Ok(rd) = std::fs::read_dir(dir.join(".github").join("PULL_REQUEST_TEMPLATE")) {
        let mut more: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "md").unwrap_or(false)).collect();
        more.sort();
        candidates.extend(more);
    }
    candidates.into_iter().find_map(|p| std::fs::read_to_string(p).ok()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[derive(Serialize, Default)]
pub struct ConflictCheck {
    pub target: String,
    pub files: Vec<String>,
    /// The check could not run (no target branch, or Git too old).
    pub note: Option<String>,
}

/// The branch to compare with: the remote's default branch (origin/HEAD), else origin/main or origin/master.
pub fn default_target(dir: &Path) -> Option<String> {
    let o = git::run(dir, &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"]);
    if o.ok && !o.stdout.trim().is_empty() {
        return Some(o.stdout.trim().to_string());
    }
    ["origin/main", "origin/master", "main", "master"]
        .iter()
        .find(|r| git::run(dir, &["rev-parse", "--verify", "-q", r]).ok)
        .map(|r| r.to_string())
}

/// Files that would conflict if HEAD were merged with `target` (no working-folder changes).
pub fn conflict_check(dir: &Path, target: Option<&str>) -> ConflictCheck {
    let Some(target) = target.map(|s| s.to_string()).or_else(|| default_target(dir)) else {
        return ConflictCheck { note: Some("No target branch to compare with.".into()), ..Default::default() };
    };
    let limit = format!("merge.renameLimit={}", rename_limit(dir));
    let threshold = format!("find-renames={MERGE_RENAME_THRESHOLD}%");
    let o = git::run(dir, &["-c", &limit, "merge-tree", "--write-tree", "--name-only", "--no-messages", "-X", &threshold, "HEAD", &target]);
    // Exit 0 = clean, 1 = conflicts (first line is the tree id, then the conflicted paths).
    if o.ok {
        return ConflictCheck { target, ..Default::default() };
    }
    let lines: Vec<&str> = o.stdout.lines().collect();
    if lines.first().map(|l| l.len() == 40 && l.chars().all(|c| c.is_ascii_hexdigit())).unwrap_or(false) {
        let mut files: Vec<String> = lines[1..].iter().filter(|l| !l.trim().is_empty()).map(|s| s.to_string()).collect();
        files.dedup();
        return ConflictCheck { target, files, note: None };
    }
    ConflictCheck { target, files: vec![], note: Some(o.message()) }
}

#[derive(Serialize, Clone)]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub modified: i64,
    pub messages: usize,
    /// "active" if written to in the last two minutes, else "idle".
    pub status: String,
}

/// Claude Code keeps each project's sessions under ~/.claude/projects/<path with non-alphanumerics as '-'>.
pub fn claude_project_dir(repo: &Path) -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    let name: String = repo.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    home.join(".claude").join("projects").join(name)
}

pub fn agent_sessions(repo: &Path) -> Vec<AgentSession> {
    let dir = claude_project_dir(repo);
    let now = std::time::SystemTime::now();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "jsonl").unwrap_or(true) {
            continue;
        }
        let modified = e.metadata().and_then(|m| m.modified()).ok();
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let mut title = String::new();
        let mut messages = 0;
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            if v["type"] == "user" || v["type"] == "assistant" {
                messages += 1;
            }
            if title.is_empty() && v["type"] == "user" {
                let c = &v["message"]["content"];
                let t = c.as_str().map(|s| s.to_string()).or_else(|| c.as_array().and_then(|a| a.iter().find_map(|b| b["text"].as_str().map(|s| s.to_string()))));
                if let Some(t) = t.filter(|t| !t.trim().is_empty() && !t.starts_with('<')) {
                    title = t.lines().next().unwrap_or_default().chars().take(120).collect();
                }
            }
        }
        let age = modified.and_then(|m| now.duration_since(m).ok()).map(|d| d.as_secs()).unwrap_or(u64::MAX);
        out.push(AgentSession {
            id: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            title: if title.is_empty() { "(untitled session)".into() } else { title },
            modified: modified.and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0),
            messages,
            status: if age < 120 { "active".into() } else { "idle".into() },
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn patches_and_conflict_check() {
        let d = std::env::temp_dir().join(format!("gitcraft-extras-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| assert!(git::run(&d, a).ok, "{a:?}");
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@e.x"]);
        g(&["config", "commit.gpgsign", "false"]);
        fs::write(d.join("a.txt"), "base\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        g(&["checkout", "-q", "-b", "side"]);
        fs::write(d.join("a.txt"), "side\n").unwrap();
        g(&["commit", "-q", "-am", "side change"]);
        let side = git::run(&d, &["rev-parse", "HEAD"]).stdout.trim().to_string();
        g(&["checkout", "-q", "main"]);
        // A clean comparison, then a conflicting one.
        assert!(conflict_check(&d, Some("side")).files.is_empty());
        fs::write(d.join("a.txt"), "main\n").unwrap();
        g(&["commit", "-q", "-am", "main change"]);
        assert_eq!(conflict_check(&d, Some("side")).files, vec!["a.txt".to_string()]);

        // Patch of a commit, applied as a commit on another branch.
        let patch = make_patch(&d, &[side.clone()]).unwrap();
        assert!(patch.starts_with("From "));
        let pf = d.join("..").join(format!("gitcraft-extras-{}.patch", std::process::id()));
        fs::write(&pf, patch).unwrap();
        g(&["checkout", "-q", "-b", "other", "main~1"]);
        assert!(apply_patch_file(&d, &pf, &|a: &[&str]| git::run(&d, a)).ok);
        assert_eq!(git::run(&d, &["log", "-1", "--format=%s"]).stdout.trim(), "side change");
        // Patch of uncommitted work.
        fs::write(d.join("a.txt"), "wip\n").unwrap();
        assert!(make_patch(&d, &[]).unwrap().contains("+wip"));
        let _ = fs::remove_file(&pf);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn patch_subjects_are_numbered_like_libgit2() {
        let p = "From abc Mon Sep 17 00:00:00 2001\nSubject: [PATCH] Fix it\n";
        assert_eq!(number_subject(p, 1, 1), p);
        assert!(number_subject(p, 2, 3).contains("Subject: [PATCH 2/3] Fix it"));
    }

    #[test]
    fn summary_of_a_missing_folder_and_a_repo() {
        assert!(!repo_summary(Path::new("Z:/surely/not/here")).exists);
        let d = std::env::temp_dir().join(format!("gitcraft-summary-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        assert!(git::run(&d, &["init", "-q", "-b", "main"]).ok);
        fs::write(d.join("x.txt"), "x").unwrap();
        let s = repo_summary(&d);
        assert!(s.exists && s.changes == 1 && s.branch.as_deref() == Some("main"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn status_finds_staged_renames_even_when_config_turns_them_off() {
        let d = std::env::temp_dir().join(format!("gitcraft-renames-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| assert!(git::run(&d, a).ok, "{a:?}");
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@e.x"]);
        g(&["config", "commit.gpgsign", "false"]);
        g(&["config", "status.renames", "false"]);
        fs::write(d.join("old.txt"), "one\ntwo\nthree\nfour\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        g(&["mv", "old.txt", "new.txt"]);
        let s = git::status(&d);
        assert_eq!(s.staged.len(), 1, "{:?}", s.staged);
        assert_eq!((s.staged[0].path.as_str(), s.staged[0].status), ("new.txt", 'R'));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn claude_project_folder_name() {
        let p = claude_project_dir(Path::new(r"C:\Users\Sam\My Project"));
        assert!(p.ends_with(Path::new(".claude").join("projects").join("C--Users-Sam-My-Project")));
    }
}
