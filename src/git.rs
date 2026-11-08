//! Everything that talks to the `git` program.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Clone, Debug, Serialize)]
pub struct Commit {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub summary: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum RefKind {
    Local,
    Remote,
    Tag,
}

#[derive(Clone, Debug, Serialize)]
pub struct GitRef {
    pub name: String,      // short name, e.g. "master", "origin/master", "v1.0"
    pub full: String,      // full name, e.g. "refs/heads/master"
    pub sha: String,       // commit it points at (tags are peeled)
    pub kind: RefKind,
    pub upstream: String,  // for local branches
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Stash {
    pub index: usize,
    pub sha: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileChange {
    pub path: String,
    pub status: char, // M, A, D, R, ?, U
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkStatus {
    pub unstaged: Vec<FileChange>,
    pub staged: Vec<FileChange>,
    pub conflicted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Worktree {
    pub path: String,
    pub branch: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RepoData {
    pub commits: Vec<Commit>,
    pub refs: Vec<GitRef>,
    pub stashes: Vec<Stash>,
    pub worktrees: Vec<Worktree>,
    pub submodules: Vec<String>,
    pub remotes: Vec<String>,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub status: WorkStatus,
    pub state_note: Option<String>, // "merging", "rebasing" ...
    #[serde(skip)]
    pub index_of: HashMap<String, usize>,
}

/// One finished git command, kept for the activity log.
#[derive(Clone, Debug, Serialize)]
pub struct LogEntry {
    pub time: String,
    pub command: String,
    pub ok: bool,
    pub output: String,
}

pub struct GitOut {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

impl GitOut {
    pub fn message(&self) -> String {
        let s = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        s.trim().to_string()
    }
}

pub fn git_cmd(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir);
    c.env("GIT_TERMINAL_PROMPT", "0");
    c.env("GIT_EDITOR", "true");
    c.env("GIT_SEQUENCE_EDITOR", "true");
    #[cfg(windows)]
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    c
}

pub fn run(dir: &Path, args: &[&str]) -> GitOut {
    match git_cmd(dir).args(args).output() {
        Ok(o) => GitOut {
            ok: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => GitOut { ok: false, stdout: String::new(), stderr: format!("Could not start git: {e}") },
    }
}

pub fn run_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> GitOut {
    let mut c = git_cmd(dir);
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    match c.output() {
        Ok(o) => GitOut {
            ok: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => GitOut { ok: false, stdout: String::new(), stderr: format!("Could not start git: {e}") },
    }
}

pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    let o = run(dir, &["rev-parse", "--show-toplevel"]);
    if o.ok {
        Some(PathBuf::from(o.stdout.trim()))
    } else {
        None
    }
}

pub fn load(dir: &Path, max_commits: usize) -> RepoData {
    let mut d = RepoData::default();

    // Refs (branches, remote branches, tags) with ahead/behind info.
    let refs = run(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname)%1f%(objectname)%1f%(*objectname)%1f%(upstream:short)%1f%(upstream:track)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    );
    for line in refs.stdout.lines() {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() < 5 {
            continue;
        }
        let full = f[0].to_string();
        let sha = if f[2].is_empty() { f[1] } else { f[2] }.to_string();
        let (kind, name) = if let Some(n) = full.strip_prefix("refs/heads/") {
            (RefKind::Local, n.to_string())
        } else if let Some(n) = full.strip_prefix("refs/remotes/") {
            if n.ends_with("/HEAD") {
                continue;
            }
            (RefKind::Remote, n.to_string())
        } else if let Some(n) = full.strip_prefix("refs/tags/") {
            (RefKind::Tag, n.to_string())
        } else {
            continue;
        };
        let (ahead, behind) = parse_track(f[4]);
        d.refs.push(GitRef { name, full, sha, kind, upstream: f[3].to_string(), ahead, behind });
    }

    let head = run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]);
    if head.ok {
        d.head_branch = Some(head.stdout.trim().to_string());
    }
    d.head_sha = run(dir, &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // History, newest first, every branch.
    let n = format!("-n{max_commits}");
    let log = run(
        dir,
        &[
            "log",
            "--branches",
            "--remotes",
            "--tags",
            "HEAD",
            "--date-order",
            &n,
            "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1e",
        ],
    );
    for rec in log.stdout.split('\x1e') {
        let rec = rec.trim_start_matches(['\n', '\r']);
        if rec.is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.splitn(7, '\x1f').collect();
        if f.len() < 7 {
            continue;
        }
        d.commits.push(Commit {
            sha: f[0].to_string(),
            parents: f[1].split_whitespace().map(|s| s.to_string()).collect(),
            author: f[2].to_string(),
            email: f[3].to_string(),
            time: f[4].parse().unwrap_or(0),
            summary: f[5].to_string(),
            body: f[6].trim().to_string(),
        });
    }
    for (i, c) in d.commits.iter().enumerate() {
        d.index_of.insert(c.sha.clone(), i);
    }

    let st = run(dir, &["stash", "list", "--format=%H%x1f%gs"]);
    for (i, line) in st.stdout.lines().enumerate() {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() == 2 {
            d.stashes.push(Stash { index: i, sha: f[0].to_string(), message: f[1].to_string() });
        }
    }

    let wt = run(dir, &["worktree", "list", "--porcelain"]);
    let mut cur: Option<Worktree> = None;
    for line in wt.stdout.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(w) = cur.take() {
                d.worktrees.push(w);
            }
            cur = Some(Worktree { path: p.to_string(), branch: String::new() });
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            if let Some(w) = cur.as_mut() {
                w.branch = b.to_string();
            }
        } else if line == "detached" {
            if let Some(w) = cur.as_mut() {
                w.branch = "(detached)".into();
            }
        }
    }
    if let Some(w) = cur.take() {
        d.worktrees.push(w);
    }

    let sm = run(dir, &["config", "--file", ".gitmodules", "--get-regexp", "path"]);
    for line in sm.stdout.lines() {
        if let Some((_, p)) = line.split_once(' ') {
            d.submodules.push(p.to_string());
        }
    }

    d.remotes = run(dir, &["remote"]).stdout.lines().map(|s| s.to_string()).collect();
    d.status = status(dir);

    let gitdir = run(dir, &["rev-parse", "--git-dir"]).stdout.trim().to_string();
    let gd = dir.join(gitdir);
    if gd.join("MERGE_HEAD").exists() {
        d.state_note = Some("merging".into());
    } else if gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists() {
        d.state_note = Some("rebasing".into());
    } else if gd.join("CHERRY_PICK_HEAD").exists() {
        d.state_note = Some("cherry-picking".into());
    }
    d
}

fn parse_track(s: &str) -> (usize, usize) {
    let mut a = 0;
    let mut b = 0;
    let s = s.trim_matches(['[', ']']);
    for part in s.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            a = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            b = n.parse().unwrap_or(0);
        }
    }
    (a, b)
}

pub fn status(dir: &Path) -> WorkStatus {
    let mut w = WorkStatus::default();
    let o = run(dir, &["status", "--porcelain=v1", "-z", "-uall"]);
    let mut parts = o.stdout.split('\0').peekable();
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let bytes: Vec<char> = entry.chars().collect();
        let x = bytes[0];
        let y = bytes[1];
        let path: String = bytes[3..].iter().collect();
        if x == 'R' || x == 'C' {
            parts.next(); // original path
        }
        if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
            w.conflicted = true;
            w.unstaged.push(FileChange { path, status: 'U' });
            continue;
        }
        if x == '?' {
            w.unstaged.push(FileChange { path, status: '?' });
            continue;
        }
        if x != ' ' {
            w.staged.push(FileChange { path: path.clone(), status: x });
        }
        if y != ' ' {
            w.unstaged.push(FileChange { path, status: y });
        }
    }
    w
}

pub fn commit_files(dir: &Path, c: &Commit) -> Vec<FileChange> {
    let o = if let Some(p) = c.parents.first() {
        run(dir, &["diff", "--name-status", "-z", "-M", p, &c.sha])
    } else {
        run(dir, &["diff-tree", "--root", "--no-commit-id", "-r", "--name-status", "-z", "-M", &c.sha])
    };
    let mut out = Vec::new();
    let mut it = o.stdout.split('\0').filter(|s| !s.is_empty());
    while let Some(st) = it.next() {
        let ch = st.chars().next().unwrap_or('M');
        if ch == 'R' || ch == 'C' {
            let _old = it.next();
            if let Some(newp) = it.next() {
                out.push(FileChange { path: newp.to_string(), status: 'R' });
            }
        } else if let Some(p) = it.next() {
            out.push(FileChange { path: p.to_string(), status: ch });
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Hunk,
    Meta,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_no: Option<usize>,
    pub new_no: Option<usize>,
    pub text: String,
}

pub fn parse_diff(text: &str) -> Vec<DiffLine> {
    let mut out = Vec::new();
    let mut old_no = 0usize;
    let mut new_no = 0usize;
    let mut in_hunk = false;
    for line in text.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
            // @@ -a,b +c,d @@
            let mut it = line.split_whitespace().skip(1);
            if let Some(o) = it.next() {
                old_no = o.trim_start_matches('-').split(',').next().and_then(|s| s.parse().ok()).unwrap_or(1);
            }
            if let Some(n) = it.next() {
                new_no = n.trim_start_matches('+').split(',').next().and_then(|s| s.parse().ok()).unwrap_or(1);
            }
            out.push(DiffLine { kind: DiffLineKind::Hunk, old_no: None, new_no: None, text: line.to_string() });
            continue;
        }
        if !in_hunk {
            if line.starts_with("Binary files") {
                out.push(DiffLine { kind: DiffLineKind::Meta, old_no: None, new_no: None, text: line.to_string() });
            }
            continue;
        }
        if let Some(t) = line.strip_prefix('+') {
            out.push(DiffLine { kind: DiffLineKind::Added, old_no: None, new_no: Some(new_no), text: t.to_string() });
            new_no += 1;
        } else if let Some(t) = line.strip_prefix('-') {
            out.push(DiffLine { kind: DiffLineKind::Removed, old_no: Some(old_no), new_no: None, text: t.to_string() });
            old_no += 1;
        } else if line.starts_with('\\') {
            out.push(DiffLine { kind: DiffLineKind::Meta, old_no: None, new_no: None, text: line.to_string() });
        } else {
            let t = line.strip_prefix(' ').unwrap_or(line);
            out.push(DiffLine { kind: DiffLineKind::Context, old_no: Some(old_no), new_no: Some(new_no), text: t.to_string() });
            old_no += 1;
            new_no += 1;
        }
    }
    out
}

/// Where a diff is taken from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "sha")]
pub enum DiffSource {
    Unstaged,
    Staged,
    Commit(String),
}

pub fn file_diff(dir: &Path, src: &DiffSource, path: &str, untracked: bool, parent: Option<&str>) -> Vec<DiffLine> {
    let text = match src {
        DiffSource::Unstaged if untracked => {
            let content = std::fs::read(dir.join(path)).unwrap_or_default();
            if content.contains(&0) {
                return vec![DiffLine { kind: DiffLineKind::Meta, old_no: None, new_no: None, text: "Binary file".into() }];
            }
            let s = String::from_utf8_lossy(&content);
            let n = s.lines().count();
            let mut t = format!("@@ -0,0 +1,{n} @@\n");
            for l in s.lines() {
                t.push('+');
                t.push_str(l);
                t.push('\n');
            }
            t
        }
        DiffSource::Unstaged => run(dir, &["diff", "--no-color", "--", path]).stdout,
        DiffSource::Staged => run(dir, &["diff", "--cached", "--no-color", "--", path]).stdout,
        DiffSource::Commit(sha) => match parent {
            Some(p) => run(dir, &["diff", "--no-color", "-M", p, sha, "--", path]).stdout,
            None => run(dir, &["show", "--no-color", "--format=", sha, "--", path]).stdout,
        },
    };
    parse_diff(&text)
}

pub fn last_commit_message(dir: &Path) -> (String, String) {
    let o = run(dir, &["log", "-1", "--format=%s%x1f%b"]);
    let mut it = o.stdout.splitn(2, '\x1f');
    let s = it.next().unwrap_or("").trim().to_string();
    let b = it.next().unwrap_or("").trim().to_string();
    (s, b)
}

pub fn repo_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}
