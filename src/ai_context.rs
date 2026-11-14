//! Compact bulk dependency imports without hiding application or local patch changes.
use std::collections::BTreeMap;

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
    let available = CONTENT_BUDGET.saturating_sub(output.len());
    let mut end = text.len().min(available);
    while !text.is_char_boundary(end) { end -= 1; }
    output.push_str(&text[..end]);
    end < text.len()
}

fn bounded_patch(dir: &std::path::Path, args: &[&str], limit: usize) -> Result<(String, bool), String> {
    use std::io::Read;
    let mut child = crate::git::git_cmd(dir).args(args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let read = child.stdout.take().unwrap().take(limit as u64 + 1).read_to_end(&mut bytes);
    let truncated = bytes.len() > limit;
    if truncated || read.is_err() { let _ = child.kill(); }
    let status = child.wait().map_err(|e| e.to_string())?;
    read.map_err(|e| e.to_string())?;
    if !status.success() && !truncated { return Err("Could not read the selected changes for AI context.".into()); }
    bytes.truncate(limit);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

/// Classify changes before asking Git for any patch text. Bulk dependency files
/// never enter the patch output buffer. Literal pathspecs preserve unusual names.
pub(crate) fn diff(dir: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let mut names = args.to_vec();
    names.extend(["--name-status", "-z", "--no-color", "--no-ext-diff", "--no-textconv"]);
    let listed = crate::git::run(dir, &names);
    if !listed.ok { return Err(listed.message()); }
    let mut records = listed.stdout.split('\0').filter(|r| !r.is_empty());
    let mut retained = Vec::new();
    let mut omitted = BTreeMap::new();
    while let Some(status) = records.next() {
        let Some(path) = records.next() else { break };
        let status = status.trim();
        if status.starts_with('R') || status.starts_with('C') {
            retained.push(path.to_owned());
            if let Some(new) = records.next() { retained.push(new.to_owned()); }
        } else if matches!(status, "A" | "D") && dependency(path).is_some() && !retain_import(path) {
            omitted.insert(path.to_owned(), if status == "A" { "added" } else { "removed" });
        } else { retained.push(path.to_owned()); }
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
        let available = CONTENT_BUDGET.saturating_sub(output.len());
        if available > 0 {
            let (text, truncated) = bounded_patch(dir, &command, available)?;
            append_context(output, &text);
            if truncated { append_context(output, "\n[Detailed patch excerpt ends here.]\n"); }
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
    if !omitted.is_empty() {
        let mut counts = args.to_vec();
        counts.extend(["--numstat", "-z", "--no-color", "--no-ext-diff", "--no-textconv"]);
        let stats = crate::git::run(dir, &counts);
        if !stats.ok { return Err(stats.message()); }
        append_context(&mut output, "\nVendored dependency summary (bulk source omitted; retained details may be excerpted). Do not infer omitted implementation details:\n");
        for record in stats.stdout.split('\0') {
            let mut fields = record.trim_start_matches('\n').splitn(3, '\t');
            if let (Some(add), Some(remove), Some(path)) = (fields.next(), fields.next(), fields.next()) {
                if let Some(status) = omitted.get(path) {
                    append_context(&mut output, &format!("- {}: {status}, +{add}/-{remove} lines\n", serde_json::to_string(path).unwrap()));
                }
            }
        }
    }
    if output.len() >= CONTENT_BUDGET {
        output.push_str(&format!("\n[AI context budget reached across {retained_count} detailed paths and {} bulk dependency paths. Patch details above are incomplete excerpts; do not infer omitted code.]\n", omitted.len()));
    }
    Ok(output)
}

/// Include new local files without staging them or changing the index.
pub(crate) fn untracked(dir: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let result = crate::git::run(dir, &["ls-files", "--others", "--exclude-standard", "-z"]);
    if !result.ok { return Err(result.message()); }
    let mut paths: Vec<_> = result.stdout.split('\0').filter(|path| !path.is_empty()).collect();
    paths.sort_by_key(|path| (context_priority(path), *path));
    let mut output = String::new(); let mut summarized = 0usize; let mut total = 0u64;
    for path in paths {
        let full = dir.join(path);
        let metadata = std::fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
        total += metadata.len();
        if output.len() >= CONTENT_BUDGET { summarized += 1; continue; }
        append_context(&mut output, &format!("\nNew (untracked) file {} ({} bytes):\n", serde_json::to_string(path).unwrap(), metadata.len()));
        if metadata.file_type().is_symlink() { append_context(&mut output, "Symlink; target contents not read.\n"); continue; }
        if dependency(path).is_some() && !retain_import(path) { append_context(&mut output, "Vendored source import; contents omitted.\n"); continue; }
        let limit = DETAIL_PER_FILE.min(CONTENT_BUDGET.saturating_sub(output.len()));
        let mut bytes = Vec::new();
        std::fs::File::open(&full).map_err(|e| e.to_string())?.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if crate::files::is_binary(&bytes) { append_context(&mut output, "Binary file; contents omitted.\n"); continue; }
        let excerpted = metadata.len() > limit as u64;
        bytes.truncate(limit);
        let text = if let Some(encoding) = crate::prefs::encoding_for(dir) { crate::prefs::decode(&bytes, encoding) } else { String::from_utf8_lossy(&bytes).into_owned() };
        append_context(&mut output, &text);
        if excerpted { summarized += 1; append_context(&mut output, "\n[File excerpt only; remaining contents omitted.]\n"); }
    }
    if summarized > 0 { output.push_str(&format!("\n[AI context budget: {summarized} new files have excerpted or omitted contents; total new-file size is {total} bytes. Do not infer omitted implementation details.]\n")); }
    Ok(output)
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
