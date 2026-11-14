//! Compact bulk dependency imports without hiding application or local patch changes.
use std::collections::BTreeMap;

// Git quotes paths containing control characters using C escapes and octal bytes.
fn patch_path(line: &str) -> Option<String> {
    let raw = line.strip_prefix("+++ ").or_else(|| line.strip_prefix("--- "))?;
    let path = if let Some(quoted) = raw.strip_prefix('"') {
        let mut bytes = Vec::new();
        let mut chars = quoted.bytes().peekable();
        while let Some(c) = chars.next() {
            match c {
                b'"' => break,
                b'\\' => {
                    let c = chars.next()?;
                    if (b'0'..=b'7').contains(&c) {
                        let mut n = (c - b'0') as u16;
                        for _ in 0..2 {
                            if let Some(d @ b'0'..=b'7') = chars.peek().copied() {
                                chars.next();
                                n = n * 8 + (d - b'0') as u16;
                            } else { break; }
                        }
                        bytes.push(u8::try_from(n).ok()?);
                    } else {
                        bytes.push(match c { b't' => b'\t', b'n' => b'\n', b'r' => b'\r', b'a' => 7, b'b' => 8, b'f' => 12, b'v' => 11, c => c });
                    }
                }
                c => bytes.push(c),
            }
        }
        String::from_utf8(bytes).ok()?
    } else { raw.trim_end_matches('\t').to_owned() };
    path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).map(str::to_owned)
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

/// Only whole-file imports/removals are compacted. Existing dependency edits stay exact.
pub(crate) fn summarize(diff: &str) -> String {
    let mut output = String::new();
    let mut summaries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut boundaries: Vec<_> = diff.match_indices("diff --git ")
        .filter(|(i, _)| *i == 0 || diff.as_bytes()[i - 1] == b'\n').map(|(i, _)| i).collect();
    if boundaries.is_empty() { return diff.to_owned(); }
    output.push_str(&diff[..boundaries[0]]);
    boundaries.push(diff.len());
    for pair in boundaries.windows(2) {
        let section = &diff[pair[0]..pair[1]];
        let headers: Vec<_> = section.lines().take_while(|l| !l.starts_with("@@")).collect();
        let path = headers.iter().filter_map(|line| patch_path(line)).last();
        let kind = if headers.iter().any(|l| l.starts_with("new file mode ")) { "added" }
            else if headers.iter().any(|l| l.starts_with("deleted file mode ")) { "removed" }
            else { "modified" };
        if let Some((path, group)) = path.as_deref().and_then(|p| dependency(p).map(|g| (p, g))) {
            if kind != "modified" && !retain_import(path) {
                let added = section.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++ ")).count();
                let removed = section.lines().filter(|l| l.starts_with('-') && !l.starts_with("--- ")).count();
                summaries.entry(group).or_default().push(format!("{}: {kind}, +{added}/-{removed} lines", serde_json::to_string(path).unwrap()));
                continue;
            }
        }
        output.push_str(section);
    }
    if !summaries.is_empty() {
        output.push_str("\nVendored dependency summary (bulk file contents omitted; manifests, notices, patch notes, and edits to existing files are retained above). Do not infer implementation details from omitted source:\n");
        for (group, files) in summaries {
            output.push_str(&format!("\n{}: {} bulk files\n", serde_json::to_string(&group).unwrap(), files.len()));
            for file in files { output.push_str(&format!("- {file}\n")); }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    fn added(path: &str, body: &str) -> String {
        format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+{body}\n")
    }
    #[test]
    fn compacts_large_imports_and_preserves_patch_evidence_and_app_code() {
        let mut diff = added("vendor/example/src/lib.rs", &"dependency-source".repeat(100_000));
        for path in ["vendor/example/GITCRAFT-PATCH.txt", "vendor/example/Cargo.toml", "vendor/example/LICENSE", "src/main.rs"] {
            diff.push_str(&added(path, "retained-evidence"));
        }
        let result = summarize(&diff);
        assert!(result.len() < 3000);
        assert!(!result.contains("dependency-source"));
        assert!(result.contains("1 bulk files"));
        assert!(result.contains("added, +1/-0 lines"));
        assert_eq!(result.matches("+retained-evidence").count(), 4);
    }
    #[test]
    fn preserves_existing_vendor_edits_and_nonvendor_imports() {
        let modified = "diff --git a/vendor/pkg/src/lib.rs b/vendor/pkg/src/lib.rs\n--- a/vendor/pkg/src/lib.rs\n+++ b/vendor/pkg/src/lib.rs\n@@ -1 +1 @@\n-old\n+security-fix\n";
        let diff = format!("{modified}{}", added("src/vendor_feature.rs", "application"));
        assert_eq!(summarize(&diff), diff);
    }
    #[test]
    fn handles_removed_nested_dependencies_and_quoted_unicode_paths() {
        let diff = "diff --git a/libs/third_party/pkg/old.rs b/libs/third_party/pkg/old.rs\ndeleted file mode 100644\n--- a/libs/third_party/pkg/old.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-old-source\n";
        let result = summarize(diff);
        assert!(!result.contains("-old-source"));
        assert!(result.contains("removed, +0/-1 lines"));
        assert_eq!(patch_path("+++ \"b/vendor/caf\\303\\251/a\\tfile.rs\""), Some("vendor/café/a\tfile.rs".into()));
    }
}
