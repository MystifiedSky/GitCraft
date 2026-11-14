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
    pub locked: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RepoData {
    #[serde(skip)]
    pub history_revs: Vec<String>,
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
    /// More commits exist beyond the ones loaded (lazy loading in the graph).
    pub has_more: bool,
    /// Message git prepared for a merge commit (MERGE_MSG), to prefill the commit box.
    pub merge_msg: Option<String>,
    /// "current" and "incoming" names while resolving conflicts.
    pub conflict_labels: Option<(String, String)>,
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
    pub fn failed(why: &str) -> Self {
        GitOut { ok: false, stdout: String::new(), stderr: why.to_string() }
    }

    pub fn message(&self) -> String {
        let s = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        s.trim().to_string()
    }
}

/// Sign-ins for HTTPS remotes: (host, user name, token). Filled from the services connected in Accounts
/// (GitHub / GitLab tokens in the system's credential store).
static HOST_TOKENS: std::sync::RwLock<Vec<(String, String, String)>> = std::sync::RwLock::new(Vec::new());

pub fn set_host_tokens(tokens: Vec<(String, String, String)>) {
    if let Ok(mut t) = HOST_TOKENS.write() {
        *t = tokens;
    }
}

/// Config (through GIT_CONFIG_COUNT, so nothing shows in the command line or the activity log) that makes Git
/// use a connected service's token for https://<host>. The token itself travels in an environment variable.
pub fn credential_env(tokens: &[(String, String, String)]) -> Vec<(String, String)> {
    let mut env = Vec::new();
    let mut n = 0;
    for (i, (host, user, token)) in tokens.iter().enumerate() {
        let key = format!("credential.https://{host}.helper");
        // An empty helper first clears any other helper for this host, so Git doesn't ask twice.
        env.push((format!("GIT_CONFIG_KEY_{n}"), key.clone()));
        env.push((format!("GIT_CONFIG_VALUE_{n}"), String::new()));
        n += 1;
        env.push((format!("GIT_CONFIG_KEY_{n}"), key));
        env.push((
            format!("GIT_CONFIG_VALUE_{n}"),
            format!("!f() {{ test \"$1\" = get || exit 0; echo username={user}; echo \"password=$GITCRAFT_TOKEN_{i}\"; }}; f"),
        ));
        n += 1;
        env.push((format!("GITCRAFT_TOKEN_{i}"), token.clone()));
    }
    if n > 0 {
        env.push(("GIT_CONFIG_COUNT".into(), n.to_string()));
    }
    env
}

pub fn git_cmd(dir: &Path) -> Command {
    // Preferences > Experimental > Git Executable, and SSH > private key (when the SSH agent isn't used).
    let rt = crate::prefs::runtime();
    let mut c = Command::new(crate::prefs::git_program());
    c.current_dir(dir);
    if let Some(ssh) = crate::prefs::ssh_command(&rt) {
        c.env("GIT_SSH_COMMAND", ssh);
    }
    if let Ok(t) = HOST_TOKENS.read() {
        if !t.is_empty() {
            c.envs(credential_env(&t));
        }
    }
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

/// Which branches and tags the graph shows: hidden refs are left out; when any ref is
/// soloed only the soloed refs are shown (full names such as "refs/heads/main").
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RefView {
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub solo: Vec<String>,
    /// "Pin to Left": full ref name whose line is drawn in lane 0.
    #[serde(default)]
    pub pinned: Option<String>,
    /// UI Customization > Branch visibility: Smart Branch Visibility.
    #[serde(default)]
    pub smart: bool,
}

/// Smart Branch Visibility: the checked-out branch, its upstream and the branch it will merge into
/// (the remote's default branch).
pub fn smart_refs(dir: &Path) -> Vec<String> {
    let mut v = vec!["HEAD".to_string()];
    let head = run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_string();
    if !head.is_empty() {
        let up = run(dir, &["for-each-ref", "--format=%(upstream)", &head]).stdout.trim().to_string();
        v.push(head);
        if !up.is_empty() {
            v.push(up);
        }
    }
    if let Some(t) = crate::extras::default_target(dir) {
        v.push(t);
    }
    v.dedup();
    v
}

/// Refs under refs/heads, refs/remotes and refs/tags that lead to a commit: `for-each-ref` skips the ones it can't
/// read (missing or invalid), and a tag of a tree or blob is left out by its peeled type, so the list holds only what
/// the history view keeps (see load_view).
pub fn walkable_refs(dir: &Path) -> Vec<String> {
    use std::io::Write;
    // Names only: asking for object types makes for-each-ref stop at a ref whose object is missing.
    let names: Vec<String> = run(dir, &["for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes", "refs/tags"])
        .stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if names.is_empty() {
        return names;
    }
    // One answer per ref: "<sha>" when it peels to a commit, "<input> missing" when it doesn't (or is broken).
    let Ok(mut child) = git_cmd(dir)
        .args(["cat-file", "--batch-check=%(objectname)"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let input: String = names.iter().map(|n| format!("{n}^{{commit}}\n")).collect();
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let Ok(out) = child.wait_with_output() else { return Vec::new() };
    let answers = String::from_utf8_lossy(&out.stdout).into_owned();
    names.into_iter().zip(answers.lines()).filter(|(_, a)| !a.ends_with(" missing") && !a.trim().is_empty()).map(|(n, _)| n).collect()
}

/// Leaves the remotes' HEAD pointers (origin/HEAD...) out of `git log --remotes`.
pub const REMOTE_HEAD_EXCLUDE: &str = "--exclude=*/HEAD";

/// Arguments for `git log` that select the commits the graph should show.
pub fn log_revs(view: &RefView) -> Vec<String> {
    if !view.solo.is_empty() {
        return view.solo.clone();
    }
    let mut out = Vec::new();
    let groups = [("refs/heads/", "--branches"), ("refs/remotes/", "--remotes"), ("refs/tags/", "--tags")];
    for (prefix, flag) in groups {
        for h in &view.hidden {
            if let Some(short) = h.strip_prefix(prefix) {
                out.push(format!("--exclude={short}"));
            }
        }
        // A remote's HEAD only points at one of its branches (already listed, and never shown as a label). When it
        // points nowhere (an empty or stale refs/remotes/origin/HEAD), `git log --remotes` stops with
        // "fatal: bad object origin/HEAD" and the graph came up empty, so leave it out.
        if flag == "--remotes" {
            out.push(REMOTE_HEAD_EXCLUDE.to_string());
        }
        out.push(flag.to_string());
    }
    out.push("HEAD".into());
    out
}

#[allow(dead_code)] // used by tests and kept as the simple entry point
pub fn load(dir: &Path, max_commits: usize) -> RepoData {
    load_view(dir, max_commits, &RefView::default())
}

pub fn load_view(dir: &Path, max_commits: usize, view: &RefView) -> RepoData {
    let smart_view;
    let view = if view.smart && view.solo.is_empty() {
        smart_view = RefView { solo: smart_refs(dir), ..view.clone() };
        &smart_view
    } else {
        view
    };
    let mut d = RepoData::default();

    // History, newest first, every branch.
    // One more than asked for tells us whether there is more history to load.
    let n = format!("-n{}", max_commits + 1);
    let requested_revs = log_revs(view);
    let mut resolve_args = vec!["rev-parse", "--revs-only"];
    resolve_args.extend(requested_revs.iter().map(String::as_str));
    let resolved = run(dir, &resolve_args);
    let revs: Vec<String> = if resolved.ok {
        resolved.stdout.lines().map(str::to_owned).collect()
    } else { requested_revs };
    d.history_revs = revs.clone();
    let mut log_args: Vec<&str> = vec!["log"];
    log_args.extend(revs.iter().map(|s| s.as_str()));
    log_args.extend(["--date-order", n.as_str(), "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1e", "--"]);

    // The ten reads don't depend on each other, and on Windows each Git process costs 20-50 ms just to start,
    // so they run at the same time instead of one after another.
    let (refs, head, head_sha, log, st, wt, sm, remotes, work, gitdir) = std::thread::scope(|s| {
        // Refs (branches, remote branches, tags) with ahead/behind info.
        let refs = s.spawn(|| {
            run(
                dir,
                &[
                    "for-each-ref",
                    "--format=%(refname)%1f%(objectname)%1f%(*objectname)%1f%(upstream:short)%1f%(upstream:track)",
                    "refs/heads",
                    "refs/remotes",
                    "refs/tags",
                ],
            )
        });
        let head = s.spawn(|| run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]));
        let head_sha = s.spawn(|| run(dir, &["rev-parse", "HEAD"]));
        let log = s.spawn(|| if resolved.ok { history_log(dir, &revs, 0, max_commits) } else { run(dir, &log_args) });
        let st = s.spawn(|| run(dir, &["stash", "list", "--format=%H%x1f%gs"]));
        let wt = s.spawn(|| run(dir, &["worktree", "list", "--porcelain"]));
        let sm = s.spawn(|| run(dir, &["config", "--file", ".gitmodules", "--get-regexp", "path"]));
        let remotes = s.spawn(|| run(dir, &["remote"]));
        let work = s.spawn(|| status(dir));
        let gitdir = s.spawn(|| run(dir, &["rev-parse", "--git-dir"]));
        let j = |h: std::thread::ScopedJoinHandle<'_, GitOut>| h.join().unwrap_or_else(|_| GitOut::failed("a Git read stopped unexpectedly"));
        (j(refs), j(head), j(head_sha), j(log), j(st), j(wt), j(sm), j(remotes), work.join().unwrap_or_default(), j(gitdir))
    });
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

    if head.ok {
        d.head_branch = Some(head.stdout.trim().to_string());
    }
    d.head_sha = head_sha.stdout.trim().to_string();

    // Another broken ref (a branch file that is empty or names a missing commit) still stops the whole log. Then read
    // the history again from only the refs `for-each-ref` could read (it skips broken ones with a warning), so one
    // damaged ref never hides the rest of the history. Hidden refs stay hidden.
    // Rule: a ref reached through a
    // pattern is skipped when it is missing, has an invalid name, or doesn't lead to a commit;
    // every other ref is walked.
    let log = if log.ok || !view.solo.is_empty() {
        log
    } else {
        let mut start: Vec<String> = walkable_refs(dir).into_iter().filter(|r| !view.hidden.contains(r)).collect();
        // Windows limits a command line to 32767 characters; with that many refs keep the branches and HEAD.
        if start.iter().map(|s| s.len() + 3).sum::<usize>() > 24_000 {
            start.retain(|r| r.starts_with("refs/heads/"));
        }
        if head_sha.ok {
            start.push("HEAD".into());
        }
        if start.is_empty() {
            log
        } else {
            let mut args: Vec<&str> = vec!["log"];
            args.extend(start.iter().map(|s| s.as_str()));
            args.extend(["--date-order", n.as_str(), "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1e", "--"]);
            let result = run(dir, &args);
            let mut resolve = vec!["rev-parse", "--revs-only"];
            resolve.extend(start.iter().map(String::as_str));
            d.history_revs = run(dir, &resolve).stdout.lines().map(str::to_owned).collect();
            result
        }
    };

    d.commits = parse_commits(&log.stdout);
    if d.commits.len() > max_commits {
        d.commits.truncate(max_commits);
        d.has_more = true;
    }
    for (i, c) in d.commits.iter().enumerate() {
        d.index_of.insert(c.sha.clone(), i);
    }

    for (i, line) in st.stdout.lines().enumerate() {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() == 2 {
            d.stashes.push(Stash { index: i, sha: f[0].to_string(), message: f[1].to_string() });
        }
    }

    let mut cur: Option<Worktree> = None;
    for line in wt.stdout.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(w) = cur.take() {
                d.worktrees.push(w);
            }
            cur = Some(Worktree { path: p.to_string(), branch: String::new(), locked: false });
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            if let Some(w) = cur.as_mut() {
                w.branch = b.to_string();
            }
        } else if line == "locked" || line.starts_with("locked ") {
            if let Some(w) = cur.as_mut() {
                w.locked = true;
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

    for line in sm.stdout.lines() {
        if let Some((_, p)) = line.split_once(' ') {
            d.submodules.push(p.to_string());
        }
    }

    d.remotes = remotes.stdout.lines().map(|s| s.to_string()).collect();
    d.status = work;

    let gitdir = gitdir.stdout.trim().to_string();
    let gd = dir.join(gitdir);
    if gd.join("MERGE_HEAD").exists() {
        d.state_note = Some("merging".into());
        d.merge_msg = crate::rewrite::merge_message(dir);
    } else if gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists() {
        d.state_note = Some("rebasing".into());
    } else if gd.join("CHERRY_PICK_HEAD").exists() {
        d.state_note = Some("cherry-picking".into());
    }
    if d.state_note.is_some() {
        d.conflict_labels = Some(crate::rewrite::conflict_labels(dir));
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

/// Shared status rules: recurse into untracked folders and use a 50% rename similarity.
pub fn status_options() -> [String; 2] {
    ["-uall".into(), format!("--find-renames={}%", crate::metrics::STATUS_RENAME_THRESHOLD)]
}

pub fn status(dir: &Path) -> WorkStatus {
    let mut w = WorkStatus::default();
    // --no-optional-locks: don't rewrite the index while reading it, so a status check never
    // wakes the .git watcher and starts another refresh.
    // Status lists untracked files, recurses into untracked folders, detects renames between HEAD and the index and
    // between the index and the working folder, and shows both the index and the working folder.
    // -uall = untracked files inside untracked folders; --find-renames=50% = rename detection at a fixed
    // similarity, whatever status.renames says in the person's Git config. Reading must not write the index back,
    // hence --no-optional-locks.
    let options = status_options();
    let o = run(dir, &["--no-optional-locks", "status", "--porcelain=v1", "-z", &options[0], &options[1]]);
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
    /// Between two points: commits, or a commit and "WORKDIR" (the working folder).
    Range(String, String),
}

pub const WORKDIR: &str = "WORKDIR";

#[allow(dead_code)] // used by tests
pub fn file_diff(dir: &Path, src: &DiffSource, path: &str, untracked: bool, parent: Option<&str>) -> Vec<DiffLine> {
    file_diff_opts(dir, src, path, untracked, parent, false)
}

/// Raw unified diff text for a file (used for the hunk view and for building partial patches).
pub fn diff_text(dir: &Path, src: &DiffSource, path: &str, parent: Option<&str>, ignore_ws: bool) -> String {
    // Diff options: 3 context lines, hunks never merged.
    let ctx = format!("-U{}", crate::metrics::DIFF_CONTEXT_LINES);
    let inter = format!("--inter-hunk-context={}", crate::metrics::DIFF_INTERHUNK_LINES);
    let mut args: Vec<&str> = vec!["diff", "--no-color", "--no-ext-diff", ctx.as_str(), inter.as_str()];
    if ignore_ws {
        // Ignore all whitespace (git -w).
        args.push("--ignore-all-space");
    }
    let range;
    match src {
        DiffSource::Unstaged => {}
        DiffSource::Staged => args.push("--cached"),
        DiffSource::Commit(sha) => match parent {
            Some(p) => {
                args.push("-M");
                args.push(p);
                args.push(sha);
            }
            None => {
                let mut a: Vec<&str> = vec!["show", "--no-color", "--format=", ctx.as_str(), inter.as_str()];
                if ignore_ws {
                    a.push("--ignore-all-space");
                }
                a.extend([sha.as_str(), "--", path]);
                return run(dir, &a).stdout;
            }
        },
        DiffSource::Range(from, to) => {
            range = (from.clone(), to.clone());
            args.push(range.0.as_str());
            if range.1 != WORKDIR {
                args.push(range.1.as_str());
            }
        }
    }
    args.push("--");
    args.push(path);
    run(dir, &args).stdout
}

pub fn file_diff_opts(dir: &Path, src: &DiffSource, path: &str, untracked: bool, parent: Option<&str>, ignore_ws: bool) -> Vec<DiffLine> {
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
        _ => diff_text(dir, src, path, parent, ignore_ws),
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

/// Parse both initial history and subsequent pages identically.
pub fn parse_commits(text: &str) -> Vec<Commit> {
    let mut commits = Vec::new();
    for rec in text.split('\x1e') {
        let rec = rec.trim_start_matches(['\n', '\r']);
        if rec.is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.splitn(7, '\x1f').collect();
        if f.len() < 7 {
            continue;
        }
        commits.push(Commit {
            sha: f[0].to_string(),
            parents: f[1].split_whitespace().map(|s| s.to_string()).collect(),
            author: f[2].to_string(),
            email: f[3].to_string(),
            time: f[4].parse().unwrap_or(0),
            summary: f[5].to_string(),
            body: f[6].trim().to_string(),
        });
    }
    commits
}

fn history_log(dir: &Path, revs: &[String], offset: usize, max: usize) -> GitOut {
    use std::io::Write;
    // Stdin avoids the Windows command-line limit for repositories with many branch/tag tips.
    let child = git_cmd(dir).args(["log", "--stdin", "--date-order", &format!("--skip={offset}"), &format!("-n{}", max + 1),
        "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1e", "--"])
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn();
    let mut child = match child { Ok(child) => child, Err(e) => return GitOut::failed(&e.to_string()) };
    let input = revs.join("\n") + "\n";
    let writer = child.stdin.take().map(|mut stdin| std::thread::spawn(move || stdin.write_all(input.as_bytes())));
    let output = child.wait_with_output();
    if let Some(writer) = writer { let _ = writer.join(); }
    match output {
        Ok(output) => GitOut { ok: output.status.success(), stdout: String::from_utf8_lossy(&output.stdout).into_owned(), stderr: String::from_utf8_lossy(&output.stderr).into_owned() },
        Err(e) => GitOut::failed(&e.to_string()),
    }
}

pub fn history_page(dir: &Path, revs: &[String], offset: usize, max: usize) -> Result<(Vec<Commit>, bool), String> {
    if revs.is_empty() || revs.iter().any(|r| !matches!(r.len(), 40 | 64) || !r.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err("History changed; reload the repository to continue.".into());
    }
    let output = history_log(dir, revs, offset, max);
    if !output.ok { return Err(output.message()); }
    let mut commits = parse_commits(&output.stdout);
    let has_more = commits.len() > max;
    commits.truncate(max);
    Ok((commits, has_more))
}

#[cfg(test)]
mod paging_tests {
    use super::*;
    #[test]
    fn history_pages_keep_their_snapshot_when_a_branch_moves() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        assert!(run(dir, &["init", "-b", "main"]).ok);
        assert!(run(dir, &["config", "user.name", "Test"]).ok);
        assert!(run(dir, &["config", "user.email", "test@example.com"]).ok);
        for name in ["one", "two", "three", "four"] {
            assert!(run(dir, &["commit", "--allow-empty", "-m", name]).ok);
        }
        let original = load_view(dir, 2, &RefView::default());
        assert!(original.has_more);
        let all = load_view(dir, 20, &RefView::default());
        assert!(run(dir, &["commit", "--allow-empty", "-m", "new tip"]).ok);
        let (page, more) = history_page(dir, &original.history_revs, 2, 2).unwrap();
        assert!(!more);
        let many_tips = vec![original.history_revs[0].clone(); 1000];
        let (many, _) = history_page(dir, &many_tips, 0, 2).unwrap();
        assert_eq!(many[0].sha, original.commits[0].sha);
        let actual: Vec<_> = original.commits.iter().chain(page.iter()).map(|c| &c.sha).collect();
        assert_eq!(actual, all.commits.iter().map(|c| &c.sha).collect::<Vec<_>>());
        assert!(history_page(dir, &["--all".into()], 0, 2).is_err());
    }
}
