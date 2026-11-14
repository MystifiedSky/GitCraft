//! Repository file watching:
//! - the working folder is watched with a 2000 ms debounce, leaving out the .git folder,
//!   `.git/lfs/tmp` and files Git ignores;
//! - the .git folder is watched separately with a 200 ms debounce, leaving out `.git/lfs/tmp`;
//! - a debounce must be between 1 and 60000 ms; a trailing '/' on an excluded path is dropped.

use crate::metrics::{GITDIR_WATCH_DEBOUNCE_MS, WATCH_DEBOUNCE_MAX_MS, WATCH_DEBOUNCE_MIN_MS, WORKDIR_WATCH_DEBOUNCE_MS};
use notify_debouncer_mini::{new_debouncer, notify::RecommendedWatcher, notify::RecursiveMode, DebounceEventResult, Debouncer};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

pub struct RepoWatch {
    _workdir: WorkWatch,
    _gitdirs: Vec<Debouncer<RecommendedWatcher>>,
}

/// Clamp a debounce to the accepted range.
pub fn debounce(ms: u32) -> Duration {
    Duration::from_millis(ms.clamp(WATCH_DEBOUNCE_MIN_MS, WATCH_DEBOUNCE_MAX_MS) as u64)
}

/// Normalise an excluded path: drop one trailing separator.
pub fn excluded(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_suffix('/').or_else(|| s.strip_suffix('\\')) {
        Some(t) => PathBuf::from(t),
        None => p,
    }
}

fn under(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
}

/// Asks Git which of these paths are ignored (one `git check-ignore` per batch).
fn not_ignored(root: &Path, paths: Vec<PathBuf>) -> Vec<PathBuf> {
    if paths.is_empty() {
        return paths;
    }
    let mut cmd = crate::git::git_cmd(root);
    cmd.args(["check-ignore", "--stdin", "-z"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else { return paths };
    let input: Vec<u8> = paths.iter().flat_map(|p| {
        let mut bytes = p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/").into_bytes();
        bytes.push(0);
        bytes
    }).collect();
    // Drain stdout while writing stdin: large batches can otherwise fill both pipes and deadlock.
    let writer = child.stdin.take().map(|mut si| std::thread::spawn(move || si.write_all(&input)));
    let out = child.wait_with_output();
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let Ok(out) = out else { return paths };
    let ignored: HashSet<String> = String::from_utf8_lossy(&out.stdout).split('\0').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
    paths
        .into_iter()
        .filter(|p| {
            let rel = p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/");
            !ignored.contains(&rel)
        })
        .collect()
}

/// Cache exact classifications only; unknown paths still go through Git's ignore engine.
#[derive(Default)]
struct IgnoreCache {
    values: std::collections::HashMap<PathBuf, bool>,
    checked: Option<std::time::Instant>,
    global_file: Option<PathBuf>,
    global_stamp: Option<std::time::SystemTime>,
}
impl IgnoreCache {
    fn filter(&mut self, root: &Path, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let stamp = self.global_file.as_ref().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
        if self.checked.is_none_or(|at| at.elapsed() > Duration::from_secs(30)) || stamp != self.global_stamp || paths.iter().any(|p| p.file_name().is_some_and(|n| n == ".gitignore")) {
            self.values.clear(); self.checked = Some(std::time::Instant::now()); self.global_stamp = stamp;
        }
        if self.values.len() > 8192 { self.values.clear(); }
        let unknown: Vec<_> = paths.iter().filter(|p| !self.values.contains_key(*p)).cloned().collect();
        if !unknown.is_empty() {
            let allowed: HashSet<_> = not_ignored(root, unknown.clone()).into_iter().collect();
            for path in unknown { let keep = allowed.contains(&path); self.values.insert(path, keep); }
        }
        paths.into_iter().filter(|path| self.values.get(path).copied().unwrap_or(true)).collect()
    }
}

/// True when every change in the .git folder is to the index (staging), which only affects the file lists.
pub fn index_only(gitdir: &Path, paths: &[PathBuf]) -> bool {
    !paths.is_empty()
        && paths.iter().all(|p| {
            p.parent() == Some(gitdir) && matches!(p.file_name().and_then(|n| n.to_str()), Some("index") | Some("index.lock"))
        })
}

pub fn relevant_metadata(gitdir: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(gitdir) else { return true };
    if relative.components().count() == 1 && relative.to_string_lossy().starts_with("gitcraft-") { return false; }
    if relative == Path::new("logs/refs/stash") { return true; }
    let first = relative.components().next().and_then(|c| c.as_os_str().to_str());
    !matches!(first, Some("objects" | "logs" | "lfs"))
}

enum WorkEvent { Paths(Vec<PathBuf>), Reconfigure(bool), Stop }
struct WorkWatch {
    events: std::sync::mpsc::Sender<WorkEvent>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl WorkWatch {
    fn start(root: &Path, changed: impl Fn(bool) + Send + 'static) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let mut plan = crate::watch_plan::WatchPlan::read(&root)?;
        let (events, messages) = std::sync::mpsc::channel();
        let sender = events.clone();
        let mut watcher = new_debouncer(debounce(WORKDIR_WATCH_DEBOUNCE_MS), move |result: DebounceEventResult| {
            let event = match result {
                Ok(events) => WorkEvent::Paths(events.into_iter().map(|event| event.path).collect()),
                Err(_) => WorkEvent::Reconfigure(true),
            };
            let _ = sender.send(event);
        }).map_err(|error| error.to_string())?;
        for directory in &plan.registrations { watcher.watcher().watch(directory, RecursiveMode::NonRecursive).map_err(|error| error.to_string())?; }
        let worker = std::thread::spawn(move || {
            let mut cache = IgnoreCache::default();
            while let Ok(event) = messages.recv() {
                let (mut reconfigure, mut full, mut paths) = match event {
                    WorkEvent::Stop => break,
                    WorkEvent::Reconfigure(config) => (true, config, Vec::new()),
                    WorkEvent::Paths(paths) => (false, false, paths),
                };
                // Coalesce metadata/index bursts into one registration plan.
                let mut stop = false;
                while let Ok(event) = messages.try_recv() {
                    match event { WorkEvent::Stop => stop = true, WorkEvent::Reconfigure(config) => { reconfigure = true; full |= config; }, WorkEvent::Paths(more) => paths.extend(more) }
                }
                if stop { break; }
                full |= paths.iter().any(|path| plan.configuration_changed(path));
                reconfigure |= full || paths.iter().any(|path| path.file_name().is_some_and(|name| name == ".gitignore") || plan.configuration.contains(path));
                let working: Vec<_> = paths.into_iter().filter(|path| path.starts_with(&root) && !path.strip_prefix(&root).is_ok_and(|relative| relative.components().any(|part| part.as_os_str() == ".git"))).collect();
                let allowed = cache.filter(&root, working);
                reconfigure |= allowed.iter().any(|path| (path.is_dir() && !plan.directories.contains(path)) || (!path.exists() && plan.directories.contains(path)));
                if reconfigure {
                    cache.values.clear();
                    let next = if full { crate::watch_plan::WatchPlan::read(&root) } else { crate::watch_plan::WatchPlan::refresh(&root, &plan) };
                    if let Ok(next) = next {
                        // Install additions before removing old registrations. Refresh afterwards
                        // also covers files created while the new directory was not yet watched.
                        let mut installed = true;
                        let mut added = Vec::new();
                        for path in next.registrations.difference(&plan.registrations) {
                            if watcher.watcher().watch(path, RecursiveMode::NonRecursive).is_err() { installed = false; }
                            else { added.push(path.clone()); }
                        }
                        if installed {
                            for path in plan.registrations.difference(&next.registrations) { let _ = watcher.watcher().unwatch(path); }
                            plan = next;
                        } else { for path in added { let _ = watcher.watcher().unwatch(&path); } }
                    }
                    changed(full);
                } else if !allowed.is_empty() { changed(false); }
            }
        });
        Ok(Self { events, worker: Some(worker) })
    }
}
impl Drop for WorkWatch {
    fn drop(&mut self) {
        let _ = self.events.send(WorkEvent::Stop);
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

/// `on_change(full)`: `full` is false when only the file lists can have changed (files edited in the working
/// folder, or the index), so the window refreshes just the status instead of the whole repository.
pub fn start(root: &Path, on_change: impl Fn(bool) + Send + Sync + Clone + 'static) -> Result<RepoWatch, String> {
    let gitdir = {
        let o = crate::git::run(root, &["rev-parse", "--absolute-git-dir"]);
        if o.ok {
            PathBuf::from(o.stdout.trim())
        } else {
            root.join(".git")
        }
    };
    let work = WorkWatch::start(root, on_change.clone())?;
    let work_events = work.events.clone();
    let lfs_tmp = excluded(gitdir.join("lfs").join("tmp"));

    // Linked worktrees keep HEAD/index in their own gitdir and branches/tags in the shared common dir.
    let common = crate::git::run(root, &["rev-parse", "--git-common-dir"]);
    let common = if common.ok { root.join(common.stdout.trim()) } else { gitdir.clone() };
    let mut dirs = vec![gitdir.clone()];
    if common != gitdir {
        dirs.push(common);
    }
    // Git metadata: 200 ms, without LFS temp files.
    let mut git = Vec::new();
    for gitdir in dirs {
        if !gitdir.exists() {
            continue;
        }
        let lt = lfs_tmp.clone();
        let cb = on_change.clone();
        let gd = gitdir.clone();
        let work_events = work_events.clone();
        let mut g = new_debouncer(debounce(GITDIR_WATCH_DEBOUNCE_MS), move |res: DebounceEventResult| {
            if let Ok(events) = res {
                let paths: Vec<PathBuf> = events.into_iter().map(|e| e.path).filter(|p| !under(p, &lt) && relevant_metadata(&gd, p)).collect();
                if !paths.is_empty() {
                    if paths.iter().any(|p| matches!(p.file_name().and_then(|n| n.to_str()), Some("index" | "config" | "config.worktree" | "exclude" | "HEAD"))) { let _ = work_events.send(WorkEvent::Reconfigure(paths.iter().any(|path| matches!(path.file_name().and_then(|name| name.to_str()), Some("config" | "config.worktree" | "HEAD"))))); }
                    cb(!index_only(&gd, &paths));
                }
            }
        })
        .map_err(|e| e.to_string())?;
        g.watcher().watch(&gitdir, RecursiveMode::Recursive).map_err(|e| e.to_string())?;
        git.push(g);
    }
    Ok(RepoWatch { _workdir: work, _gitdirs: git })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_refreshes_ignore_object_and_reflog_writes_but_keep_refs_and_state() {
        let root = PathBuf::from("repo/.git");
        for path in ["objects/pack/new.pack", "objects/ab/cdef", "logs/HEAD", "lfs/tmp/file"] {
            assert!(!relevant_metadata(&root, &root.join(path)));
        }
        for path in ["HEAD", "index", "refs/heads/main", "refs/stash", "packed-refs", "MERGE_HEAD", "config", "logs/refs/stash"] {
            assert!(relevant_metadata(&root, &root.join(path)));
        }
    }

    #[test]
    fn index_changes_only_need_a_status_refresh() {
        let g = PathBuf::from("/r/.git");
        assert!(index_only(&g, &[g.join("index"), g.join("index.lock")]));
        assert!(!index_only(&g, &[g.join("index"), g.join("HEAD")]));
        assert!(!index_only(&g, &[g.join("refs").join("heads").join("index")]));
        assert!(!index_only(&g, &[]));
    }

    #[test]
    fn debounce_is_clamped() {
        assert_eq!(debounce(0), Duration::from_millis(1));
        assert_eq!(debounce(2000), Duration::from_millis(2000));
        assert_eq!(debounce(90_000), Duration::from_millis(60_000));
    }

    #[test]
    fn trailing_separator_dropped() {
        assert_eq!(excluded(PathBuf::from("a/b/")), PathBuf::from("a/b"));
        assert_eq!(excluded(PathBuf::from("a/b")), PathBuf::from("a/b"));
    }
}


#[cfg(test)]
mod ignore_cache_tests {
    use super::*;
    #[test]
    fn caches_ignored_build_events_and_invalidates_changed_ignore_rules() {
        let folder=tempfile::tempdir().unwrap();let dir=folder.path();assert!(crate::git::run(dir,&["init","-q"]).ok);
        std::fs::write(dir.join(".gitignore"),"build/\n").unwrap();std::fs::create_dir(dir.join("build")).unwrap();std::fs::write(dir.join("build/file.txt"),"ignored").unwrap();
        let path=dir.join("build/file.txt");let mut cache=IgnoreCache::default();let start=std::time::Instant::now();assert!(cache.filter(dir,vec![path.clone()]).is_empty());let first=start.elapsed();
        let start=std::time::Instant::now();for _ in 0..1000 {assert!(cache.filter(dir,vec![path.clone()]).is_empty());}let cached=start.elapsed();
        eprintln!("Ignore classification: first Git query {first:?}; 1000 cached batches {cached:?}");
        std::fs::write(dir.join(".gitignore"),"").unwrap();assert!(cache.filter(dir,vec![dir.join(".gitignore"),path.clone()]).contains(&path));
        std::fs::write(dir.join(".gitignore"),"build/\n").unwrap();assert!(crate::git::run(dir,&["add","-f","build/file.txt"]).ok);cache.values.clear();assert!(cache.filter(dir,vec![path.clone()]).contains(&path));
    }
}


#[cfg(test)]
mod registration_tests {
    use super::*;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    fn wait_until(label:&str, condition:impl Fn()->bool) {
        let start=std::time::Instant::now();
        while start.elapsed()<Duration::from_secs(12) {if condition(){return;}std::thread::sleep(Duration::from_millis(50));}
        panic!("Timed out: {label}");
    }
    #[test]
    fn real_events_follow_new_directories_ignore_edits_and_tracked_exceptions() {
        let folder=tempfile::tempdir().unwrap();let root=folder.path().canonicalize().unwrap();assert!(crate::git::run(&root,&["init","-q"]).ok);
        std::fs::write(root.join(".gitignore"),"build/\n").unwrap();std::fs::create_dir(root.join("build")).unwrap();
        let events=Arc::new(AtomicUsize::new(0));let count=events.clone();let watch=start(&root,move |_|{count.fetch_add(1,Ordering::SeqCst);}).unwrap();
        std::fs::create_dir_all(root.join("new/deep")).unwrap();std::fs::write(root.join("new/deep/file"),"initial").unwrap();
        wait_until("new directory",||events.load(Ordering::SeqCst)>0);
        std::thread::sleep(Duration::from_millis(2500));let baseline=events.load(Ordering::SeqCst);
        std::fs::write(root.join("new/deep/file"),"later edit").unwrap();wait_until("registered child edit",||events.load(Ordering::SeqCst)>baseline);
        std::thread::sleep(Duration::from_millis(2500));let baseline=events.load(Ordering::SeqCst);
        std::fs::create_dir_all(root.join("build/generated/deep")).unwrap();for n in 0..300 {std::fs::write(root.join(format!("build/generated/deep/{n}")),"ignored").unwrap();}
        std::thread::sleep(Duration::from_millis(3500));assert_eq!(events.load(Ordering::SeqCst),baseline,"ignored build caused a refresh");
        std::fs::write(root.join("build/keep"),"tracked").unwrap();assert!(crate::git::run(&root,&["add","-f","build/keep"]).ok);
        std::thread::sleep(Duration::from_millis(3000));let baseline=events.load(Ordering::SeqCst);std::fs::write(root.join("build/keep"),"tracked edit").unwrap();wait_until("tracked ignored exception",||events.load(Ordering::SeqCst)>baseline);
        std::fs::write(root.join(".gitignore"),"").unwrap();std::thread::sleep(Duration::from_millis(3500));let baseline=events.load(Ordering::SeqCst);
        std::fs::write(root.join("build/generated/deep/0"),"now visible").unwrap();wait_until("unignored subtree",||events.load(Ordering::SeqCst)>baseline);
        drop(watch);
    }
}
