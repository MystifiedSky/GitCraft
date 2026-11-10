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

/// Preferences > AI (the original's GKAIPreferences): on/off, which provider your own key is for, an
/// optional endpoint, and per feature a model and custom instructions on top of the global ones.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    pub enabled: bool,
    /// anthropic, openai, google, ollama or custom (any OpenAI-compatible endpoint).
    pub provider: String,
    pub endpoint: String,
    pub instructions: String,
    /// Feature -> model ("" = the provider's default).
    pub models: std::collections::HashMap<String, String>,
    /// Feature -> extra instructions.
    pub feature_instructions: std::collections::HashMap<String, String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig { enabled: true, provider: "anthropic".into(), endpoint: String::new(), instructions: String::new(), models: Default::default(), feature_instructions: Default::default() }
    }
}

static CONFIG: std::sync::RwLock<Option<AiConfig>> = std::sync::RwLock::new(None);

pub fn set_config(c: AiConfig) {
    if let Ok(mut w) = CONFIG.write() {
        *w = Some(c);
    }
}

fn config() -> AiConfig {
    CONFIG.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}

/// The credential-store name of each provider's key.
pub fn key_name(provider: &str) -> &'static str {
    match provider {
        "openai" => "openai-key",
        "google" => "google-key",
        "ollama" => "ollama-key",
        "custom" => "custom-ai-key",
        _ => "anthropic-key",
    }
}

fn provider_label(p: &str) -> &'static str {
    match p {
        "openai" => "OpenAI",
        "google" => "Google",
        "ollama" => "Ollama",
        "custom" => "your AI provider",
        _ => "Anthropic",
    }
}

/// OpenAI-style chat completions URL for the non-Anthropic providers.
fn chat_url(c: &AiConfig) -> Result<String, String> {
    let e = c.endpoint.trim().trim_end_matches('/');
    Ok(match c.provider.as_str() {
        "openai" if e.is_empty() => "https://api.openai.com/v1/chat/completions".into(),
        "google" if e.is_empty() => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions".into(),
        "ollama" if e.is_empty() => "http://localhost:11434/v1/chat/completions".into(),
        "custom" if e.is_empty() => return Err("Set the API endpoint URL in Preferences > AI.".into()),
        _ if e.ends_with("/chat/completions") => e.to_string(),
        _ => format!("{e}/chat/completions"),
    })
}

fn too_big(what: &str, n: usize) -> String {
    format!("{what} is too large to send ({} KB). Stage fewer files and try again.", n / 1024)
}

/// One request with a JSON schema for the answer; returns the parsed JSON object. `feature` picks the model
/// and the custom instructions (commit, compose, stash, explain, pr, conflict).
fn ask(feature: &str, system: &str, prompt: String, effort: &str, schema: Value) -> Result<Value, String> {
    let c = config();
    if !c.enabled {
        return Err("AI features are turned off (Preferences > AI).".into());
    }
    let mut system = system.to_string();
    for extra in [c.instructions.trim(), c.feature_instructions.get(feature).map(|s| s.trim()).unwrap_or("")] {
        if !extra.is_empty() {
            system.push_str("\n\nFollow these instructions from the user as well:\n");
            system.push_str(extra);
        }
    }
    let model = c.models.get(feature).map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    if c.provider != "anthropic" && !c.provider.is_empty() {
        return ask_openai_style(&c, model, &system, prompt, schema);
    }
    let key = secret_get("anthropic-key").ok_or("Add your Anthropic API key in Preferences > AI to use the AI helpers.")?;
    let url = if c.endpoint.trim().is_empty() { API_URL.to_string() } else { format!("{}/v1/messages", c.endpoint.trim().trim_end_matches('/').trim_end_matches("/v1/messages")) };
    let body = json!({
        "model": model.as_deref().unwrap_or(MODEL),
        "max_tokens": 16000,
        "system": system,
        "messages": [{ "role": "user", "content": prompt }],
        "output_config": { "effort": effort, "format": { "type": "json_schema", "schema": schema } },
        "fallbacks": "default",
    });
    let res = ureq::post(&url)
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
                401 => "The Anthropic API key was not accepted. Check it in Preferences > AI.".into(),
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

/// OpenAI, Google (its OpenAI-compatible endpoint), Ollama and custom endpoints: chat completions with a JSON schema.
fn ask_openai_style(c: &AiConfig, model: Option<String>, system: &str, prompt: String, schema: Value) -> Result<Value, String> {
    let url = chat_url(c)?;
    let key = secret_get(key_name(&c.provider));
    if key.is_none() && c.provider != "ollama" {
        return Err(format!("Add your {} API key in Preferences > AI.", provider_label(&c.provider)));
    }
    let model = model.ok_or_else(|| format!("Choose a model for {} in Preferences > AI.", provider_label(&c.provider)))?;
    let body = json!({
        "model": model,
        "messages": [{ "role": "system", "content": system }, { "role": "user", "content": prompt }],
        "response_format": { "type": "json_schema", "json_schema": { "name": "answer", "strict": true, "schema": schema } },
    });
    let mut req = ureq::post(&url).set("content-type", "application/json").timeout(std::time::Duration::from_secs(300));
    if let Some(k) = key {
        req = req.set("Authorization", &format!("Bearer {k}"));
    }
    let v: Value = match req.send_json(body) {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, r)) => {
            let v: Value = r.into_json().unwrap_or(Value::Null);
            let msg = v["error"]["message"].as_str().or(v[0]["error"]["message"].as_str()).unwrap_or("request failed").to_string();
            return Err(format!("{} error {code}: {msg}", provider_label(&c.provider)));
        }
        Err(e) => return Err(format!("Could not reach {}: {e}", provider_label(&c.provider))),
    };
    let choice = &v["choices"][0];
    if choice["finish_reason"] == "length" {
        return Err("The answer was cut off. Try with fewer changes.".into());
    }
    let text = choice["message"]["content"].as_str().unwrap_or_default();
    let text = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    serde_json::from_str(text).map_err(|e| format!("Unexpected answer from {}: {e}", provider_label(&c.provider)))
}

/// Preferences > AI > Test Connection.
pub fn test_connection() -> Result<String, String> {
    let schema = json!({ "type": "object", "properties": { "ok": { "type": "boolean" } }, "required": ["ok"], "additionalProperties": false });
    ask("test", "Answer with {\"ok\": true}.", "Reply ok.".into(), "low", schema)?;
    Ok(format!("{} API key is valid", provider_label(&config().provider)))
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
    let v = ask("commit", COMMIT_SYSTEM, prompt, "low", schema)?;
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
        "stash",
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
        "explain",
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
        "compose",
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

/// Pull Request (GKAIPreferences-PullRequest): a title and description from a branch's commits and changes.
pub fn pull_request_text(dir: &Path, source: &str, target: &str) -> Result<(String, String), String> {
    let base = format!("{target}...{source}");
    let log = git::run(dir, &["log", "--format=- %s%n%b", &format!("{target}..{source}")]).stdout;
    let diff = git::run(dir, &["diff", "--no-color", "--no-ext-diff", &base]).stdout;
    if log.trim().is_empty() && diff.trim().is_empty() {
        return Err(format!("{source} has no changes compared with {target}."));
    }
    let prompt = format!("Write a pull request for merging '{source}' into '{target}'.\nCommits:\n{log}\nCombined diff:\n{diff}");
    if prompt.len() > MAX_INPUT_CHARS {
        return Err(too_big("This branch's changes", prompt.len()));
    }
    let schema = json!({ "type": "object", "properties": { "title": { "type": "string" }, "description": { "type": "string" } }, "required": ["title", "description"], "additionalProperties": false });
    let v = ask(
        "pr",
        "You write pull request titles and descriptions. The title is one short line. The description says what changed and why, \
         lists notable changes as bullet points, and mentions anything reviewers should check. Use Markdown.",
        prompt,
        "medium",
        schema,
    )?;
    Ok((v["title"].as_str().unwrap_or_default().to_string(), v["description"].as_str().unwrap_or_default().to_string()))
}

/// Conflict Resolution (GKAIPreferences-ConflictResolution): a merged version of a conflicted file, for the
/// merge tool's Output (the person reviews it before saving).
pub fn resolve_conflict(base: &str, ours: &str, theirs: &str, file: &str) -> Result<String, String> {
    let prompt = format!("File: {file}\n\n=== BASE (common ancestor) ===\n{base}\n=== CURRENT (ours) ===\n{ours}\n=== INCOMING (theirs) ===\n{theirs}");
    if prompt.len() > MAX_INPUT_CHARS {
        return Err(too_big("This file", prompt.len()));
    }
    let schema = json!({ "type": "object", "properties": { "merged": { "type": "string" }, "notes": { "type": "string" } }, "required": ["merged", "notes"], "additionalProperties": false });
    let v = ask(
        "conflict",
        "You resolve git merge conflicts. Combine the current and incoming versions so both sides' intended changes are kept \
         where possible. Return the complete merged file without conflict markers, preserving formatting and line endings.",
        prompt,
        "high",
        schema,
    )?;
    Ok(v["merged"].as_str().unwrap_or_default().to_string())
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
