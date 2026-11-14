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
    _workdir: Debouncer<RecommendedWatcher>,
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

/// True when every change in the .git folder is to the index (staging), which only affects the file lists.
pub fn index_only(gitdir: &Path, paths: &[PathBuf]) -> bool {
    !paths.is_empty()
        && paths.iter().all(|p| {
            p.parent() == Some(gitdir) && matches!(p.file_name().and_then(|n| n.to_str()), Some("index") | Some("index.lock"))
        })
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
    let dot_git = excluded(root.join(".git"));
    let lfs_tmp = excluded(gitdir.join("lfs").join("tmp"));

    // Working folder: 2000 ms, without .git, LFS temp files and ignored files.
    let r = root.to_path_buf();
    let (dg, lt) = (dot_git.clone(), lfs_tmp.clone());
    let cb = on_change.clone();
    let mut work = new_debouncer(debounce(WORKDIR_WATCH_DEBOUNCE_MS), move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let paths: Vec<PathBuf> = events.into_iter().map(|e| e.path).filter(|p| !under(p, &dg) && !under(p, &lt)).collect();
            if !not_ignored(&r, paths).is_empty() {
                cb(false);
            }
        }
    })
    .map_err(|e| e.to_string())?;
    work.watcher().watch(root, RecursiveMode::Recursive).map_err(|e| e.to_string())?;

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
        let mut g = new_debouncer(debounce(GITDIR_WATCH_DEBOUNCE_MS), move |res: DebounceEventResult| {
            if let Ok(events) = res {
                let paths: Vec<PathBuf> = events.into_iter().map(|e| e.path).filter(|p| !under(p, &lt)).collect();
                if !paths.is_empty() {
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
