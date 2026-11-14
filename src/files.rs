//! Stage 2: file contents at any point, partial (hunk / line) staging, file history,
//! blame, per-commit change counts and comparisons between two points.

use crate::git::{self, run, Commit, DiffSource, FileChange, WORKDIR};
use base64::Engine as _;
use serde::Serialize;
use std::path::Path;

/// Runs git and returns raw stdout bytes (file contents may not be UTF-8).
fn run_bytes(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let o = git::git_cmd(dir).args(args).output().ok()?;
    if o.status.success() {
        Some(o.stdout)
    } else {
        None
    }
}

/// Contents of `file` at `rev`: a commit sha, ":" for the index (staging area), or WORKDIR.
pub fn contents(dir: &Path, rev: &str, file: &str) -> Option<Vec<u8>> {
    if rev == WORKDIR {
        return std::fs::read(dir.join(file)).ok();
    }
    let spec = if rev == ":" { format!(":{file}") } else { format!("{rev}:{file}") };
    run_bytes(dir, &["show", &spec])
}

#[derive(Serialize, Default)]
pub struct FileVersions {
    pub old: Option<String>,
    pub new: Option<String>,
    pub binary: bool,
    /// Image previews as data URLs (before / after) when the file is a picture.
    pub old_image: Option<String>,
    pub new_image: Option<String>,
    pub encoding: String,
    /// The working-folder copy can be edited and saved.
    pub editable: bool,
}

const IMAGE_TYPES: [(&str, &str); 8] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("bmp", "image/bmp"),
    ("webp", "image/webp"),
    ("ico", "image/x-icon"),
    ("svg", "image/svg+xml"),
];

/// A file is binary by libgit2's rules:
/// look at the first 8000 bytes; a UTF-16 / UTF-32 byte-order mark means binary; a UTF-8 one is
/// skipped; a NUL byte means binary; otherwise binary when (printable >> 7) < non-printable.
/// TAB, LF, VT and CR count as neither; BS, FF and ESC count as printable.
pub fn is_binary(data: &[u8]) -> bool {
    use crate::metrics::{BINARY_CHECK_BYTES, BINARY_PRINTABLE_SHIFT};
    let data = &data[..data.len().min(BINARY_CHECK_BYTES)];
    let mut start = 0;
    if data.len() > 1 {
        match data {
            [0xEF, 0xBB, 0xBF, ..] => start = 3,                    // UTF-8: still text
            [0xFE, 0xFF, ..] | [0xFF, 0xFE, ..] => return true,      // UTF-16 (and UTF-32 LE)
            [0x00, 0x00, 0xFE, 0xFF, ..] => return true,             // UTF-32 BE
            _ => {}
        }
    }
    let (mut printable, mut nonprintable) = (0u32, 0u32);
    for &b in &data[start..] {
        match b {
            0 => return true,
            8 | 12 | 27 => printable += 1,
            9 | 10 | 11 | 13 => {} // TAB, LF, VT and CR are neutral.
            0x7f => nonprintable += 1,
            b if b < 0x20 => nonprintable += 1,
            _ => printable += 1,
        }
    }
    (printable >> BINARY_PRINTABLE_SHIFT) < nonprintable
}

fn mime_of(file: &str) -> Option<&'static str> {
    let ext = file.rsplit('.').next()?.to_ascii_lowercase();
    IMAGE_TYPES.iter().find(|(e, _)| *e == ext).map(|(_, m)| *m)
}

/// The two sides ("before" and "after") of a file for a diff source.
pub fn sides(src: &DiffSource, parent: Option<&str>) -> (Option<String>, String) {
    match src {
        DiffSource::Unstaged => (Some(":".into()), WORKDIR.into()),
        DiffSource::Staged => (Some("HEAD".into()), ":".into()),
        DiffSource::Commit(sha) => (parent.map(|p| p.to_string()), sha.clone()),
        DiffSource::Range(a, b) => (Some(a.clone()), b.clone()),
    }
}

pub fn versions(dir: &Path, src: &DiffSource, file: &str, parent: Option<&str>, untracked: bool) -> FileVersions {
    let (old_rev, new_rev) = sides(src, parent);
    let old = if untracked { None } else { old_rev.as_deref().and_then(|r| contents(dir, r, file)) };
    let new = contents(dir, &new_rev, file);
    let mut v = FileVersions { encoding: "UTF-8".into(), editable: new_rev == WORKDIR, ..Default::default() };
    if let Some(mime) = mime_of(file) {
        let enc = |b: &Vec<u8>| format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(b));
        v.old_image = old.as_ref().map(enc);
        v.new_image = new.as_ref().map(enc);
        v.binary = true;
        v.editable = false;
        return v;
    }
    // Repo-Specific > Encoding (UTF-16 files contain zero bytes, so they aren't treated as binary then).
    let enc = crate::prefs::encoding_for(dir);
    let utf16 = enc.map(|e| e == encoding_rs::UTF_16LE || e == encoding_rs::UTF_16BE).unwrap_or(false);
    let is_bin = |b: &Option<Vec<u8>>| !utf16 && b.as_ref().map(|x| is_binary(x)).unwrap_or(false);
    if is_bin(&old) || is_bin(&new) {
        v.binary = true;
        v.editable = false;
        return v;
    }
    let text = |b: Option<Vec<u8>>| {
        b.map(|x| {
            if let Some(e) = enc {
                return crate::prefs::decode(&x, e);
            }
            let x = x.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).map(|s| s.to_vec()).unwrap_or(x);
            match String::from_utf8(x) {
                Ok(s) => s,
                Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
            }
        })
    };
    v.old = text(old);
    v.new = text(new);
    v
}

/// One hunk of a unified diff: its header line and body lines (each starting with ' ', '+', '-' or '\').
struct Hunk {
    header: String,
    lines: Vec<String>,
}

fn split_diff(text: &str) -> (String, Vec<Hunk>) {
    let mut head = String::new();
    let mut hunks: Vec<Hunk> = Vec::new();
    for line in text.split_inclusive('\n') {
        let l = line.strip_suffix('\n').unwrap_or(line);
        if l.starts_with("@@") {
            hunks.push(Hunk { header: l.to_string(), lines: Vec::new() });
        } else if let Some(h) = hunks.last_mut() {
            h.lines.push(l.to_string());
        } else {
            head.push_str(l);
            head.push('\n');
        }
    }
    (head, hunks)
}

/// Builds a patch with one hunk, optionally keeping only some of its changed lines.
/// `lines` are indexes into the hunk body. For a reverse patch (unstage / discard) the
/// roles of '+' and '-' swap, as in git's own line staging.
pub fn partial_patch(diff: &str, hunk: usize, lines: Option<&[usize]>, reverse: bool) -> Option<String> {
    let (head, hunks) = split_diff(diff);
    let h = hunks.get(hunk)?;
    let mut out = head;
    out.push_str(&h.header);
    out.push('\n');
    for (i, l) in h.lines.iter().enumerate() {
        let chosen = lines.map(|ls| ls.contains(&i)).unwrap_or(true);
        let (keep_sign, ctx_sign) = if reverse { ('-', '+') } else { ('+', '-') };
        if chosen || l.starts_with(' ') || l.starts_with('\\') {
            out.push_str(l);
        } else if l.starts_with(ctx_sign) {
            // Not chosen: the line stays as it is on the side the patch is applied to.
            out.push(' ');
            out.push_str(&l[1..]);
        } else if l.starts_with(keep_sign) {
            continue; // not chosen: leave this addition out
        } else {
            out.push_str(l);
        }
        out.push('\n');
    }
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchOp {
    Stage,
    Unstage,
    Discard,
}

/// Stage, unstage or discard one hunk (or chosen lines of it). Returns the patch that was
/// applied and how, so it can be undone.
pub fn apply_part(dir: &Path, file: &str, untracked: bool, op: PatchOp, hunk: usize, lines: Option<&[usize]>) -> Result<(String, bool, bool), String> {
    if untracked {
        // Make the new file visible to `git diff` without staging its contents.
        let o = run(dir, &["add", "--intent-to-add", "--", file]);
        if !o.ok {
            return Err(o.message());
        }
    }
    let src = if op == PatchOp::Unstage { DiffSource::Staged } else { DiffSource::Unstaged };
    let diff = git::diff_text(dir, &src, file, None, false);
    let reverse = op != PatchOp::Stage;
    let patch = partial_patch(&diff, hunk, lines, reverse).ok_or("That change is no longer there. The file changed; try again.")?;
    let cached = op != PatchOp::Discard;
    apply_patch(dir, &patch, cached, reverse)?;
    Ok((patch, cached, reverse))
}

/// Hunks are checked exactly:
/// match the complete preimage bytes at the hunk position, then splice the postimage
/// before checking the next hunk. Preserve CR and the presence/absence of final LF.
/// Reverse patches exchange old/new ranges and '+'/'-' lines.
pub fn check_hunks_exact(text: &str, patch: &str, reverse: bool) -> Result<(), String> {
    fn range(value: &str) -> Option<(i64, usize)> {
        let (start, count) = value.split_once(',').unwrap_or((value, "1"));
        let start = start.parse::<i64>().ok()?;
        let count = count.parse::<usize>().ok()?;
        (start >= 0).then_some((start, count))
    }
    fn remove_final_lf(lines: &mut [String]) {
        if let Some(last) = lines.last_mut() {
            if last.ends_with('\n') {
                last.pop();
            }
        }
    }

    let mut file: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    let mut shift: i64 = 0;
    let mut lines = patch.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("@@ -") else { continue };
        let (ranges, _) = rest.split_once(" @@").ok_or("Invalid hunk header")?;
        let (old, new) = ranges.split_once(" +").ok_or("Invalid hunk header")?;
        let old = range(old).ok_or("Invalid old hunk range")?;
        let new = range(new).ok_or("Invalid new hunk range")?;
        let (start, count) = if reverse { new } else { old };
        let (keep, gone) = if reverse { (b'+', b'-') } else { (b'-', b'+') };
        let mut pre: Vec<String> = Vec::new();
        let mut post: Vec<String> = Vec::new();
        let mut previous = b' ';
        while let Some(line) = lines.peek() {
            let Some(&sign) = line.as_bytes().first() else { break };
            if !matches!(sign, b' ' | b'+' | b'-' | b'\\') {
                break;
            }
            let line = lines.next().unwrap();
            if sign == b'\\' {
                // "\ No newline at end of file" belongs to the preceding side.
                if previous == b' ' || previous == keep {
                    remove_final_lf(&mut pre);
                }
                if previous == b' ' || previous == gone {
                    remove_final_lf(&mut post);
                }
                continue;
            }
            if sign == b' ' || sign == keep {
                pre.push(line[1..].to_owned());
            }
            if sign == b' ' || sign == gone {
                post.push(line[1..].to_owned());
            }
            previous = sign;
        }
        // A zero-length range inserts AFTER start; otherwise start is one-based.
        let position = if count == 0 { start } else { (start - 1).max(0) } + shift;
        if position < 0 {
            return Err(format!("hunk at line {start} did not apply"));
        }
        let at = (position as usize).min(file.len());
        let end = at.checked_add(pre.len()).ok_or("Hunk range overflow")?;
        if file.get(at..end) != Some(pre.as_slice()) {
            return Err(format!("hunk at line {start} did not apply"));
        }
        let delta = post.len() as i64 - pre.len() as i64;
        file.splice(at..end, post);
        shift += delta;
    }
    Ok(())
}

pub fn apply_patch(dir: &Path, patch: &str, cached: bool, reverse: bool) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut args = vec!["apply", "--recount", "--whitespace=nowarn"];
    if cached {
        args.push("--cached");
    }
    if reverse {
        args.push("--reverse");
    }
    args.push("-");
    let mut child = git::git_cmd(dir)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("no stdin")?.write_all(patch.as_bytes()).map_err(|e| e.to_string())?;
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

pub fn save(dir: &Path, file: &str, content: &str) -> Result<(), String> {
    let p = dir.join(file);
    // Keep the file's existing line endings.
    let before = std::fs::read(&p).unwrap_or_default();
    let enc = crate::prefs::encoding_for(dir);
    let old_text = match enc {
        Some(e) => crate::prefs::decode(&before, e),
        None => String::from_utf8_lossy(&before).into_owned(),
    };
    let crlf = old_text.contains("\r\n");
    let text = if crlf { content.replace("\r\n", "\n").replace('\n', "\r\n") } else { content.to_string() };
    let bytes = match enc {
        Some(e) => crate::prefs::encode(&text, e, before.starts_with(&[0xFF, 0xFE]) || before.starts_with(&[0xFE, 0xFF])),
        None => text.into_bytes(),
    };
    std::fs::write(p, bytes).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct HistoryEntry {
    #[serde(flatten)]
    pub commit: Commit,
    /// The file's path in that commit (it may have been renamed since).
    pub path: String,
    pub status: char,
}

/// Commits that changed a file, newest first, following renames.
pub fn history(dir: &Path, file: &str, max: usize) -> Vec<HistoryEntry> {
    let n = format!("-n{max}");
    let o = run(dir, &["log", "--follow", &n, "--name-status", "--format=%x1e%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1d", "--", file]);
    let mut out = Vec::new();
    for rec in o.stdout.split('\x1e').filter(|r| !r.trim().is_empty()) {
        let (meta, files) = rec.split_once('\x1d').unwrap_or((rec, ""));
        let f: Vec<&str> = meta.splitn(7, '\x1f').collect();
        if f.len() < 7 {
            continue;
        }
        let mut path = file.to_string();
        let mut status = 'M';
        if let Some(line) = files.lines().find(|l| !l.trim().is_empty()) {
            let parts: Vec<&str> = line.split('\t').collect();
            status = parts[0].chars().next().unwrap_or('M');
            if let Some(p) = parts.last() {
                path = p.to_string();
            }
        }
        out.push(HistoryEntry {
            commit: Commit {
                sha: f[0].to_string(),
                parents: f[1].split_whitespace().map(|s| s.to_string()).collect(),
                author: f[2].to_string(),
                email: f[3].to_string(),
                time: f[4].parse().unwrap_or(0),
                summary: f[5].to_string(),
                body: f[6].trim().to_string(),
            },
            path,
            status,
        });
    }
    out
}

#[derive(Serialize, Clone)]
pub struct BlameLine {
    pub sha: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    pub line: usize,
    pub text: String,
}

/// Who last changed each line (`git blame --porcelain`). `rev` None = working folder.
pub fn blame(dir: &Path, file: &str, rev: Option<&str>) -> Result<Vec<BlameLine>, String> {
    let mut args = vec!["blame", "--porcelain"];
    if let Some(r) = rev {
        args.push(r);
    }
    args.push("--");
    args.push(file);
    let o = run(dir, &args);
    if !o.ok {
        return Err(o.message());
    }
    let mut info: std::collections::HashMap<String, (String, i64, String)> = Default::default();
    let mut out = Vec::new();
    let mut cur_sha = String::new();
    let mut cur_line = 0usize;
    let mut author = String::new();
    let mut time = 0i64;
    let mut summary = String::new();
    for l in o.stdout.lines() {
        if let Some(t) = l.strip_prefix('\t') {
            let (a, tm, s) = info
                .entry(cur_sha.clone())
                .or_insert_with(|| (author.clone(), time, summary.clone()))
                .clone();
            out.push(BlameLine { sha: cur_sha.clone(), author: a, time: tm, summary: s, line: cur_line, text: t.to_string() });
        } else if let Some(v) = l.strip_prefix("author ") {
            author = v.to_string();
        } else if let Some(v) = l.strip_prefix("author-time ") {
            time = v.parse().unwrap_or(0);
        } else if let Some(v) = l.strip_prefix("summary ") {
            summary = v.to_string();
        } else {
            let mut it = l.split_whitespace();
            if let (Some(sha), Some(_orig), Some(fin)) = (it.next(), it.next(), it.next()) {
                if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
                    cur_sha = sha.to_string();
                    cur_line = fin.parse().unwrap_or(0);
                    if let Some((a, tm, s)) = info.get(&cur_sha) {
                        author = a.clone();
                        time = *tm;
                        summary = s.clone();
                    }
                }
            }
        }
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct Stats {
    pub sha: String,
    pub files: u32,
    pub added: u32,
    pub deleted: u32,
}

/// Files changed, lines added and removed for each commit (the graph's CHANGES column).
pub fn stats(dir: &Path, shas: &[String]) -> Vec<Stats> {
    if shas.is_empty() {
        return Vec::new();
    }
    let mut args: Vec<&str> = vec!["log", "--no-walk=unsorted", "--diff-merges=first-parent", "--shortstat", "--format=%x1e%H"];
    args.extend(shas.iter().map(|s| s.as_str()));
    let o = run(dir, &args);
    let mut out = Vec::new();
    for rec in o.stdout.split('\x1e').filter(|r| !r.trim().is_empty()) {
        let mut lines = rec.lines().filter(|l| !l.trim().is_empty());
        let sha = lines.next().unwrap_or("").trim().to_string();
        let mut s = Stats { sha, files: 0, added: 0, deleted: 0 };
        if let Some(stat) = lines.next() {
            for part in stat.split(',') {
                let n: u32 = part.trim().split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
                if part.contains("file") {
                    s.files = n;
                } else if part.contains("insertion") {
                    s.added = n;
                } else if part.contains("deletion") {
                    s.deleted = n;
                }
            }
        }
        out.push(s);
    }
    out
}

/// Files that differ between two points (`to` may be WORKDIR).
pub fn range_files(dir: &Path, from: &str, to: &str) -> Vec<FileChange> {
    let mut args = vec!["diff", "--name-status", "-z", "-M", from];
    if to != WORKDIR {
        args.push(to);
    }
    let o = run(dir, &args);
    let mut out = Vec::new();
    let mut it = o.stdout.split('\0').filter(|s| !s.is_empty());
    while let Some(st) = it.next() {
        let ch = st.chars().next().unwrap_or('M');
        if ch == 'R' || ch == 'C' {
            let _old = it.next();
            if let Some(p) = it.next() {
                out.push(FileChange { path: p.to_string(), status: 'R' });
            }
        } else if let Some(p) = it.next() {
            out.push(FileChange { path: p.to_string(), status: ch });
        }
    }
    out
}

/// Every tracked file (for the file history finder).
pub fn tracked_files(dir: &Path) -> Vec<String> {
    run(dir, &["ls-files"]).stdout.lines().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,3 +1,4 @@\n a\n-b\n+B\n+new\n c\n@@ -9,1 +10,1 @@\n-x\n+y\n";

    #[test]
    fn binary_test_matches_libgit2() {
        assert!(!is_binary(b"hello
world
	ok"));
        assert!(is_binary(b"abc def"));
        assert!(is_binary(&[0xFF, 0xFE, b'a', 0]));
        assert!(!is_binary(&[0xEF, 0xBB, 0xBF, b'a', b'b']));
        // 128 printable characters allow one control character (128 >> 7 = 1, and 1 < 1 is false)...
        let mut t = vec![b'a'; 128];
        t.push(1);
        assert!(!is_binary(&t));
        // ...but not two.
        t.push(2);
        assert!(is_binary(&t));
        // ESC, BS and FF count as printable; TAB, LF, VT and CR are neutral.
        assert!(!is_binary(&[27, 8, 12]));
        assert!(!is_binary(&[9, 10, 11, 13]));
        assert!(is_binary(&[14]));
        // Only the first 8000 bytes are looked at.
        let mut big = vec![b'a'; 8000];
        big.push(0);
        assert!(!is_binary(&big));
    }

    #[test]
    fn hunks_apply_only_at_their_own_line() {
        let patch = "--- a/f\n+++ b/f\n@@ -2,2 +2,2 @@\n b\n-c\n+C\n";
        // Forward: lines 2-3 must be "b", "c".
        assert!(check_hunks_exact("a\nb\nc\nd\n", patch, false).is_ok());
        // The same text one line lower would be found by git apply, but not by libgit2.
        assert_eq!(check_hunks_exact("x\na\nb\nc\nd\n", patch, false).unwrap_err(), "hunk at line 2 did not apply");
        // Reverse (revert): match the new side, including its actual line endings.
        assert!(check_hunks_exact("a\nb\nC\nd\n", patch, true).is_ok());
        assert!(check_hunks_exact("a\r\nb\r\nC\r\nd\r\n", patch, true).is_err());
        let crlf_patch = "--- a/f\n+++ b/f\n@@ -2,2 +2,2 @@\n b\r\n-c\r\n+C\r\n";
        assert!(check_hunks_exact("a\r\nb\r\nC\r\nd\r\n", crlf_patch, true).is_ok());
    }

    #[test]
    fn exact_hunks_follow_insertions_and_deletions() {
        let inserted = "--- a/f\n+++ b/f\n@@ -1,1 +1,2 @@\n a\n+inserted\n@@ -3,1 +4,1 @@\n-c\n+C\n";
        assert!(check_hunks_exact("a\nb\nc\n", inserted, false).is_ok());
        assert!(check_hunks_exact("a\ninserted\nb\nC\n", inserted, true).is_ok());
        let deleted = "--- a/f\n+++ b/f\n@@ -1,2 +1,1 @@\n a\n-b\n@@ -4,1 +3,1 @@\n-d\n+D\n";
        assert!(check_hunks_exact("a\nb\nc\nd\n", deleted, false).is_ok());
        assert!(check_hunks_exact("a\nc\nD\n", deleted, true).is_ok());
        let insert_after_first = "--- a/f\n+++ b/f\n@@ -1,0 +2,1 @@\n+inserted\n@@ -2,1 +3,1 @@\n-b\n+B\n";
        assert!(check_hunks_exact("a\nb\n", insert_after_first, false).is_ok());
    }

    #[test]
    fn exact_hunks_preserve_final_newline_and_patch_bytes() {
        let patch = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n";
        assert!(check_hunks_exact("old", patch, false).is_ok());
        assert!(check_hunks_exact("old\n", patch, false).is_err());
        assert!(check_hunks_exact("new\n", patch, true).is_ok());
        assert!(check_hunks_exact("new", patch, true).is_err());
        let crlf = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\r\n+new\r\n";
        let partial = partial_patch(crlf, 0, None, false).unwrap();
        assert!(partial.contains("-old\r\n+new\r\n"));
        assert!(check_hunks_exact("old\r\n", &partial, false).is_ok());
        assert!(check_hunks_exact("", "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-\n+x\n", false).is_err());
    }

    #[test]
    fn whole_hunk_patch() {
        let p = partial_patch(DIFF, 1, None, false).unwrap();
        assert!(p.starts_with("diff --git"));
        assert!(p.contains("@@ -9,1 +10,1 @@\n-x\n+y\n"));
        assert!(!p.contains("+new"));
    }

    #[test]
    fn line_patch_forward_and_reverse() {
        // body indexes: 0 " a", 1 "-b", 2 "+B", 3 "+new", 4 " c"; choose only "+new"
        let p = partial_patch(DIFF, 0, Some(&[3]), false).unwrap();
        assert!(p.contains(" a\n b\n+new\n c\n"), "{p}");
        let r = partial_patch(DIFF, 0, Some(&[3]), true).unwrap();
        // reverse: unchosen "+B" is already there (context), unchosen "-b" is gone (dropped)
        assert!(r.contains(" a\n B\n+new\n c\n"), "{r}");
        assert!(!r.contains("-b"));
    }
}

/// A file inside the repository's working folder. Refuses absolute paths and `..`, so the file actions below never
/// reach outside the repository.
pub fn repo_file(dir: &Path, file: &str) -> Result<std::path::PathBuf, String> {
    let rel = Path::new(file);
    if file.is_empty() || rel.is_absolute() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(format!("\"{file}\" is not a file in this repository"));
    }
    Ok(dir.join(rel))
}

/// Largest file "Copy file contents" puts on the clipboard.
pub const COPY_CONTENTS_MAX: usize = 10 * 1024 * 1024;

/// The text of a file, from the working folder or (with `rev`) as it was in a commit, for "Copy file contents".
/// Binary files are refused by the same rule as the diff view (is_binary).
pub fn file_text(dir: &Path, file: &str, rev: Option<&str>) -> Result<String, String> {
    let path = repo_file(dir, file)?;
    let bytes = match rev {
        Some(rev) if !rev.is_empty() => {
            let file = file.replace('\\', "/");
            let spec = if rev == ":" { format!(":{file}") } else { format!("{rev}:{file}") };
            let o = git::git_cmd(dir).args(["show", &spec]).output().map_err(|e| e.to_string())?;
            if !o.status.success() {
                return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
            }
            o.stdout
        }
        _ => std::fs::read(&path).map_err(|e| e.to_string())?,
    };
    if bytes.len() > COPY_CONTENTS_MAX {
        return Err("This file is too large to copy.".into());
    }
    if is_binary(&bytes) {
        return Err("This is a binary file, so it can't be copied as text.".into());
    }
    Ok(match crate::prefs::encoding_for(dir) {
        Some(e) => crate::prefs::decode(&bytes, e),
        None => String::from_utf8_lossy(&bytes).into_owned(),
    })
}

#[cfg(test)]
mod file_action_tests {
    use super::*;

    #[test]
    fn clipboard_contents_distinguish_index_worktree_and_saved_commit() {
        let dir = std::env::temp_dir().join(format!("gitcraft-copy-index-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for args in [&["init", "-q"][..], &["config", "user.name", "Test"], &["config", "user.email", "t@example.com"], &["config", "commit.gpgsign", "false"]] {
            assert!(git::run(&dir, args).ok);
        }
        std::fs::write(dir.join("a.txt"), "saved\n").unwrap();
        assert!(git::run(&dir, &["add", "."]).ok);
        assert!(git::run(&dir, &["commit", "-qm", "saved"]).ok);
        std::fs::write(dir.join("a.txt"), "staged\n").unwrap();
        assert!(git::run(&dir, &["add", "."]).ok);
        std::fs::write(dir.join("a.txt"), "working\n").unwrap();
        assert_eq!(file_text(&dir, "a.txt", Some(":")).unwrap().replace("\r\n", "\n"), "staged\n");
        assert_eq!(file_text(&dir, "a.txt", None).unwrap().replace("\r\n", "\n"), "working\n");
        assert_eq!(file_text(&dir, "a.txt", Some("HEAD")).unwrap().replace("\r\n", "\n"), "saved\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_file_stays_inside_the_repository() {
        let dir = Path::new("C:/repo");
        assert!(repo_file(dir, "src/main.rs").is_ok());
        assert!(repo_file(dir, "../secret.txt").is_err());
        assert!(repo_file(dir, "src/../../x").is_err());
        assert!(repo_file(dir, "").is_err());
        assert!(repo_file(dir, "C:/Windows/win.ini").is_err());
        assert!(repo_file(dir, "/etc/passwd").is_err());
    }
}
