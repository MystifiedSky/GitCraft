//! Stage 6 local stand-ins for cloud features:
//! - Patches: share changes as a .patch file (instead of hosted cloud patches).
//! - Conflict prevention: warn early when your branch would conflict with its target
//!   (proactive conflict detection with the target branch), computed locally
//!   with `git merge-tree` — nothing leaves the computer.

use crate::git;
use crate::metrics::{DIFF_CONTEXT_LINES, DIFF_INTERHUNK_LINES, MERGE_RENAME_LIMIT, MERGE_RENAME_THRESHOLD, PATCH_START_NUMBER, PATCH_SUBJECT_PREFIX};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A patch of uncommitted changes (`wip`) or of the given commits (oldest first, mailbox format).
pub fn make_patch(dir: &Path, shas: &[String]) -> Result<String, String> {
    // Keep the hunk boundaries unchanged in every export.
    let context = format!("-U{DIFF_CONTEXT_LINES}");
    let interhunk = format!("--inter-hunk-context={DIFF_INTERHUNK_LINES}");
    if shas.is_empty() {
        let baseline = if git::run(dir, &["rev-parse", "--verify", "HEAD"]).ok { "HEAD".into() } else {
            let empty = git::run(dir, &["hash-object", "-w", "-t", "tree", "--stdin"]);
            if !empty.ok { return Err(empty.message()); } empty.stdout.trim().to_owned()
        };
        let text = git::run_utf8(dir, &["diff", &baseline, "--binary", "--no-color", "--no-ext-diff", &context, &interhunk])?;
        if text.trim().is_empty() {
            return Err("There are no uncommitted changes to put in a patch (new files must be staged first).".into());
        }
        return Ok(text);
    }
    let mut out = String::new();
    let total = shas.len();
    // The subject prefix is PATCH, numbering starts at 1, and there is no reroll suffix by default.
    // Override personal format.* settings before adding the series numbering ourselves.
    let prefix = format!("--subject-prefix={PATCH_SUBJECT_PREFIX}");
    let start = format!("--start-number={PATCH_START_NUMBER}");
    for (i, sha) in shas.iter().enumerate() {
        let parents = git::run(dir, &["rev-list", "--parents", "-n", "1", sha]);
        if !parents.ok { return Err(parents.message()); }
        if parents.stdout.split_whitespace().count() > 2 {
            return Err("Merge commits cannot be exported as mailbox patches. Choose the individual commits instead.".into());
        }
        let text = git::run_utf8(dir, &["format-patch", "-1", "--stdout", "--binary", &context, &interhunk, &prefix, "--no-numbered", "--no-cover-letter", &start, sha])?;
        // Native indices are one-based: number = i_native - 1 + S, total = N - 1 + S.
        out.push_str(&number_subject(&text, i + PATCH_START_NUMBER, total - 1 + PATCH_START_NUMBER));
    }
    Ok(out)
}

/// The email subject: "Subject: [PATCH] …" for one patch,
/// "[PATCH n/total]" when there are several (start number 1).
pub fn number_subject(patch: &str, n: usize, total: usize) -> String {
    if total <= 1 {
        return patch.to_string();
    }
    patch.replacen(
        &format!("Subject: [{PATCH_SUBJECT_PREFIX}] "),
        &format!("Subject: [{PATCH_SUBJECT_PREFIX} {n}/{total}] "),
        1,
    )
}

/// A zero merge limit selects diff.renamelimit; a nonpositive result selects 1000.
fn rename_limit(dir: &Path) -> u32 {
    let read = |key: &str| -> i32 {
        let o = git::run(dir, &["config", "--int", "--get", key]);
        if o.ok { o.stdout.trim().parse().unwrap_or(0) } else { 0 }
    };
    // diff.renamelimit is used only when merge.renamelimit is zero. A negative merge
    // value goes straight to the default; it must not select diff.renamelimit.
    let merge = read("merge.renamelimit");
    let limit = if merge == 0 { read("diff.renamelimit") } else { merge };
    if limit > 0 { limit as u32 } else { MERGE_RENAME_LIMIT }
}

/// Shared merge settings: rename detection is on,
/// similarity defaults to 50%, and the candidate limit follows rename_limit above.
/// Use for previews and merges so personal merge.renames settings cannot make them disagree.
pub fn merge_args(dir: &Path, command: &str, args: &[&str]) -> Vec<String> {
    let mut out = vec![
        "-c".into(),
        "merge.renames=true".into(),
        "-c".into(),
        format!("merge.renameLimit={}", rename_limit(dir)),
        command.into(),
        "-X".into(),
        format!("find-renames={MERGE_RENAME_THRESHOLD}%"),
    ];
    out.extend(args.iter().map(|s| (*s).to_string()));
    out
}

/// Apply a patch file: mailbox patches become commits (`git am`), plain diffs change the working folder.
pub fn apply_patch_file(dir: &Path, file: &Path, run: &dyn Fn(&[&str]) -> git::GitOut) -> git::GitOut {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let f = file.to_string_lossy().into_owned();
    if text.starts_with("From ") {
        for state in ["rebase-apply", "rebase-merge", "MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"] {
            let path = git::run(dir, &["rev-parse", "--git-path", state]);
            let path = dir.join(path.stdout.trim());
            if path.exists() {
                return git::GitOut::failed("Finish or abort the current Git operation before importing a mailbox patch.");
            }
        }
        // Keep a newly conflicted session so the user can resolve or explicitly abort it.
        run(&["am", "--3way", &f])
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
    // Use the same recursion and rename rules as the working-directory lists.
    // NUL records keep filenames containing line breaks from inflating the count.
    let options = git::status_options();
    let o = git::run(dir, &["--no-optional-locks", "status", "--porcelain=v2", "--branch", "-z", &options[0], &options[1]]);
    if !o.ok {
        return RepoSummary::default();
    }
    let mut s = RepoSummary { exists: true, ..Default::default() };
    let mut records = o.stdout.split('\0');
    while let Some(line) = records.next() {
        if let Some(b) = line.strip_prefix("# branch.head ") {
            s.branch = (b != "(detached)").then(|| b.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut it = ab.split_whitespace();
            s.ahead = it.next().and_then(|x| x.trim_start_matches('+').parse().ok()).unwrap_or(0);
            s.behind = it.next().and_then(|x| x.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if line.starts_with("2 ") {
            s.changes += 1;
            records.next(); // A renamed/copied file has a second NUL record for its old path.
        } else if line.starts_with("1 ") || line.starts_with("u ") || line.starts_with("? ") {
            // Each porcelain entry represents a path, even when both index and worktree changed.
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
    let named = o.stdout.trim();
    // origin/HEAD can name a branch the remote has since deleted; only use it when it still exists.
    if o.ok && !named.is_empty() && git::run(dir, &["rev-parse", "--verify", "-q", &format!("{named}^{{commit}}")]).ok {
        return Some(named.to_string());
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
    let args = merge_args(dir, "merge-tree", &["--write-tree", "--name-only", "--no-messages", "HEAD", &target]);
    let o = git::run(dir, &args.iter().map(String::as_str).collect::<Vec<_>>());
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

        // Personal email settings must not override the default subject rules.
        g(&["config", "format.subjectPrefix", "CUSTOM"]);
        g(&["config", "format.numbered", "true"]);
        g(&["config", "format.coverLetter", "true"]);
        // Patch of a commit, applied as a commit on another branch.
        let patch = make_patch(&d, &[side.clone()]).unwrap();
        assert!(patch.starts_with("From "));
        assert!(patch.lines().any(|l| l == "Subject: [PATCH] side change"), "{patch}");
        assert_eq!(patch.lines().filter(|l| l.starts_with("Subject: ")).count(), 1);
        let series = make_patch(&d, &[side.clone(), side.clone()]).unwrap();
        assert!(series.lines().any(|l| l == "Subject: [PATCH 1/2] side change"), "{series}");
        assert!(series.lines().any(|l| l == "Subject: [PATCH 2/2] side change"), "{series}");
        assert_eq!(series.lines().filter(|l| l.starts_with("Subject: ")).count(), 2);
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
    fn patch_exports_keep_hunk_boundaries() {
        let d = std::env::temp_dir().join(format!("gitcraft-patch-context-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| assert!(git::run(&d, a).ok, "{a:?}");
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@e.x"]);
        g(&["config", "commit.gpgsign", "false"]);
        let base: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        fs::write(d.join("context.txt"), &base).unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        // These settings would remove context and join both changes without explicit overrides.
        g(&["config", "diff.context", "0"]);
        g(&["config", "diff.interHunkContext", "100"]);
        let changed = base.replace("line 5\n", "changed 5\n").replace("line 15\n", "changed 15\n");
        fs::write(d.join("context.txt"), changed).unwrap();
        let check = |patch: String| {
            assert_eq!(patch.lines().filter(|l| l.starts_with("@@ ")).count(), 2, "{patch}");
            assert!(patch.contains("@@ -2,7 +2,7 @@"), "{patch}");
            assert!(patch.contains("@@ -12,7 +12,7 @@"), "{patch}");
        };
        check(make_patch(&d, &[]).unwrap());
        g(&["commit", "-q", "-am", "two changes"]);
        let sha = git::run(&d, &["rev-parse", "HEAD"]).stdout.trim().to_string();
        check(make_patch(&d, &[sha]).unwrap());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rename_limit_uses_zero_and_negative_rules() {
        let d = std::env::temp_dir().join(format!("gitcraft-merge-limits-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| assert!(git::run(&d, a).ok, "{a:?}");
        g(&["init", "-q", "-b", "main"]);
        // Set both keys so inherited user configuration cannot affect the cases.
        g(&["config", "merge.renamelimit", "0"]);
        g(&["config", "diff.renamelimit", "77"]);
        assert_eq!(rename_limit(&d), 77);
        g(&["config", "merge.renamelimit", "-1"]);
        assert_eq!(rename_limit(&d), 1000);
        g(&["config", "merge.renamelimit", "2k"]);
        assert_eq!(rename_limit(&d), 2048);
        g(&["config", "merge.renamelimit", "0"]);
        g(&["config", "diff.renamelimit", "-1"]);
        assert_eq!(rename_limit(&d), 1000);
        g(&["config", "diff.renamelimit", "0"]);
        assert_eq!(rename_limit(&d), 1000);
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
        // Personal settings must not hide files or collapse a new directory into one change.
        assert!(git::run(&d, &["config", "status.showUntrackedFiles", "no"]).ok);
        fs::create_dir_all(d.join("nested")).unwrap();
        fs::write(d.join("nested/a.txt"), "a").unwrap();
        fs::write(d.join("nested/b.txt"), "b").unwrap();
        assert_eq!(repo_summary(&d).changes, 3);
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
        // The repository summary must consume the old-path record and count this rename once.
        assert_eq!(repo_summary(&d).changes, 1);
        fs::write(d.join("new.txt"), "edited after staging\n").unwrap();
        assert_eq!(repo_summary(&d).changes, 1);
        let _ = fs::remove_dir_all(&d);
    }

}
