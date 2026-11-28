//! Compact bulk dependency imports without hiding application or local patch changes.
use std::collections::BTreeMap;

pub(crate) fn sensitive_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.split('/').any(|part| matches!(part, ".ssh" | ".aws" | ".gnupg" | ".secrets")) { return true; }
    let name = lower.rsplit('/').next().unwrap_or_default();
    name == ".env" || name.starts_with(".env.") || name.starts_with("id_") ||
        matches!(name, ".npmrc" | ".pypirc" | ".netrc" | "_netrc" | ".git-credentials" | "credentials.json" | "secrets.json") ||
        [".pem", ".key", ".pfx", ".p12", ".jks", ".keystore"].iter().any(|suffix| name.ends_with(suffix))
}

pub(crate) fn redact(text: &str) -> std::borrow::Cow<'_, str> {
    static SENSITIVE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = SENSITIVE.get_or_init(|| regex::Regex::new(concat!(
        r"(?s:-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z))",
        r"|(?:gh[pousr]_|github_pat_|glpat-|sk-(?:proj-|ant-)?)[A-Za-z0-9_-]{8,}",
        r"|(?:AKIA|ASIA)[A-Z0-9]{16}",
        r#"|(?im)^[+\- ]*['"]?[a-z0-9_.-]*(?:api[_.-]?key|password|passwd|secret|token|authorization)['"]?[ \t]*[:=][^\r\n]+"#,
    )).expect("valid redaction pattern"));
    pattern.replace_all(text, "[redacted]")
}

fn dependency(path: &str) -> Option<String> {
    let parts: Vec<_> = path.split('/').collect();
    let i = parts.iter().position(|p| matches!(*p, "vendor" | "vendored" | "third_party" | "third-party"))?;
    if parts.len() <= i + 2 { return None; }
    Some(parts[..i + 2].join("/"))
}

fn retain_import(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    name.ends_with(".md") || name.ends_with(".patch") || name.ends_with(".diff") ||
        ["cargo.toml", "cargo.toml.orig", "package.json", "pyproject.toml", "go.mod"].contains(&name.as_str()) ||
        ["license", "copying", "copyright", "notice", "readme"].iter().any(|prefix| name.starts_with(prefix)) ||
        name.contains("patch") || name.contains("changelog")
}

fn context_priority(path: &str) -> u8 {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    if ["cargo.toml", "package.json", "pyproject.toml", "go.mod"].contains(&name.as_str()) { 0 }
    else if ["cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "go.sum"].contains(&name.as_str()) { 3 }
    else if dependency(path).is_none() { 1 } else { 2 }
}

pub(crate) const CONTEXT_BUDGET: usize = 256 * 1024;
const DETAIL_PER_FILE: usize = 32 * 1024;
const CONTENT_BUDGET: usize = CONTEXT_BUDGET - 1024;

fn append_context(output: &mut String, text: &str) -> bool {
    append_limited(output, text, CONTENT_BUDGET)
}

pub(crate) fn append_limited(output: &mut String, text: &str, budget: usize) -> bool {
    let available = budget.saturating_sub(output.len());
    let mut end = text.len().min(available);
    while !text.is_char_boundary(end) { end -= 1; }
    output.push_str(&text[..end]);
    end < text.len()
}

fn bounded_patch(dir: &std::path::Path, args: &[&str], limit: usize) -> Result<(String, bool), String> {
    use std::io::Read;
    let mut command = crate::git::git_cmd(dir);
    command.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let (mut child, tree) = crate::process_tree::ProcessTree::spawn(&mut command).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let read = child.stdout.take().unwrap().take(limit as u64 + 1).read_to_end(&mut bytes);
    let truncated = bytes.len() > limit;
    if truncated || read.is_err() { tree.kill(); let _ = child.kill(); }
    let status = child.wait().map_err(|e| e.to_string())?;
    read.map_err(|e| e.to_string())?;
    if !status.success() && !truncated { return Err("Could not read the selected changes for AI context.".into()); }
    bytes.truncate(limit);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

/// Classify changes before asking Git for any patch text. Bulk dependency files
/// never enter the patch output buffer. Literal pathspecs preserve unusual names.
pub(crate) fn diff(dir: &std::path::Path, args: &[&str]) -> Result<String, String> {
    diff_limited(dir, args, CONTENT_BUDGET)
}

fn change_paths(dir: &std::path::Path, args: &[&str]) -> Result<Vec<(char, String, Option<String>)>, String> {
    let mut names = args.to_vec();
    names.extend(["--name-status", "-z", "--no-color", "--no-ext-diff", "--no-textconv"]);
    let listed = crate::git::run(dir, &names);
    if !listed.ok { return Err(listed.message()); }
    let mut records = listed.stdout.split('\0').filter(|r| !r.is_empty());
    let mut paths = Vec::new();
    while let Some(status) = records.next() {
        let Some(path) = records.next() else { break };
        let Some(kind) = status.trim().chars().next() else { continue };
        let new = if matches!(kind, 'R' | 'C') { records.next() } else { None };
        if sensitive_path(path) || new.is_some_and(sensitive_path) || matches!(kind, 'R' | 'C') && new.is_none() {
            continue;
        }
        paths.push((kind, path.to_owned(), new.map(str::to_owned)));
    }
    Ok(paths)
}

pub(crate) fn diff_limited(dir: &std::path::Path, args: &[&str], budget: usize) -> Result<String, String> {
    let mut retained = Vec::new();
    let mut omitted = BTreeMap::new();
    for (kind, path, new) in change_paths(dir, args)? {
        if let Some(new) = new {
            retained.extend([path, new]);
        } else if matches!(kind, 'A' | 'D') && dependency(&path).is_some() && !retain_import(&path) {
            omitted.insert(path, if kind == 'A' { "added" } else { "removed" });
        } else { retained.push(path); }
    }
    let mut output = String::new();
    // Keep each Windows command line bounded even in repositories with many files.
    let mut batch = Vec::new();
    let mut size = 0;
    let flush = |batch: &mut Vec<String>, output: &mut String| -> Result<(), String> {
        if batch.is_empty() { return Ok(()); }
        let mut command = args.to_vec();
        command.extend(["--patch", "--no-color", "--no-ext-diff", "--no-textconv", "--"]);
        command.extend(batch.iter().map(String::as_str));
        let available = budget.saturating_sub(output.len());
        if available > 0 {
            let (text, truncated) = bounded_patch(dir, &command, available)?;
            append_limited(output, &text, budget);
            if truncated { append_limited(output, "\n[Detailed patch excerpt ends here.]\n", budget); }
        }
        batch.clear();
        Ok(())
    };
    retained.sort_by_key(|path| (context_priority(path), path.clone()));
    let mut priority = None;
    let retained_count = retained.len();
    for path in retained {
        let next_priority = context_priority(&path);
        if priority.is_some_and(|previous| previous != next_priority) { flush(&mut batch, &mut output)?; size = 0; }
        priority = Some(next_priority);
        if size + path.len() > 12000 { flush(&mut batch, &mut output)?; size = 0; }
        let spec = format!(":(literal){path}");
        size += spec.len() + 3;
        batch.push(spec);
    }
    flush(&mut batch, &mut output)?;
    if !omitted.is_empty() && output.len() < budget.saturating_sub(4) {
        let mut counts = args.to_vec();
        counts.extend(["--numstat", "-z", "--no-color", "--no-ext-diff", "--no-textconv"]);
        let stats = crate::git::run(dir, &counts);
        if !stats.ok { return Err(stats.message()); }
        append_limited(&mut output, "\nVendored dependency summary (bulk source omitted; retained details may be excerpted). Do not infer omitted implementation details:\n", budget);
        for record in stats.stdout.split('\0') {
            let mut fields = record.trim_start_matches('\n').splitn(3, '\t');
            if let (Some(add), Some(remove), Some(path)) = (fields.next(), fields.next(), fields.next()) {
                if let Some(status) = omitted.get(path) {
                    append_limited(&mut output, &format!("- {}: {status}, +{add}/-{remove} lines\n", serde_json::to_string(path).unwrap()), budget);
                }
            }
        }
    }
    if output.len() >= budget.saturating_sub(4) {
        output.push_str(&format!("\n[AI context budget reached across {retained_count} detailed paths and {} bulk dependency paths. Patch details above are incomplete excerpts; do not infer omitted code.]\n", omitted.len()));
    }
    Ok(output)
}

/// Count all selected files and highlight representative paths without reading their contents.
pub(crate) fn summarize_changes(dir: &std::path::Path, args: &[&str], approved_files: &[String]) -> Result<(usize, String), String> {
    let mut paths = if args.is_empty() { Vec::new() } else { change_paths(dir, args)? };
    paths.extend(approved_files.iter().filter(|path| !sensitive_path(path)).map(|path| ('A', path.clone(), None)));
    let mut groups = BTreeMap::<&str, usize>::new();
    let mut kinds = BTreeMap::<&str, usize>::new();
    let action = |kind| match kind { 'A' => "added", 'D' => "deleted", 'R' => "renamed", 'C' => "copied", _ => "modified" };
    for (kind, old, new) in &paths {
        let path = new.as_ref().unwrap_or(old);
        let group = path.split_once('/').map_or("(repository root)", |(group, _)| group);
        *groups.entry(group).or_default() += 1;
        *kinds.entry(action(*kind)).or_default() += 1;
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by_key(|(group, count)| (std::cmp::Reverse(*count), *group));
    let counts = kinds.into_iter().map(|(kind, count)| format!("{count} {kind}")).collect::<Vec<_>>().join(", ");
    let mut summary = format!("{} changed files ({counts}).\n", paths.len());
    for (group, count) in groups.iter().take(12) {
        append_limited(&mut summary, &format!("- {}: {count} files\n", serde_json::to_string(group).unwrap()), 1024);
    }
    if groups.len() > 12 {
        append_limited(&mut summary, &format!("- {} additional directories\n", groups.len() - 12), 1200);
    }
    paths.sort_by_key(|(_, old, new)| context_priority(new.as_deref().unwrap_or(old)));
    summary.push_str("\nRepresentative paths:\n");
    for (kind, old, new) in paths.iter().take(24) {
        let path = new.as_ref().unwrap_or(old);
        if append_limited(&mut summary, &format!("- {} {}\n", action(*kind), serde_json::to_string(path).unwrap()), 3000) {
            break;
        }
    }
    Ok((paths.len(), summary))
}

#[derive(Default)]
pub struct Untracked {
    pub files: Vec<String>,
    pub text: String,
}

fn collect_untracked(dir: &std::path::Path) -> Result<Untracked, String> {
    use std::io::Read;
    let result = crate::git::run(dir, &["ls-files", "--others", "--exclude-standard", "-z"]);
    if !result.ok { return Err(result.message()); }
    let root = dir.canonicalize().map_err(|error| error.to_string())?;
    let mut paths: Vec<_> = result.stdout.split('\0').filter(|path| !path.is_empty() && !sensitive_path(path)).collect();
    paths.sort_by_key(|path| (context_priority(path), *path));
    let mut context = Untracked::default();
    let mut summarized = 0usize;
    let mut total = 0u64;
    for path in paths {
        let full = crate::files::repo_file(&root, path)?;
        let Ok(metadata) = std::fs::symlink_metadata(&full) else { continue; };
        if !metadata.is_file() || dependency(path).is_some() && !retain_import(path) { continue; }
        total += metadata.len();
        if context.text.len() >= CONTENT_BUDGET || context.files.len() >= 200 { summarized += 1; continue; }
        let Ok(full) = full.canonicalize() else { continue; };
        if !full.starts_with(&root) { continue; }
        let header = format!("\nNew (untracked) file {} ({} bytes):\n", serde_json::to_string(path).unwrap(), metadata.len());
        let limit = DETAIL_PER_FILE.min(CONTENT_BUDGET.saturating_sub(context.text.len() + header.len()));
        if limit == 0 { summarized += 1; continue; }
        let mut bytes = Vec::new();
        std::fs::File::open(&full).map_err(|error| error.to_string())?.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
        if crate::files::is_binary(&bytes) { continue; }
        let excerpted = bytes.len() > limit || metadata.len() > limit as u64;
        bytes.truncate(limit);
        let text = if let Some(encoding) = crate::prefs::encoding_for(dir) { crate::prefs::decode(&bytes, encoding) } else { String::from_utf8_lossy(&bytes).into_owned() };
        context.files.push(path.to_owned());
        append_context(&mut context.text, &header);
        append_context(&mut context.text, &redact(&text));
        if excerpted { summarized += 1; append_context(&mut context.text, "\n[File excerpt only; remaining contents omitted.]\n"); }
    }
    if summarized > 0 && !context.files.is_empty() { context.text.push_str(&format!("\n[AI context budget: {summarized} new files have excerpted or omitted contents; total new-file size is {total} bytes.]\n")); }
    Ok(context)
}

#[cfg(test)]
pub(crate) fn untracked(dir: &std::path::Path) -> Result<String, String> {
    Ok(collect_untracked(dir)?.text)
}

#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind { Commit, Stash, Compose }

#[derive(serde::Serialize)]
pub struct Preview { pub id: Option<u32>, pub files: Vec<String> }

struct Pending {
    id: u32,
    dir: std::path::PathBuf,
    kind: PreviewKind,
    context: Untracked,
    created: std::time::Instant,
}

static PREVIEWS: std::sync::Mutex<Vec<Pending>> = std::sync::Mutex::new(Vec::new());
const PREVIEW_TTL: std::time::Duration = std::time::Duration::from_secs(600);

pub fn preview(dir: &std::path::Path, kind: PreviewKind) -> Result<Preview, String> {
    if kind != PreviewKind::Compose && !diff_limited(dir, &["diff", "--cached"], 1024)?.trim().is_empty() {
        return Ok(Preview { id: None, files: vec![] });
    }
    let context = collect_untracked(dir)?;
    if context.files.is_empty() { return Ok(Preview { id: None, files: vec![] }); }
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let files = context.files.clone();
    let dir = dir.canonicalize().map_err(|error| error.to_string())?;
    let mut pending = PREVIEWS.lock().unwrap_or_else(|error| error.into_inner());
    pending.retain(|item| item.created.elapsed() <= PREVIEW_TTL);
    if pending.len() >= 8 { pending.remove(0); }
    pending.push(Pending { id, dir, kind, context, created: std::time::Instant::now() });
    Ok(Preview { id: Some(id), files })
}

pub fn take_preview(dir: &std::path::Path, kind: PreviewKind, id: Option<u32>) -> Result<Untracked, String> {
    let Some(id) = id else { return Ok(Untracked::default()); };
    let dir = dir.canonicalize().map_err(|error| error.to_string())?;
    let mut pending = PREVIEWS.lock().unwrap_or_else(|error| error.into_inner());
    pending.retain(|item| item.created.elapsed() <= PREVIEW_TTL);
    let index = pending.iter().position(|item| item.id == id && item.dir == dir && item.kind == kind).ok_or("The AI file preview expired. Review the files again before sending.")?;
    Ok(pending.remove(index).context)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let folder = tempfile::tempdir().unwrap();
        for args in [vec!["init", "-q"], vec!["config", "user.name", "Test"], vec!["config", "user.email", "test@example.invalid"], vec!["config", "commit.gpgsign", "false"], vec!["commit", "--allow-empty", "-qm", "base"]] {
            assert!(crate::git::run(folder.path(), &args).ok);
        }
        folder
    }
    #[test]
    fn only_reviewed_sanitized_untracked_content_is_sent() {
        let folder = fixture(); let dir = folder.path();
        std::fs::write(dir.join(".env"), "denylisted content").unwrap();
        std::fs::write(dir.join("notes.txt"), "safe text ghp_syntheticExampleToken").unwrap();
        let review = preview(dir, PreviewKind::Commit).unwrap();
        assert_eq!(review.files, ["notes.txt"]);
        std::fs::write(dir.join("notes.txt"), "later change").unwrap();
        assert!(take_preview(dir, PreviewKind::Stash, review.id).is_err());
        let approved = take_preview(dir, PreviewKind::Commit, review.id).unwrap();
        assert!(approved.text.contains("safe text [redacted]"));
        assert!(!approved.text.contains("syntheticExampleToken"));
        assert!(!approved.text.contains("denylisted content"));
        assert!(!approved.text.contains("later change"));
        assert!(take_preview(dir, PreviewKind::Commit, review.id).is_err());
    }
    #[test]
    fn existing_vendor_edits_and_literal_unicode_files_keep_their_contents() {
        let folder = fixture(); let dir = folder.path();
        std::fs::create_dir_all(dir.join("vendor/pkg/src")).unwrap();
        std::fs::write(dir.join("vendor/pkg/src/lib.rs"), "original\n").unwrap();
        assert!(crate::git::run(dir, &["add", "."]).ok);
        assert!(crate::git::run(dir, &["commit", "-qm", "dependency"]).ok);
        std::fs::write(dir.join("vendor/pkg/src/lib.rs"), "security fix\n").unwrap();
        std::fs::write(dir.join("space café.txt"), "application content\n").unwrap();
        assert!(crate::git::run(dir, &["add", "."]).ok);
        let prompt = diff(dir, &["diff", "HEAD"]).unwrap();
        assert!(prompt.contains("+security fix"));
        assert!(prompt.contains("+application content"));
    }
    #[test]
    fn removed_vendor_source_is_counted_and_untracked_text_is_read_without_staging() {
        let folder = fixture(); let dir = folder.path();
        std::fs::create_dir_all(dir.join("vendor/pkg/src")).unwrap();
        std::fs::write(dir.join("vendor/pkg/src/lib.rs"), "old bulk source\n".repeat(1000)).unwrap();
        assert!(crate::git::run(dir, &["add", "."]).ok);
        assert!(crate::git::run(dir, &["commit", "-qm", "dependency"]).ok);
        std::fs::remove_file(dir.join("vendor/pkg/src/lib.rs")).unwrap();
        let prompt = diff(dir, &["diff", "HEAD"]).unwrap();
        assert!(prompt.contains("removed, +0/-1000 lines"));
        assert!(!prompt.contains("old bulk source"));
        std::fs::write(dir.join("new-file.rs"), "new implementation\n").unwrap();
        let status = crate::git::run(dir, &["status", "--porcelain"]).stdout;
        assert!(untracked(dir).unwrap().contains("new implementation"));
        assert_eq!(crate::git::run(dir, &["status", "--porcelain"]).stdout, status);
    }
}


#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn ordinary_new_files_are_excerpted_with_an_aggregate_bound() {
        let folder=tempfile::tempdir().unwrap();let dir=folder.path();assert!(crate::git::run(dir,&["init","-q"]).ok);
        let contents="x".repeat(1024*1024);
        for index in 0..32 {std::fs::write(dir.join(format!("ordinary-{index:02}.txt")),&contents).unwrap();}
        let result=untracked(dir).unwrap();assert!(result.len()<=CONTEXT_BUDGET);assert!(result.contains("excerpted or omitted"));assert!(result.contains("33554432"));
        assert!(crate::git::run(dir,&["ls-files"]).stdout.is_empty());
        assert!(crate::git::run(dir,&["add","."]).ok);
        let result=diff(dir,&["diff","--cached"]).unwrap();assert!(result.len()<=CONTEXT_BUDGET);assert!(result.contains("budget reached"));
    }
}
