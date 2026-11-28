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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Stash {
    pub index: usize,
    pub sha: String,
    pub message: String,
    /// The commit the stash was made on (its first parent).
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub time: i64,
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
    pub prunable: bool,
}

fn parse_worktrees(output: &str) -> Vec<Worktree> {
    output.split("\0\0").filter_map(|record| {
        let mut fields = record.split('\0');
        let path = fields.next()?.strip_prefix("worktree ")?.to_owned();
        let mut worktree = Worktree { path, branch: String::new(), locked: false, prunable: false };
        for field in fields {
            if let Some(branch) = field.strip_prefix("branch refs/heads/") {
                worktree.branch = branch.to_owned();
            } else if field == "detached" {
                worktree.branch = "(detached)".into();
            } else if field == "locked" || field.starts_with("locked ") {
                worktree.locked = true;
            } else if field == "prunable" || field.starts_with("prunable ") {
                worktree.prunable = true;
            }
        }
        Some(worktree)
    }).collect()
}

pub(crate) fn try_worktrees(dir: &Path) -> Result<Vec<Worktree>, String> {
    let out = run(dir, &["worktree", "list", "--porcelain", "-z"]);
    if !out.ok { return Err(out.message()); }
    Ok(parse_worktrees(&out.stdout))
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RepoData {
    #[serde(skip)]
    pub history_revs: Vec<String>,
    /// The history was read in `--date-order` (see [`plain_order_holds`]); later pages are too.
    #[serde(skip)]
    pub history_date_order: bool,
    pub commits: Vec<Commit>,
    pub refs: Vec<GitRef>,
    pub stashes: Vec<Stash>,
    pub worktrees: Vec<Worktree>,
    pub submodules: Vec<String>,
    pub remotes: Vec<String>,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub status: WorkStatus,
    pub bisect: Option<BisectProgress>,
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

#[derive(Clone, Debug, Serialize)]
pub struct BisectProgress {
    pub tested: usize,
    pub first_bad: Option<String>,
}

pub fn bisect_progress(gitdir: &Path) -> Option<BisectProgress> {
    use std::io::Read;
    if !gitdir.join("BISECT_START").is_file() { return None; }
    let mut text = String::new();
    if let Ok(log) = std::fs::File::open(gitdir.join("BISECT_LOG")) {
        let _ = log.take(256 * 1024).read_to_string(&mut text);
    }
    let tested = text.lines().filter(|line| {
        ["git bisect good ", "git bisect bad ", "git bisect skip "].iter().any(|prefix| line.starts_with(prefix))
    }).count();
    let first_bad = bisect_first_bad(&text).map(str::to_owned);
    Some(BisectProgress { tested, first_bad })
}

fn bisect_first_bad(log: &str) -> Option<&str> {
    log.lines().rev().find_map(|line| {
        let entry = line.strip_prefix("# first bad commit: [")
            .or_else(|| line.strip_prefix("# first 'bad' commit: ["))?;
        let (sha, _) = entry.split_once(']')?;
        (matches!(sha.len(), 40 | 64) && sha.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(sha)
    })
}

#[cfg(test)]
#[test]
fn bisect_recognizes_completion_in_old_and_new_git_logs() {
    let sha1 = "a".repeat(40);
    let sha256 = "b".repeat(64);
    assert_eq!(bisect_first_bad(&format!("# first bad commit: [{sha1}] Subject\n")), Some(sha1.as_str()));
    assert_eq!(bisect_first_bad(&format!("# first 'bad' commit: [{sha256}] Subject\r\n")), Some(sha256.as_str()));
    assert_eq!(bisect_first_bad(&format!("# possible first 'bad' commit: [{sha1}] Subject\n")), None);
    assert_eq!(bisect_first_bad("# first 'bad' commit: [123] Incomplete\n"), None);
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

/// Remove credentials before diagnostics leave the Git boundary or enter activity storage.
pub fn redact_diagnostic(text: &str) -> String {
    let mut value=text.to_string();let mut cursor=0;
    while let Some(offset)=value[cursor..].find("://") {
        let start=cursor+offset+3;
        let end=value[start..].find(|character:char|character.is_whitespace() || matches!(character,'/'|'?'|'#')).map(|offset|start+offset).unwrap_or(value.len());
        if let Some(at)=value[start..end].rfind('@') {
            value.replace_range(start..start+at,"[redacted]");cursor=start+"[redacted]@".len();
        } else {cursor=end;}
    }
    for key in ["access_token","private_token","oauth_token","token","password","api_key"] {
        for prefix in ['?','&'] {
            let pattern=format!("{prefix}{key}=");let mut cursor=0;
            loop {
                let lower=value.to_ascii_lowercase();let Some(offset)=lower[cursor..].find(&pattern) else {break};let start=cursor+offset+pattern.len();
                let end=value[start..].find(|character:char|character.is_whitespace() || matches!(character,'&'|'#')).map(|offset|start+offset).unwrap_or(value.len());
                value.replace_range(start..end,"[redacted]");cursor=start+"[redacted]".len();
            }
        }
    }
    if let Some(start)=value.to_ascii_lowercase().find("http.extraheader") {value.truncate(start);value.push_str("http.extraheader=[redacted]");}
    for marker in ["authorization:","cookie:","x-auth-token:","x-api-key:"] {
        let mut cursor=0;
        loop {
            let lower=value.to_ascii_lowercase();let Some(offset)=lower[cursor..].find(marker) else {break};let start=cursor+offset+marker.len();
            let end=value[start..].find('\n').map(|offset|start+offset).unwrap_or(value.len());value.replace_range(start..end," [redacted]");cursor=start+" [redacted]".len();
        }
    }
    if let Ok(tokens)=HOST_TOKENS.read() {
        for (_,_,token) in tokens.iter().filter(|(_,_,token)|!token.is_empty()) {value=value.replace(token,"[redacted]");}
    }
    value
}

impl GitOut {
    pub fn failed(why: &str) -> Self {
        GitOut { ok: false, stdout: String::new(), stderr: why.to_string() }
    }

    pub fn message(&self) -> String {
        let s = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        redact_diagnostic(s.trim())
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
/// use a connected service's credentials for https://<host>. Usernames and tokens travel as environment data.
pub fn credential_env(tokens: &[(String, String, String)]) -> Vec<(String, String)> {
    let mut env = Vec::new();
    let mut n = 0;
    for (i, (host, user, token)) in tokens.iter().enumerate() {
        if user.contains(['\r', '\n']) || token.contains(['\r', '\n']) { continue; }
        let key = format!("credential.https://{host}.helper");
        // An empty helper first clears any other helper for this host, so Git doesn't ask twice.
        env.push((format!("GIT_CONFIG_KEY_{n}"), key.clone()));
        env.push((format!("GIT_CONFIG_VALUE_{n}"), String::new()));
        n += 1;
        env.push((format!("GIT_CONFIG_KEY_{n}"), key));
        env.push((
            format!("GIT_CONFIG_VALUE_{n}"),
            format!("!f() {{ test \"$1\" = get || exit 0; printf 'username=%s\\npassword=%s\\n' \"$GITCRAFT_USER_{i}\" \"$GITCRAFT_TOKEN_{i}\"; }}; f"),
        ));
        n += 1;
        env.push((format!("GITCRAFT_USER_{i}"), user.clone()));
        env.push((format!("GITCRAFT_TOKEN_{i}"), token.clone()));
    }
    if n > 0 {
        env.push(("GIT_CONFIG_COUNT".into(), n.to_string()));
    }
    env
}

pub fn check_version(program: &str) -> Result<(), String> {
    let program = if program.trim().is_empty() { "git" } else { program.trim() };
    let mut command = Command::new(program);
    command.arg("--version");
    english_diagnostics(&mut command);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let output = crate::read_jobs::run_bounded(command, None, std::time::Duration::from_secs(5), "Git version check timed out.");
    const REQUIREMENT: &str = "GitCraft requires Git 2.38 or newer.";
    if !output.ok {
        return Err(format!("{REQUIREMENT} Git could not start. Check Preferences > Experimental > Git Executable."));
    }
    let version = output.stdout.trim().strip_prefix("git version ").unwrap_or_default();
    let mut parts = version.split('.');
    let numbers = parts.next().and_then(|major| major.parse::<u32>().ok())
        .zip(parts.next().and_then(|minor| minor.parse::<u32>().ok()));
    match numbers {
        Some(installed) if installed >= (2, 38) => Ok(()),
        Some(_) => Err(format!("{REQUIREMENT} The configured executable reports Git {version}.")),
        None => Err(format!("{REQUIREMENT} The configured executable did not report a Git version.")),
    }
}

pub fn git_cmd(dir: &Path) -> Command {
    // Preferences > Experimental > Git Executable, and SSH > private key (when the SSH agent isn't used).
    let rt = crate::prefs::runtime();
    let mut c = Command::new(crate::prefs::git_program());
    c.current_dir(dir);
    if let Some(ssh) = crate::prefs::ssh_command(&rt) {
        c.env("GIT_SSH_COMMAND", ssh);
    }
    english_diagnostics(&mut c);
    c.env("GIT_TERMINAL_PROMPT", "0");
    c.env("GIT_EDITOR", "true");
    c.env("GIT_SEQUENCE_EDITOR", "true");
    #[cfg(windows)]
    {
        // Windows Git cannot see Linux permission bits through \\wsl$ shares. A repository created
        // inside WSL keeps core.fileMode=true, so host Git would otherwise stage every executable
        // script as a mode change to 100644. Keep the modes already recorded in the index.
        if wsl_location(dir).is_some() { c.args(["-c", "core.fileMode=false"]); }
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

pub(crate) fn english_diagnostics(command: &mut Command) {
    if let Some(locale) = std::env::var_os("LC_ALL").filter(|locale| !locale.is_empty()) {
        command.env_remove("LC_ALL").env("LC_CTYPE", locale);
    }
    command.env("LC_MESSAGES", "C").env("LANGUAGE", "C");
}

fn command_args<'a>(mut args: &'a [&'a str]) -> &'a [&'a str] {
    while let Some((first, rest)) = args.split_first() {
        if !first.starts_with('-') { break; }
        args = if matches!(*first, "-c" | "-C" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env") {
            rest.get(1..).unwrap_or_default()
        } else { rest };
    }
    args
}

fn uses_network_credentials(args: &[&str]) -> bool {
    matches!(command_args(args),
        ["fetch" | "pull" | "push" | "clone" | "ls-remote", ..]
            | ["submodule", "update" | "add", ..]
            | ["lfs", "fetch" | "pull" | "push" | "lock" | "unlock" | "locks", ..])
}

#[cfg(windows)]
pub(crate) fn wsl_location(dir: &Path) -> Option<(&str, String)> {
    use std::path::{Component, Prefix};
    let mut components = dir.components();
    let Component::Prefix(prefix) = components.next()? else { return None; };
    let (server, distro) = match prefix.kind() {
        Prefix::UNC(server, distro) | Prefix::VerbatimUNC(server, distro) => (server.to_str()?, distro.to_str()?),
        _ => return None,
    };
    if !(server.eq_ignore_ascii_case("wsl$") || server.eq_ignore_ascii_case("wsl.localhost")) ||
        distro.is_empty() || distro.starts_with('-') || distro.chars().any(char::is_control) {
        return None;
    }
    let mut linux = String::new();
    for component in components {
        if let Component::Normal(part) = component {
            linux.push('/');
            linux.push_str(part.to_str()?);
        } else if component != Component::RootDir { return None; }
    }
    if linux.is_empty() { linux.push('/'); }
    Some((distro, linux))
}

#[cfg(windows)]
fn wsl_read_command(dir: &Path, args: &[&str]) -> Option<Command> {
    let operation = command_args(args).first().copied()?;
    // These reads use Git objects or repo-relative paths. Operations that pass
    // Windows paths, editor programs or credentials use the configured host Git.
    if !matches!(operation, "status" | "log" | "show" | "diff" | "ls-files" | "for-each-ref" | "cat-file" | "rev-list" | "blame" | "check-ignore") ||
        args.iter().any(|arg| Path::new(arg).is_absolute() || arg.contains(":/") || arg.contains(":\\")) {
        return None;
    }
    let (distro, linux) = wsl_location(dir)?;
    let system = std::env::var_os("SystemRoot")?;
    let mut command = Command::new(Path::new(&system).join("System32/wsl.exe"));
    command.args(["--distribution", distro, "--cd", &linux, "--exec", "env", "LC_MESSAGES=C", "LANGUAGE=C",
        "GIT_TERMINAL_PROMPT=0", "git", "--no-optional-locks"]).args(args).creation_flags(0x0800_0000);
    Some(command)
}

pub(crate) fn git_command(dir: &Path, args: &[&str]) -> Command {
    #[cfg(windows)]
    if let Some(command) = wsl_read_command(dir, args) { return command; }
    let mut command = git_cmd(dir);
    command.args(args);
    if uses_network_credentials(args) {
        let tokens = HOST_TOKENS.read().unwrap_or_else(|error| error.into_inner());
        command.envs(credential_env(&tokens));
    }
    command
}

pub fn run(dir: &Path, args: &[&str]) -> GitOut {
    let verb = command_args(args).first().copied().unwrap_or("");
    if crate::read_jobs::has_context() || matches!(verb, "log" | "ls-remote" | "blame" | "status" | "show" | "diff" | "ls-files" | "cat-file" | "rev-list" | "for-each-ref") {
        return run_limited(dir, args, 64 * 1024 * 1024);
    }
    crate::read_jobs::run_mutation(git_command(dir, args), None)
}

pub fn run_limited(dir: &Path, args: &[&str], limit: usize) -> GitOut {
    crate::read_jobs::run(git_command(dir, args), limit, None)
}

pub fn run_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> GitOut {
    #[cfg(windows)]
    let mut command = if wsl_location(dir).is_some() && !env.is_empty() {
        let mut command = git_cmd(dir);
        command.args(args);
        command
    } else { git_command(dir, args) };
    #[cfg(not(windows))]
    let mut command = git_command(dir, args);
    command.envs(env.iter().copied());
    crate::read_jobs::run_mutation(command, None)
}

pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    let o = run(dir, &["rev-parse", "--show-toplevel"]);
    if o.ok {
        Some(PathBuf::from(o.stdout.trim()))
    } else {
        None
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct HistoryFilter {
    pub author: Option<String>,
    pub path: Option<String>,
    pub text: Option<String>,
    pub mode: Option<String>,
}

impl HistoryFilter {
    /// Whether the history is narrowed by author, path or text.
    pub fn is_active(&self) -> bool {
        [&self.author, &self.path, &self.text].iter().any(|v| v.as_deref().is_some_and(|v| !v.is_empty()))
    }

    fn arguments(&self) -> Result<(Vec<String>, Option<&str>), String> {
        let mut options = Vec::new();
        if let Some(author) = self.author.as_deref().filter(|value| !value.is_empty()) {
            options.extend(["--use-mailmap".into(), "--fixed-strings".into(), "--regexp-ignore-case".into(), format!("--author={author}")]);
        }
        if let Some(text) = self.text.as_deref().filter(|value| !value.is_empty()) {
            let option = match self.mode.as_deref() {
                Some("regex") => "-G",
                None | Some("text") => "-S",
                _ => return Err("Choose a supported content search mode.".into()),
            };
            options.push(format!("{option}{text}"));
            options.push("--no-textconv".into());
        }
        let path = self.path.as_deref().filter(|value| !value.is_empty());
        if let Some(path) = path {
            if Path::new(path).is_absolute() || path.split('/').any(|component| component == "..")
                || path.contains(['\\', '\0']) || path.as_bytes().get(1) == Some(&b':') {
                return Err("Enter a repository-relative path using forward slashes.".into());
            }
        }
        if self.author.iter().chain(self.text.iter()).any(|value| value.contains('\0') || value.len() > 4096) {
            return Err("History filters must be at most 4,096 bytes and cannot contain NUL characters.".into());
        }
        Ok((options, path))
    }
}

/// Which branches and tags the graph shows: hidden refs are left out; when any ref is
/// soloed only the soloed refs are shown (full names such as "refs/heads/main").
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub filter: HistoryFilter,
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
    let input: String = names.iter().map(|n| format!("{n}^{{commit}}\n")).collect();
    let command = git_command(dir, &["cat-file", "--batch-check=%(objectname)"]);
    let out = crate::read_jobs::run_bytes(command, 64 * 1024 * 1024, Some(input.into_bytes()));
    if !out.ok { return Vec::new(); }
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
    load_view_checked(dir, max_commits, view).unwrap_or_default()
}

pub fn load_view_checked(dir: &Path, max_commits: usize, view: &RefView) -> Result<RepoData, String> {
    let smart_view;
    let view = if view.smart && view.solo.is_empty() {
        smart_view = RefView { solo: smart_refs(dir), ..view.clone() };
        &smart_view
    } else {
        view
    };
    let (filters, path) = view.filter.arguments()?;
    let mut d = RepoData::default();

    // History, newest first, every branch.
    // One more than asked for tells us whether there is more history to load.
    let n = format!("-n{}", max_commits + 1);
    let requested_revs = log_revs(view);

    // The ten reads don't depend on each other, and on Windows each Git process costs 20-50 ms just to start,
    // so they run at the same time instead of one after another. The history (resolving its
    // branches first) and the status take longest, so they start first.
    let ((revs, log, mut date_order), refs, head, head_sha, st, wt, sm, remotes, work, gitdir) = std::thread::scope(|s| {
        let log = s.spawn(crate::read_jobs::inherit(|| {
            let mut resolve_args = vec!["rev-parse", "--revs-only"];
            resolve_args.extend(requested_revs.iter().map(String::as_str));
            let resolved = run(dir, &resolve_args);
            if resolved.ok {
                let revs: Vec<String> = resolved.stdout.lines().map(str::to_owned).collect();
                // Git's plain order, which doesn't have to walk the whole history first, unless
                // it put a parent before its child here (or did before in this repository).
                if !needs_date_order(dir) {
                    let log = history_log(dir, &revs, 0, max_commits, &filters, path, false);
                    let mut seen = std::collections::HashSet::new();
                    if !log.ok || plain_order_holds(&parse_commits(&log.stdout), &mut seen) {
                        return (revs, log, false);
                    }
                    mark_date_order(dir);
                }
                let log = history_log(dir, &revs, 0, max_commits, &filters, path, true);
                (revs, log, true)
            } else {
                let mut log_args: Vec<&str> = vec!["--literal-pathspecs", "log"];
                log_args.extend(filters.iter().map(String::as_str));
                log_args.extend(requested_revs.iter().map(|s| s.as_str()));
                log_args.extend(["--date-order", n.as_str(), "--format=%H%x00%P%x00%aN%x00%aE%x00%at%x00%s%x00%b%x00", "--"]);
                log_args.extend(path);
                (requested_revs.clone(), run(dir, &log_args), true)
            }
        }));
        let work = s.spawn(crate::read_jobs::inherit(|| try_status(dir)));
        // Refs (branches, remote branches, tags) with ahead/behind info.
        let refs = s.spawn(crate::read_jobs::inherit(|| {
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
        }));
        let head = s.spawn(crate::read_jobs::inherit(|| run(dir, &["symbolic-ref", "--short", "-q", "HEAD"])));
        let head_sha = s.spawn(crate::read_jobs::inherit(|| run(dir, &["rev-parse", "HEAD"])));
        let st = s.spawn(crate::read_jobs::inherit(|| run(dir, &["stash", "list", "--format=%H%x1f%gs%x1f%P%x1f%aN%x1f%aE%x1f%at"])));
        let wt = s.spawn(crate::read_jobs::inherit(|| run(dir, &["worktree", "list", "--porcelain", "-z"])));
        let sm = s.spawn(crate::read_jobs::inherit(|| run(dir, &["config", "--file", ".gitmodules", "--get-regexp", "path"])));
        let remotes = s.spawn(crate::read_jobs::inherit(|| run(dir, &["remote"])));
        let gitdir = s.spawn(crate::read_jobs::inherit(|| run(dir, &["rev-parse", "--git-dir"])));
        let j = |h: std::thread::ScopedJoinHandle<'_, GitOut>| h.join().unwrap_or_else(|_| GitOut::failed("a Git read stopped unexpectedly"));
        let log = log.join().unwrap_or_else(|_| (Vec::new(), GitOut::failed("a Git read stopped unexpectedly"), true));
        (log, j(refs), j(head), j(head_sha), j(st), j(wt), j(sm), j(remotes), work.join().unwrap_or_else(|_| Err("Git status read stopped unexpectedly".into())), j(gitdir))
    });
    d.history_revs = revs;
    d.history_date_order = date_order;
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

    crate::read_jobs::check()?;

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
            let mut args: Vec<&str> = vec!["--literal-pathspecs", "log"];
            args.extend(filters.iter().map(String::as_str));
            args.extend(start.iter().map(|s| s.as_str()));
            args.extend(["--date-order", n.as_str(), "--format=%H%x00%P%x00%aN%x00%aE%x00%at%x00%s%x00%b%x00", "--"]);
            args.extend(path);
            let result = run(dir, &args);
            let mut resolve = vec!["rev-parse", "--revs-only"];
            resolve.extend(start.iter().map(String::as_str));
            d.history_revs = run(dir, &resolve).stdout.lines().map(str::to_owned).collect();
            // The recovery log is read in date order, so later pages must continue in that order.
            date_order = true;
            result
        }
    };
    d.history_date_order = date_order;

    crate::read_jobs::check()?;
    if !log.ok && head_sha.ok { return Err(log.message()); }
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
        if f.len() == 6 {
            let base = f[2].split_whitespace().next().unwrap_or_default().to_owned();
            d.stashes.push(Stash { index: i, sha: f[0].to_string(), message: f[1].to_string(), base, author: f[3].to_owned(), email: f[4].to_owned(), time: f[5].parse().unwrap_or(0) });
        }
    }

    d.worktrees = parse_worktrees(&wt.stdout);

    for line in sm.stdout.lines() {
        if let Some((_, p)) = line.split_once(' ') {
            d.submodules.push(p.to_string());
        }
    }

    d.remotes = remotes.stdout.lines().map(|s| s.to_string()).collect();
    d.status = work?;

    let gitdir = gitdir.stdout.trim().to_string();
    let gd = dir.join(gitdir);
    d.bisect = bisect_progress(&gd);
    if gd.join("MERGE_HEAD").exists() {
        d.state_note = Some("merging".into());
        d.merge_msg = crate::rewrite::merge_message(dir);
    } else if gd.join("rebase-apply/applying").exists() {
        d.state_note = Some("applying-mailbox".into());
    } else if gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists() {
        d.state_note = Some("rebasing".into());
    } else if gd.join("CHERRY_PICK_HEAD").exists() {
        d.state_note = Some("cherry-picking".into());
    } else if gd.join("REVERT_HEAD").exists() {
        d.state_note = Some("reverting".into());
    }
    if d.state_note.is_some() {
        d.conflict_labels = Some(crate::rewrite::conflict_labels(dir));
    }
    crate::read_jobs::check()?;
    Ok(d)
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

#[cfg(test)]
pub fn status(dir: &Path) -> WorkStatus { try_status(dir).expect("Git status should succeed in this fixture") }

pub fn try_status(dir: &Path) -> Result<WorkStatus, String> {
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
    if !o.ok { return Err(o.message()); }
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
    Ok(w)
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

#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Hunk,
    Meta,
    Limit,
}

#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
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

/// Resolve the source path before a rename without losing it at the UI boundary.
pub fn rename_source(dir: &Path, src: &DiffSource, path: &str, parent: Option<&str>) -> String {
    let mut args = vec!["diff", "--name-status", "-z", "-M", "--no-ext-diff", "--no-textconv"];
    match src {
        DiffSource::Unstaged => {},
        DiffSource::Staged => args.push("--cached"),
        DiffSource::Commit(sha) => {
            if let Some(parent) = parent { args.extend([parent, sha]); }
            else { return path.into(); }
        },
        DiffSource::Range(from, to) => { args.push(from); if to != WORKDIR { args.push(to); } },
    }
    args.push("--");
    let output = run(dir, &args);
    let mut records = output.stdout.split('\0');
    while let Some(status) = records.next() {
        let Some(old) = records.next() else { break };
        if (status.starts_with('R') || status.starts_with('C'))
            && records.next() == Some(path) { return old.into(); }
    }
    path.into()
}

/// Raw unified diff text for a file (used for the hunk view and for building partial patches).
pub fn run_utf8(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = git_cmd(dir); command.args(args);
    let output = crate::read_jobs::run_bytes(command, 64 * 1024 * 1024, None);
    if !output.ok { return Err(String::from_utf8_lossy(&output.stderr).into_owned()); }
    String::from_utf8(output.stdout).map_err(|_| "Partial changes and text patch export require UTF-8 patch contents. Stage the whole file to preserve its original bytes, or explicitly convert it to UTF-8 first.".into())
}

pub fn diff_text(dir: &Path, src: &DiffSource, path: &str, parent: Option<&str>, ignore_ws: bool) -> String {
    diff_text_impl(dir, src, path, parent, ignore_ws, false).unwrap_or_default()
}
pub fn diff_text_checked(dir: &Path, src: &DiffSource, path: &str, parent: Option<&str>, ignore_ws: bool) -> Result<String, String> {
    diff_text_impl(dir, src, path, parent, ignore_ws, true)
}

fn diff_text_impl(dir: &Path, src: &DiffSource, path: &str, parent: Option<&str>, ignore_ws: bool, strict: bool) -> Result<String, String> {
    // Diff options: 3 context lines, hunks never merged.
    let ctx = format!("-U{}", crate::metrics::DIFF_CONTEXT_LINES);
    let inter = format!("--inter-hunk-context={}", crate::metrics::DIFF_INTERHUNK_LINES);
    let mut args: Vec<&str> = vec!["--literal-pathspecs", "-c", "diff.noprefix=false", "diff", "-M", "--src-prefix=a/", "--dst-prefix=b/", "--no-relative", "--no-color", "--no-ext-diff", "--no-textconv", ctx.as_str(), inter.as_str()];
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
                let mut a: Vec<&str> = vec!["--literal-pathspecs", "-c", "diff.noprefix=false", "show", "--src-prefix=a/", "--dst-prefix=b/", "--no-relative", "--no-ext-diff", "--no-textconv", "--no-color", "--format=", ctx.as_str(), inter.as_str()];
                if ignore_ws {
                    a.push("--ignore-all-space");
                }
                a.extend([sha.as_str(), "--", path]);
                return if strict { run_utf8(dir, &a) } else { Ok(run(dir, &a).stdout) };
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
    let original = rename_source(dir, src, path, parent);
    if original != path { args.push(&original); }
    if strict { run_utf8(dir, &args) } else { Ok(run(dir, &args).stdout) }
}

// Bound structured bridge allocations as well as the original file preview bytes.
pub fn diff_within_budget(text: &str) -> bool {
    text.len() <= 512 * 1024 && text.lines().take(5002).count() <= 5000
        && text.lines().filter(|line| line.starts_with("@@")).take(202).count() <= 200
}
fn diff_limit() -> Vec<DiffLine> {
    vec![DiffLine { kind: DiffLineKind::Limit, old_no: None, new_no: None,
        text: "Large diff: use the virtualized comparison or full-file actions.".into() }]
}

pub fn file_diff_opts(dir: &Path, src: &DiffSource, path: &str, untracked: bool, parent: Option<&str>, ignore_ws: bool) -> Vec<DiffLine> {
    let text = match src {
        DiffSource::Unstaged if untracked => {
            use std::io::Read;
            let mut content = Vec::new();
            if let Ok(file) = std::fs::File::open(dir.join(path)) {
                let _ = file.take(512 * 1024 + 1).read_to_end(&mut content);
            }
            if content.len() > 512 * 1024 || content.split(|b| *b == b'\n').take(5002).count() > 5000 { return diff_limit(); }
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
    if !diff_within_budget(&text) { return diff_limit(); }
    parse_diff(&text)
}

pub fn last_commit_message(dir: &Path) -> (String, String) {
    let o = run(dir, &["log", "-1", "--format=%s%x00%b"]);
    let mut it = o.stdout.splitn(2, '\0');
    let s = it.next().unwrap_or("").trim().to_string();
    let b = it.next().unwrap_or("").trim().to_string();
    (s, b)
}

pub fn repo_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Parse both initial history and subsequent pages identically.
pub fn parse_commit_fields(fields: &[&str]) -> Option<Commit> {
    fn valid_sha(value: &str) -> bool { matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) }
    if fields.len() != 7 { return None; }
    let sha = fields[0].trim_start_matches(['\r', '\n']);
    if !valid_sha(sha) || fields[1].split_whitespace().any(|parent| !valid_sha(parent)) { return None; }
    Some(Commit { sha: sha.into(), parents: fields[1].split_whitespace().map(str::to_owned).collect(), author: fields[2].into(), email: fields[3].into(), time: fields[4].parse().ok()?, summary: fields[5].into(), body: fields[6].trim().into() })
}
pub fn parse_commits(text: &str) -> Vec<Commit> {
    let fields: Vec<_> = text.split('\0').collect();
    fields.chunks_exact(7).filter_map(parse_commit_fields).collect()
}

/// Repositories whose history Git's plain order printed a parent before its child in (clock skew,
/// or equal times across merges): read in `--date-order` from then on.
fn date_order_repos() -> &'static std::sync::Mutex<std::collections::HashSet<PathBuf>> {
    static REPOS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> = std::sync::OnceLock::new();
    REPOS.get_or_init(Default::default)
}

fn needs_date_order(dir: &Path) -> bool {
    date_order_repos().lock().unwrap_or_else(|e| e.into_inner()).contains(dir)
}

fn mark_date_order(dir: &Path) {
    date_order_repos().lock().unwrap_or_else(|e| e.into_inner()).insert(dir.to_path_buf());
}

/// Whether every commit comes before its parents, as the graph needs, given the commits listed
/// earlier (`seen`, by the start of their id), which these are added to. `--date-order`
/// guarantees it but first walks the whole history unless the repository has a commit-graph
/// file; Git's plain order, by commit date, keeps it whenever children aren't dated before
/// their parents.
pub fn plain_order_holds(commits: &[Commit], seen: &mut std::collections::HashSet<u64>) -> bool {
    let key = |sha: &str| u64::from_str_radix(sha.get(..16).unwrap_or(sha), 16).unwrap_or(0);
    let mut ok = true;
    for c in commits {
        // A commit whose parent was listed already: the parent came first.
        if c.parents.iter().any(|p| seen.contains(&key(p))) {
            ok = false;
        }
        seen.insert(key(&c.sha));
    }
    ok
}

/// What a history page gives when Git's plain order turned out wrong for the repository: it is
/// read again from the start in `--date-order`.
pub const HISTORY_REORDERED: &str = "The history is read again in date order.";

#[allow(clippy::too_many_arguments)]
fn history_log(dir: &Path, revs: &[String], offset: usize, max: usize, filters: &[String], path: Option<&str>, date_order: bool) -> GitOut {
    let skip = format!("--skip={offset}");
    let limit = format!("-n{}", max + 1);
    let mut args = vec!["--literal-pathspecs", "log"];
    args.extend(filters.iter().map(String::as_str));
    args.push("--stdin");
    if date_order {
        args.push("--date-order");
    }
    args.extend([&skip, &limit, "--format=%H%x00%P%x00%aN%x00%aE%x00%at%x00%s%x00%b%x00", "--"]);
    args.extend(path);
    let command = git_command(dir, &args);
    crate::read_jobs::run(command, 64 * 1024 * 1024, Some((revs.join("\n") + "\n").into_bytes()))
}

/// The next page of history, in the order the first page was read in (`date_order`). In
/// Git's plain order the page is checked against the ids listed so far (`seen`); when a parent
/// came before its child, the repository switches to `--date-order` and this gives
/// [`HISTORY_REORDERED`].
#[allow(clippy::too_many_arguments)]
pub fn history_page_in(dir: &Path, revs: &[String], offset: usize, max: usize, filter: &HistoryFilter, date_order: bool, seen: &mut std::collections::HashSet<u64>) -> Result<(Vec<Commit>, bool), String> {
    if revs.is_empty() || revs.iter().any(|r| !matches!(r.len(), 40 | 64) || !r.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err("History changed; reload the repository to continue.".into());
    }
    let (filters, path) = filter.arguments()?;
    let output = history_log(dir, revs, offset, max, &filters, path, date_order);
    if !output.ok { return Err(output.message()); }
    let mut commits = parse_commits(&output.stdout);
    if !date_order && !plain_order_holds(&commits, seen) {
        mark_date_order(dir);
        return Err(HISTORY_REORDERED.into());
    }
    let has_more = commits.len() > max;
    commits.truncate(max);
    Ok((commits, has_more))
}

pub fn history_page(dir: &Path, revs: &[String], offset: usize, max: usize, filter: &HistoryFilter) -> Result<(Vec<Commit>, bool), String> {
    history_page_in(dir, revs, offset, max, filter, true, &mut Default::default())
}

#[cfg(test)]
mod paging_tests {
    use super::*;
    #[test]
    fn only_explicit_network_commands_qualify_for_hosting_credentials() {
        for args in [vec!["commit", "--message", "push"], vec!["-c", "test=fetch", "checkout", "main"], vec!["stash", "push"], vec!["remote", "add", "origin"]] {
            assert!(!uses_network_credentials(&args));
        }
        for args in [vec!["fetch"], vec!["-c", "test=value", "push"], vec!["lfs", "pull"], vec!["submodule", "update"], vec!["ls-remote"]] {
            assert!(uses_network_credentials(&args));
        }
    }
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
        let (page, more) = history_page(dir, &original.history_revs, 2, 2, &HistoryFilter::default()).unwrap();
        assert!(!more);
        let many_tips = vec![original.history_revs[0].clone(); 1000];
        let (many, _) = history_page(dir, &many_tips, 0, 2, &HistoryFilter::default()).unwrap();
        assert_eq!(many[0].sha, original.commits[0].sha);
        let actual: Vec<_> = original.commits.iter().chain(page.iter()).map(|c| &c.sha).collect();
        assert_eq!(actual, all.commits.iter().map(|c| &c.sha).collect::<Vec<_>>());
        assert!(history_page(dir, &["--all".into()], 0, 2, &HistoryFilter::default()).is_err());
    }

    /// main: A then C; branch x: B on A but dated before A (clock skew). Git's
    /// plain order lists A before its child B.
    fn skewed_repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "Test"], &["config", "user.email", "test@example.com"], &["config", "commit.gpgsign", "false"]] {
            assert!(run(dir, args).ok);
        }
        let commit = |msg: &str, at: &str| {
            let date = format!("{at} +0000");
            assert!(run_env(dir, &["commit", "-q", "--allow-empty", "-m", msg], &[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)]).ok);
        };
        commit("A", "1600001000");
        assert!(run(dir, &["checkout", "-q", "-b", "x"]).ok);
        commit("B", "1600000500");
        assert!(run(dir, &["checkout", "-q", "main"]).ok);
        commit("C", "1600002000");
        temp
    }

    fn children_first(commits: &[Commit]) -> bool {
        let mut seen = std::collections::HashSet::new();
        plain_order_holds(commits, &mut seen)
    }

    #[test]
    fn history_is_read_in_plain_order_unless_a_parent_would_come_first() {
        // Ordinary history: the plain order, which needn't walk everything first.
        let plain = tempfile::tempdir().unwrap();
        let dir = plain.path();
        for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "Test"], &["config", "user.email", "test@example.com"]] {
            assert!(run(dir, args).ok);
        }
        for name in ["one", "two", "three"] {
            assert!(run(dir, &["commit", "-q", "--allow-empty", "-m", name]).ok);
        }
        let data = load_view(dir, 10, &RefView::default());
        assert!(!data.history_date_order);
        assert_eq!(data.commits.len(), 3);
        // Clock skew on the first page: read again in date order, children before parents.
        let skewed = skewed_repo();
        let data = load_view(skewed.path(), 10, &RefView::default());
        assert!(data.history_date_order);
        assert_eq!(data.commits.len(), 3);
        assert!(children_first(&data.commits));
        let mut seen = std::collections::HashSet::new();
        let plain_log = parse_commits(&history_log(skewed.path(), &data.history_revs, 0, 10, &[], None, false).stdout);
        assert!(!plain_order_holds(&plain_log, &mut seen), "the fixture must trip the plain order");
    }

    #[test]
    fn a_later_page_in_the_wrong_order_switches_to_date_order() {
        let skewed = skewed_repo();
        let dir = skewed.path();
        // One commit per page: the first page alone is in order.
        let first = load_view(dir, 1, &RefView::default());
        assert!(!first.history_date_order);
        assert!(first.has_more);
        let mut seen = std::collections::HashSet::new();
        plain_order_holds(&first.commits, &mut seen);
        let next = history_page_in(dir, &first.history_revs, 1, 2, &HistoryFilter::default(), false, &mut seen);
        assert_eq!(next.unwrap_err(), HISTORY_REORDERED);
        // Read again: date order from the start.
        let again = load_view(dir, 10, &RefView::default());
        assert!(again.history_date_order);
        assert!(children_first(&again.commits));
    }
}

#[derive(Serialize)]
pub struct CommitFile {
    #[serde(flatten)] pub change: FileChange,
    pub storage: Option<String>,
}

pub fn commit_file_entries(dir: &Path, commit: &Commit) -> Vec<CommitFile> {
    let mut files: Vec<_> = commit_files(dir, commit).into_iter().map(|change| CommitFile { change, storage: None }).collect();
    // A stash's index commit has precisely the stash base as its parent. Its optional
    // third parent is a root commit containing the saved untracked files.
    if commit.parents.len() == 3 {
        let index_parent = run(dir, &["show", "-s", "--format=%P", &commit.parents[1]]);
        let untracked_parent = run(dir, &["show", "-s", "--format=%P", &commit.parents[2]]);
        if index_parent.ok && index_parent.stdout.trim() == commit.parents[0] && untracked_parent.ok && untracked_parent.stdout.trim().is_empty() {
            let names = run(dir, &["ls-tree", "-r", "--name-only", "-z", &commit.parents[2]]);
            for path in names.stdout.split('\0').filter(|s| !s.is_empty()) {
                if !files.iter().any(|f| f.change.path == path) {
                    files.push(CommitFile { change: FileChange { path: path.into(), status: 'A' }, storage: Some(commit.parents[2].clone()) });
                }
            }
        }
    }
    files
}
