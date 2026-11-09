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

/// libgit2's binary test as GitKraken runs it (nodegit.node git_blob_is_binary 0x180257b80,
/// text test 0x1802e5380, byte-order-mark check 0x1802e5430):
/// look at the first 8000 bytes; a UTF-16 / UTF-32 byte-order mark means binary; a UTF-8 one is
/// skipped; a NUL byte means binary; otherwise binary when (printable >> 7) < non-printable.
/// Tab, LF and CR count as neither; BS, FF and ESC count as printable.
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
            9 | 10 | 13 => {}
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
    let is_bin = |b: &Option<Vec<u8>>| b.as_ref().map(|x| is_binary(x)).unwrap_or(false);
    if is_bin(&old) || is_bin(&new) {
        v.binary = true;
        v.editable = false;
        return v;
    }
    let text = |b: Option<Vec<u8>>| {
        b.map(|x| {
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
        let l = line.trim_end_matches(['\n', '\r']);
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

/// libgit2's hunk rule (nodegit.node 0x180245620): a hunk applies only at its own position - the old start
/// line, moved by the lines earlier hunks added or removed - and every preimage line (context and removed lines)
/// must match there exactly. Otherwise: "hunk at line N did not apply". Git's own `apply` would search nearby
/// lines instead, so GitCraft checks this first. For a reverse patch the preimage is the context and '+' lines,
/// starting at the new start line.
pub fn check_hunks_exact(text: &str, patch: &str, reverse: bool) -> Result<(), String> {
    let file: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let mut shift: i64 = 0;
    let mut lines = patch.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("@@ -") else { continue };
        let nums: Vec<i64> = rest
            .split(|c: char| c == ' ' || c == ',' || c == '+')
            .take_while(|t| *t != "@@")
            .filter_map(|t| t.parse().ok())
            .collect();
        // "-a,b +c,d" (counts may be missing when they are 1).
        let (old_start, new_start) = match nums.len() {
            4 => (nums[0], nums[2]),
            3 => (nums[0], if rest.split(' ').next().unwrap_or("").contains(',') { nums[2] } else { nums[1] }),
            2 => (nums[0], nums[1]),
            _ => continue,
        };
        let start = if reverse { new_start } else { old_start };
        let (keep, gone) = if reverse { ('+', '-') } else { ('-', '+') };
        let mut pre = Vec::new();
        let mut post_len = 0i64;
        while let Some(l) = lines.peek() {
            if l.starts_with("@@") || l.starts_with("diff ") {
                break;
            }
            let l = lines.next().unwrap_or_default();
            if l.starts_with(' ') || l.starts_with(keep) {
                pre.push(&l[1..]);
            }
            if l.starts_with(' ') || l.starts_with(gone) {
                post_len += 1;
            }
        }
        if start == 0 && pre.is_empty() {
            shift += post_len;
            continue;
        }
        let at = start - 1 + shift;
        let ok = at >= 0 && pre.iter().enumerate().all(|(i, p)| file.get((at as usize) + i).map(|f| f == p).unwrap_or(false));
        if !ok {
            return Err(format!("hunk at line {start} did not apply"));
        }
        shift += post_len - pre.len() as i64;
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
    let crlf = std::fs::read(&p).map(|b| b.windows(2).any(|w| w == b"\r\n")).unwrap_or(false);
    let text = if crlf { content.replace("\r\n", "\n").replace('\n', "\r\n") } else { content.to_string() };
    std::fs::write(p, text).map_err(|e| e.to_string())
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
        // ESC, BS and FF count as printable.
        assert!(!is_binary(&[27, 8, 12]));
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
        // Reverse (revert): the file must hold "b", "C" at the new start line; CRLF files compare the same.
        assert!(check_hunks_exact("a\r\nb\r\nC\r\nd\r\n", patch, true).is_ok());
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
