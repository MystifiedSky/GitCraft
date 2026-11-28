//! Remove an individual stale worktree registration without pruning its neighbors.

use crate::git;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

fn lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => { normalized.pop(); }
            Component::CurDir => {}
            _ => normalized.push(part.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    // Git can expand an existing folder's Windows short name when it lists worktrees.
    if let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) {
        return a == b;
    }
    let (a, b) = (lexical_path(a), lexical_path(b));
    if cfg!(windows) {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    } else {
        a == b
    }
}

fn missing(path: &Path) -> io::Result<bool> {
    match path.symlink_metadata() {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

fn registered_link(entry: &Path) -> io::Result<PathBuf> {
    let target = fs::read_to_string(entry.join("gitdir"))?;
    // Newer Git versions can store the link relative to its administrative directory.
    Ok(entry.join(target.trim_end_matches(['\r', '\n'])))
}

/// Recheck both sides of the Git link before allowing recursive removal.
pub(crate) fn force_removal_path(dir: &Path, path: &str) -> Result<String, String> {
    let changed = || "The worktree is locked, disconnected, or has moved. Refresh and review it again.".to_string();
    let target = dir.join(path);
    let worktree = git::try_worktrees(dir)?.into_iter().skip(1).find(|worktree| {
        same_path(Path::new(&worktree.path), &target)
    }).ok_or_else(changed)?;
    let root = Path::new(&worktree.path);
    let link = root.join(".git");
    if worktree.locked || worktree.prunable || !root.is_absolute()
        || !link.symlink_metadata().is_ok_and(|meta| meta.is_file())
    {
        return Err(changed());
    }
    let git_path = |dir: &Path, args: &[&str]| {
        let output = git::run(dir, args);
        output.ok.then(|| fs::canonicalize(output.stdout.trim_end_matches(['\r', '\n'])).ok()).flatten()
    };
    let common = git_path(dir, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).ok_or_else(changed)?;
    let admin = git_path(root, &["rev-parse", "--absolute-git-dir"]).ok_or_else(changed)?;
    if admin.parent() != Some(common.join("worktrees").as_path())
        || !registered_link(&admin).is_ok_and(|registered| same_path(&registered, &link))
    {
        return Err(changed());
    }
    Ok(worktree.path)
}

/// Git's prune command removes all eligible entries. Detach only the selected entry,
/// identified by its pointer back to the worktree, and leave its working folder alone.
pub(crate) fn forget_stale(dir: &Path, worktree: &git::Worktree) -> Result<(), String> {
    let common = git::run(dir, &["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    if !common.ok {
        return Err(common.message());
    }
    forget_entry(Path::new(common.stdout.trim()), worktree).map_err(|error| {
        format!("Could not remove the stale worktree registration. {error}")
    })
}

fn forget_entry(common: &Path, worktree: &git::Worktree) -> io::Result<()> {
    let storage = common.join("worktrees");
    let canonical_common = common.canonicalize()?;
    let canonical_storage = storage.canonicalize()?;
    if canonical_storage != canonical_common.join("worktrees") {
        return Err(io::Error::other("The worktree registry is outside the repository."));
    }
    let link = Path::new(&worktree.path).join(".git");
    if !worktree.prunable || worktree.locked || !link.is_absolute() || !missing(&link)? {
        return Err(io::Error::other("The worktree is locked or its Git link has changed."));
    }

    let mut matched = None;
    for entry in fs::read_dir(&storage)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let target = match registered_link(&path) {
            Ok(target) => target,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !same_path(&target, &link) {
            continue;
        }
        let canonical = path.canonicalize()?;
        if matched.is_some() || canonical != canonical_storage.join(entry.file_name()) {
            return Err(io::Error::other("The worktree registration is ambiguous or redirected."));
        }
        matched = Some(path);
    }
    let path = matched.ok_or_else(|| io::Error::other("Git's registration for this folder could not be found."))?;
    let quarantine = tempfile::Builder::new().prefix("gitcraft-removed-worktree-").tempdir_in(&canonical_common)?;
    if !missing(&link)? || !missing(&path.join("locked"))? || !missing(&path.join("index.lock"))?
        || !same_path(&registered_link(&path)?, &link)
    {
        return Err(io::Error::other("The worktree was reconnected or locked. Refresh and try again."));
    }
    // Moving the verified entry out of the registry removes it atomically. The temporary
    // directory only owns Git administrative files, even if the old folder contains edits.
    fs::rename(path, quarantine.path().join("registration"))
}
