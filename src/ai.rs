//! Stage 6: AI helpers through the Claude API, using the person's own Anthropic API key
//! (kept in the OS credential store). Rust has no official Anthropic SDK, so this calls the
//! Messages API over HTTP with structured JSON output. Nothing is sent unless an AI button is pressed.

use crate::git;
use crate::hosting::secret_get;
use serde_json::{json, Value};
use std::path::Path;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
const MODEL: &str = "claude-opus-5-5";
/// Re-run a request that the model's safety classifiers decline on Anthropic's recommended fallback.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Largest diff sent (characters). Bigger changes are refused with a message instead of being cut.
const MAX_INPUT_CHARS: usize = 600000;

fn too_big(what: &str, n: usize) -> String {
    format!("{what} is too large to send ({} KB). Stage fewer files and try again.", n / 1024)
}

/// One request with a JSON schema for the answer; returns the parsed JSON object.
fn ask(system: &str, prompt: String, effort: &str, schema: Value) -> Result<Value, String> {
    let key = secret_get("anthropic-key").ok_or("Add your Anthropic API key in Accounts to use the AI helpers.")?;
    let body = json!({
        "model": MODEL,
        "max_tokens": 16000,
        "system": system,
        "messages": [{ "role": "user", "content": prompt }],
        "output_config": { "effort": effort, "format": { "type": "json_schema", "schema": schema } },
        "fallbacks": "default",
    });
    let res = ureq::post(API_URL)
        .set("x-api-key", &key)
        .set("anthropic-version", API_VERSION)
        .set("anthropic-beta", FALLBACK_BETA)
        .set("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(300))
        .send_json(body);
    let v: Value = match res {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, r)) => {
            let v: Value = r.into_json().unwrap_or(Value::Null);
            let msg = v["error"]["message"].as_str().unwrap_or("request failed").to_string();
            return Err(match code {
                401 => "The Anthropic API key was not accepted. Check it in Accounts.".into(),
                429 => "The Claude API is busy or your rate limit was reached. Try again shortly.".into(),
                _ => format!("Claude API error {code}: {msg}"),
            });
        }
        Err(e) => return Err(format!("Could not reach the Claude API: {e}")),
    };
    match v["stop_reason"].as_str() {
        Some("refusal") => return Err("Claude declined this request.".into()),
        Some("max_tokens") => return Err("The answer was cut off. Try with fewer changes.".into()),
        _ => {}
    }
    let text: String = v["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    serde_json::from_str(&text).map_err(|e| format!("Unexpected answer from Claude: {e}"))
}

fn staged_or_all(dir: &Path) -> String {
    let staged = git::run(dir, &["diff", "--cached", "--no-color", "--no-ext-diff"]).stdout;
    if !staged.trim().is_empty() {
        return staged;
    }
    let mut d = git::run(dir, &["diff", "HEAD", "--no-color", "--no-ext-diff"]).stdout;
    let untracked = git::run(dir, &["ls-files", "--others", "--exclude-standard"]).stdout;
    if !untracked.trim().is_empty() {
        d.push_str("\nNew (untracked) files:\n");
        d.push_str(&untracked);
    }
    d
}

const COMMIT_SYSTEM: &str = "You write git commit messages. Read the diff and describe what changed and why it matters, \
following the repository's existing message style shown in the recent commits. The summary is one line, imperative mood, \
at most 72 characters, no trailing period. The description explains the change in a few short lines, or is empty for trivial changes.";

pub fn commit_message(dir: &Path) -> Result<(String, String), String> {
    let diff = staged_or_all(dir);
    if diff.trim().is_empty() {
        return Err("There are no changes to describe.".into());
    }
    if diff.len() > MAX_INPUT_CHARS {
        return Err(too_big("The diff", diff.len()));
    }
    let recent = git::run(dir, &["log", "-15", "--format=%s"]).stdout;
    let prompt = format!("Recent commit summaries in this repository:\n{recent}\n\nDiff to describe:\n{diff}");
    let schema = json!({
        "type": "object",
        "properties": { "summary": { "type": "string" }, "description": { "type": "string" } },
        "required": ["summary", "description"],
        "additionalProperties": false
    });
    let v = ask(COMMIT_SYSTEM, prompt, "low", schema)?;
    Ok((v["summary"].as_str().unwrap_or_default().to_string(), v["description"].as_str().unwrap_or_default().to_string()))
}

pub fn stash_message(dir: &Path) -> Result<String, String> {
    let diff = staged_or_all(dir);
    if diff.trim().is_empty() {
        return Err("There are no changes to stash.".into());
    }
    if diff.len() > MAX_INPUT_CHARS {
        return Err(too_big("The diff", diff.len()));
    }
    let schema = json!({ "type": "object", "properties": { "title": { "type": "string" } }, "required": ["title"], "additionalProperties": false });
    let v = ask(
        "You name git stashes. Give a short title (under 60 characters) saying what work in progress these changes hold.",
        format!("Changes being stashed:\n{diff}"),
        "low",
        schema,
    )?;
    Ok(v["title"].as_str().unwrap_or_default().to_string())
}

fn explain(prompt: String) -> Result<String, String> {
    if prompt.len() > MAX_INPUT_CHARS {
        return Err(too_big("This change", prompt.len()));
    }
    let schema = json!({ "type": "object", "properties": { "explanation": { "type": "string" } }, "required": ["explanation"], "additionalProperties": false });
    let v = ask(
        "You explain code changes to someone who may not be a programmer. Say what changed, why it likely matters, and anything risky \
         worth checking, in plain language and short paragraphs or bullet points. Mention file names where helpful.",
        prompt,
        "medium",
        schema,
    )?;
    Ok(v["explanation"].as_str().unwrap_or_default().to_string())
}

pub fn explain_commit(dir: &Path, sha: &str) -> Result<String, String> {
    let o = git::run(dir, &["show", "--stat", "--patch", "--no-color", "--no-ext-diff", "--format=fuller", sha]);
    if !o.ok {
        return Err(o.message());
    }
    explain(format!("Explain this commit:\n{}", o.stdout))
}

/// Explain what a branch adds compared with another (usually the main branch).
pub fn explain_branch(dir: &Path, branch: &str, base: &str) -> Result<String, String> {
    let log = git::run(dir, &["log", "--format=- %s", &format!("{base}..{branch}")]).stdout;
    let diff = git::run(dir, &["diff", "--no-color", "--no-ext-diff", &format!("{base}...{branch}")]).stdout;
    if log.trim().is_empty() && diff.trim().is_empty() {
        return Err(format!("{branch} has no changes compared with {base}."));
    }
    explain(format!("Explain what branch '{branch}' changes compared with '{base}'.\nCommits:\n{log}\nCombined diff:\n{diff}"))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ComposedCommit {
    pub summary: String,
    pub description: String,
    pub files: Vec<String>,
}

/// "Compose commits with AI": split the uncommitted changes into a few focused commits (whole files).
pub fn compose(dir: &Path) -> Result<Vec<ComposedCommit>, String> {
    let st = git::status(dir);
    let mut files: Vec<String> = st.unstaged.iter().chain(st.staged.iter()).map(|f| f.path.clone()).collect();
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err("There are no changes to compose into commits.".into());
    }
    let diff = git::run(dir, &["diff", "HEAD", "--no-color", "--no-ext-diff"]).stdout;
    let prompt = format!("Changed files (every one must appear in exactly one commit):\n{}\n\nDiff:\n{diff}", files.join("\n"));
    if prompt.len() > MAX_INPUT_CHARS {
        return Err(too_big("The changes", prompt.len()));
    }
    let schema = json!({
        "type": "object",
        "properties": {
            "commits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "summary": { "type": "string" },
                        "description": { "type": "string" },
                        "files": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["summary", "description", "files"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["commits"],
        "additionalProperties": false
    });
    let v = ask(
        "You organise uncommitted work into a small number of focused git commits, oldest first. Group whole files that belong to the \
         same logical change. Every listed file goes in exactly one commit. Each commit gets an imperative one-line summary of at most \
         72 characters and a short description (may be empty).",
        prompt,
        "medium",
        schema,
    )?;
    let mut out: Vec<ComposedCommit> = serde_json::from_value(v["commits"].clone()).map_err(|e| e.to_string())?;
    // Keep only real changed files, each once; anything left out goes into a final commit.
    let mut used = std::collections::HashSet::new();
    for c in &mut out {
        c.files.retain(|f| files.contains(f) && used.insert(f.clone()));
    }
    out.retain(|c| !c.files.is_empty());
    let rest: Vec<String> = files.iter().filter(|f| !used.contains(*f)).cloned().collect();
    if !rest.is_empty() {
        out.push(ComposedCommit { summary: "Remaining changes".into(), description: String::new(), files: rest });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_a_key_and_refuses_empty_changes() {
        // With no changes, nothing is sent to the API.
        let d = std::env::temp_dir().join(format!("gitcraft-ai-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(git::run(&d, &["init", "-q"]).ok);
        assert!(commit_message(&d).unwrap_err().contains("no changes"));
        assert!(compose(&d).unwrap_err().contains("no changes"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
