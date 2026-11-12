//! Every AI helper shares a connection, including subscription CLIs and compatible APIs.
//! Credentials stay in their provider's credential store. Requests only follow an AI button press.
//! Compatibility retries stay on the chosen endpoint.

use crate::{ai_cli, git};
use crate::hosting::secret_get;
use serde_json::{json, Value};
use std::path::Path;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub(crate) const API_VERSION: &str = "2026-10-21";
const MODEL: &str = "claude-opus-5-5";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const MAX_INPUT_CHARS: usize = 600000;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    pub enabled: bool,
    /// anthropic, openai, codex, claude_code, google, ollama or custom.
    pub provider: String,
    pub endpoint: String,
    pub cli_path: String,
    pub cli_config_dir: String,
    /// Shared by every feature, including Test Connection; feature models override this.
    pub default_model: String,
    /// auto, json_schema, json_object or text (JSON requested in the prompt).
    pub response_format: String,
    pub instructions: String,
    pub models: std::collections::HashMap<String, String>,
    pub feature_instructions: std::collections::HashMap<String, String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            enabled: true, provider: "anthropic".into(), endpoint: String::new(),
            cli_path: String::new(), cli_config_dir: String::new(),
            default_model: String::new(), response_format: "auto".into(),
            instructions: String::new(), models: Default::default(), feature_instructions: Default::default(),
        }
    }
}

static CONFIG: std::sync::RwLock<Option<AiConfig>> = std::sync::RwLock::new(None);

pub fn set_config(c: AiConfig) {
    if let Ok(mut w) = CONFIG.write() { *w = Some(c); }
}

fn config() -> AiConfig {
    CONFIG.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}

pub fn key_name(provider: &str) -> &'static str {
    match provider {
        "openai" => "openai-key", "google" => "google-key",
        "ollama" => "ollama-key", "custom" => "custom-ai-key", _ => "anthropic-key",
    }
}

pub(crate) fn provider_label(p: &str) -> &'static str {
    match p {
        "openai" => "OpenAI", "google" => "Google", "ollama" => "Ollama",
        "codex" => "ChatGPT via Codex", "claude_code" => "Claude Code",
        "custom" => "your compatible AI server", _ => "Anthropic",
    }
}

/// Accept a server root, API base or complete endpoint, preserving proxy paths and query parameters.
pub(crate) fn api_url(c: &AiConfig) -> Result<String, String> {
    let default = match c.provider.as_str() {
        "anthropic" => API_URL,
        "openai" => "https://api.openai.com/v1/chat/completions",
        "google" => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        "ollama" => "http://localhost:11434/v1/chat/completions",
        "custom" => "",
        _ => return Err("Choose a supported provider in Preferences > AI.".into()),
    };
    let endpoint = if c.endpoint.trim().is_empty() { default } else { c.endpoint.trim() };
    if endpoint.is_empty() { return Err("Set the API endpoint URL in Preferences > AI.".into()); }
    let mut url = tauri::Url::parse(endpoint).map_err(|_| "Enter a complete http:// or https:// endpoint in Preferences > AI.")?;
    if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err("Use an HTTP or HTTPS endpoint without login details or a #fragment. Save the API key separately.".into());
    }
    let path = url.path().trim_end_matches('/');
    let path = if c.provider == "anthropic" {
        if path.ends_with("/messages") { path.to_string() }
        else if path.is_empty() { "/v1/messages".into() }
        else { format!("{path}/messages") }
    } else if path.ends_with("/chat/completions") {
        path.to_string()
    } else if path.is_empty() || (c.provider == "ollama" && path == "/api") {
        "/v1/chat/completions".into()
    } else {
        format!("{path}/chat/completions")
    };
    url.set_path(&path);
    Ok(url.into())
}

fn model_override<'a>(c: &'a AiConfig, feature: &str) -> Option<&'a str> {
    c.models.get(feature).map(|s| s.trim()).filter(|s| !s.is_empty())
        .or_else(|| (!c.default_model.trim().is_empty()).then(|| c.default_model.trim()))
        // Older settings may only have per-feature models; test the commit model in that case.
        .or_else(|| (feature == "test").then(|| c.models.get("commit").map(|s| s.trim()).filter(|s| !s.is_empty())).flatten())
}

fn selected_model(c: &AiConfig, feature: &str) -> Result<String, String> {
    match model_override(c, feature) {
        Some(m) => Ok(m.to_string()),
        None if c.provider == "anthropic" => Ok(MODEL.into()),
        None => Err("Set a Default Model in Preferences > AI, or a model override for this feature.".into()),
    }
}

/// Check configuration without making a network request.
pub fn configured() -> bool {
    let c = config();
    c.enabled && if let Some(agent) = ai_cli::Agent::from_provider(&c.provider) {
        ai_cli::signed_in(agent, &c.cli_path, &c.cli_config_dir)
    } else {
        api_url(&c).is_ok() && selected_model(&c, "test").is_ok()
            && (matches!(c.provider.as_str(), "ollama" | "custom") || secret_get(key_name(&c.provider)).is_some())
    }
}

fn too_big(what: &str, n: usize) -> String {
    format!("{what} is too large to send ({} KB). Stage fewer files and try again.", n / 1024)
}

/// Validate prompt-only replies before a helper puts them into a commit or file.
fn validate_answer(v: &Value, schema: &Value) -> Result<(), String> {
    match schema["type"].as_str() {
        Some("object") => {
            let obj = v.as_object().ok_or("Expected a JSON object")?;
            if let Some(required) = schema["required"].as_array() {
                for name in required.iter().filter_map(Value::as_str) {
                    if !obj.contains_key(name) { return Err(format!("Missing field '{name}'")); }
                }
            }
            if let Some(props) = schema["properties"].as_object() {
                for (name, value) in obj {
                    if let Some(field) = props.get(name) { validate_answer(value, field).map_err(|e| format!("{name}: {e}"))?; }
                    else if schema["additionalProperties"] == false { return Err(format!("Unexpected field '{name}'")); }
                }
            }
        }
        Some("array") => {
            for item in v.as_array().ok_or("Expected an array")? { validate_answer(item, &schema["items"])?; }
        }
        Some("string") if !v.is_string() => return Err("Expected text".into()),
        Some("boolean") if !v.is_boolean() => return Err("Expected true or false".into()),
        _ => {}
    }
    Ok(())
}

fn parse_answer(text: &str, schema: &Value, provider: &str) -> Result<Value, String> {
    let mut text = text.trim();
    if text.starts_with("<think>") {
        text = text.split_once("</think>").map(|(_, rest)| rest.trim()).unwrap_or(text);
    }
    let fence = char::from(96).to_string().repeat(3);
    if text.starts_with(&fence) || text.starts_with("~~~") {
        if let Some((_, body)) = text.split_once('\n') {
            text = body.trim_end().trim_end_matches(char::from(96)).trim_end_matches('~').trim();
        }
    }
    let v: Value = serde_json::from_str(text).map_err(|_| format!("{} returned an invalid JSON answer. Try another model or response mode in Preferences > AI.", provider_label(provider)))?;
    validate_answer(&v, schema).map_err(|e| format!("{} returned an incomplete answer ({e}). Try another model.", provider_label(provider)))?;
    Ok(v)
}

fn system_with_instructions(c: &AiConfig, feature: &str, system: &str) -> String {
    let mut system = system.to_string();
    // A connection check shouldn't send saved repository-specific instructions to the provider.
    if feature != "test" {
        for extra in [c.instructions.trim(), c.feature_instructions.get(feature).map(|s| s.trim()).unwrap_or("")] {
            if !extra.is_empty() {
                system.push_str("\n\nFollow these instructions from the user as well:\n");
                system.push_str(extra);
            }
        }
    }
    system
}

/// All features, including PR drafting and conflict resolution, use this connection.
fn ask(feature: &str, system: &str, prompt: String, effort: &str, schema: Value) -> Result<Value, String> {
    let c = config();
    if !c.enabled { return Err("AI features are turned off (Preferences > AI).".into()); }
    let system = system_with_instructions(&c, feature, system);
    if let Some(agent) = ai_cli::Agent::from_provider(&c.provider) {
        let reply = ai_cli::ask(agent, &c.cli_path, &c.cli_config_dir, model_override(&c, feature), &system, &prompt, &schema)?;
        return parse_answer(&reply, &schema, &c.provider);
    }
    let model = selected_model(&c, feature)?;
    let url = api_url(&c)?;
    let key = secret_get(key_name(&c.provider)).filter(|k| !k.trim().is_empty());
    if key.is_none() && !matches!(c.provider.as_str(), "ollama" | "custom") {
        return Err(format!("Add your {} API key in Preferences > AI.", provider_label(&c.provider)));
    }
    if c.provider != "anthropic" {
        return ask_openai_style(&c, &url, &model, key.as_deref(), &system, &prompt, &schema);
    }
    let body = json!({
        "model": model, "max_tokens": 16000, "system": system,
        "messages": [{ "role": "user", "content": prompt }],
        "output_config": { "effort": effort, "format": { "type": "json_schema", "schema": schema } },
        "fallbacks": "default",
    });
    let res = ureq::post(&url)
        .set("x-api-key", key.as_deref().unwrap_or_default())
        .set("anthropic-version", API_VERSION)
        .set("anthropic-beta", FALLBACK_BETA)
        .set("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(300)).send_json(body);
    let v = read_response(res, &c.provider)?;
    match v["stop_reason"].as_str() {
        Some("refusal") => return Err("The model declined this request.".into()),
        Some("max_tokens") => return Err("The answer was cut off. Try with fewer changes.".into()),
        _ => {}
    }
    let text: String = v["content"].as_array().into_iter().flatten()
        .filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
    parse_answer(&text, &schema, &c.provider)
}

fn error_message(v: &Value) -> &str {
    v["error"]["message"].as_str().or(v["error"].as_str())
        .or(v[0]["error"]["message"].as_str()).or(v["message"].as_str()).unwrap_or("request failed")
}

fn read_response(res: Result<ureq::Response, ureq::Error>, provider: &str) -> Result<Value, String> {
    match res {
        Ok(r) => r.into_json().map_err(|_| format!("{} returned an invalid API response.", provider_label(provider))),
        Err(ureq::Error::Status(code, r)) => {
            let v: Value = r.into_json().unwrap_or(Value::Null);
            Err(match code {
                401 | 403 => format!("{} rejected access. Check the API key and endpoint in Preferences > AI.", provider_label(provider)),
                429 => format!("{} is busy or its rate limit was reached. Try again shortly.", provider_label(provider)),
                _ => format!("{} error {code}: {}", provider_label(provider), error_message(&v)),
            })
        }
        Err(_) => Err(format!("Could not reach {}. Check that the server is running and the endpoint is correct in Preferences > AI.", provider_label(provider))),
    }
}

/// Retry only explicit output-format incompatibility, never auth, rate-limit or model errors.
fn unsupported_format(code: u16, v: &Value) -> bool {
    let message = format!("{} {}", error_message(v), v["error"]["param"].as_str().unwrap_or("")).to_lowercase();
    matches!(code, 400 | 422 | 501)
        && ["response_format", "json_schema", "json_object", "structured output", "structured_output"].iter().any(|s| message.contains(s))
        && ["unsupported", "not support", "does not support", "not available", "unknown", "unrecognized", "not permitted", "not allowed", "invalid", "extra", "must be", "only"].iter().any(|s| message.contains(s))
}

fn ask_openai_style(c: &AiConfig, url: &str, model: &str, key: Option<&str>, system: &str, prompt: &str, schema: &Value) -> Result<Value, String> {
    let modes: &[&str] = match c.response_format.as_str() {
        "" | "auto" => &["json_schema", "json_object", "text"],
        "json_schema" => &["json_schema"],
        "json_object" => &["json_object"],
        "text" => &["text"],
        _ => return Err("Choose a response mode in Preferences > AI.".into()),
    };
    let system = format!("{system}\n\nReturn only one JSON object matching this schema. Do not include Markdown or commentary:\n{schema}");
    for (index, mode) in modes.iter().enumerate() {
        let mut body = json!({
            "model": model,
            "messages": [{ "role": "system", "content": system }, { "role": "user", "content": prompt }],
            "stream": false,
        });
        if *mode == "json_schema" {
            body["response_format"] = json!({ "type": "json_schema", "json_schema": { "name": "answer", "strict": true, "schema": schema } });
        } else if *mode == "json_object" {
            body["response_format"] = json!({ "type": "json_object" });
        }
        let mut req = ureq::post(url).set("content-type", "application/json").timeout(std::time::Duration::from_secs(300));
        if let Some(k) = key.filter(|s| !s.is_empty()) { req = req.set("Authorization", &format!("Bearer {k}")); }
        let res = match req.send_json(body) {
            Err(ureq::Error::Status(code, r)) if index + 1 < modes.len() => {
                let status = r.status_text().to_string();
                let v: Value = r.into_json().unwrap_or(Value::Null);
                if unsupported_format(code, &v) { continue; }
                return Err(format!("{} error {code}: {}", provider_label(&c.provider), v.as_object().map(|_| error_message(&v)).unwrap_or(&status)));
            }
            other => other,
        };
        let v = read_response(res, &c.provider)?;
        let choice = &v["choices"][0];
        if choice["finish_reason"] == "length" { return Err("The answer was cut off. Try with fewer changes or increase your server's output limit.".into()); }
        if choice["finish_reason"] == "content_filter" || choice["message"]["refusal"].as_str().is_some_and(|s| !s.is_empty()) {
            return Err("The model declined this request.".into());
        }
        let content = &choice["message"]["content"];
        let text = content.as_str().map(str::to_string).unwrap_or_else(|| content.as_array().into_iter().flatten().filter_map(|p| p["text"].as_str()).collect());
        return parse_answer(&text, schema, &c.provider);
    }
    Err("The server could not produce a supported response. Choose a response mode in Preferences > AI.".into())
}

pub fn test_connection() -> Result<String, String> {
    let schema = json!({ "type": "object", "properties": { "ok": { "type": "boolean" } }, "required": ["ok"], "additionalProperties": false });
    let v = ask("test", "Answer with {\"ok\": true}.", "Reply ok.".into(), "low", schema)?;
    if v["ok"] != true { return Err("The server answered, but did not pass the connection test.".into()); }
    Ok(format!("Connected to {} — the selected model answered successfully", provider_label(&config().provider)))
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


#[cfg(test)]
mod connection_tests {
    use super::*;
    use std::io::{BufRead, Read, Write};
    use std::net::TcpListener;

    fn local_config() -> AiConfig {
        AiConfig { provider: "custom".into(), default_model: "local-model".into(), ..Default::default() }
    }

    #[test]
    fn endpoint_roots_bases_complete_urls_and_queries() {
        let mut c = local_config();
        for endpoint in ["http://localhost:1234", "http://localhost:1234/v1/", "http://localhost:1234/v1/chat/completions"] {
            c.endpoint = endpoint.into();
            assert_eq!(api_url(&c).unwrap(), "http://localhost:1234/v1/chat/completions");
        }
        c.endpoint = "https://example.test/proxy/v1/?api-version=1".into();
        assert_eq!(api_url(&c).unwrap(), "https://example.test/proxy/v1/chat/completions?api-version=1");
        for endpoint in ["", "localhost:1234", "file:///tmp/model", "https://user:pass@example.test/v1", "https://example.test/#fragment"] {
            c.endpoint = endpoint.into();
            assert!(api_url(&c).is_err(), "{endpoint}");
        }
        c.provider = "ollama".into();
        c.endpoint = "http://localhost:11434/api".into();
        assert_eq!(api_url(&c).unwrap(), "http://localhost:11434/v1/chat/completions");
        c.provider = "anthropic".into();
        c.endpoint = "https://example.test/v1".into();
        assert_eq!(api_url(&c).unwrap(), "https://example.test/v1/messages");
    }

    #[test]
    fn every_feature_inherits_default_and_keeps_overrides_and_legacy_settings() {
        let mut c = local_config();
        for feature in ["commit", "compose", "stash", "explain", "pr", "conflict", "test"] {
            assert_eq!(selected_model(&c, feature).unwrap(), "local-model");
        }
        c.models.insert("conflict".into(), " larger-local-model ".into());
        assert_eq!(selected_model(&c, "conflict").unwrap(), "larger-local-model");
        assert_eq!(selected_model(&c, "test").unwrap(), "local-model");
        let legacy: AiConfig = serde_json::from_value(json!({"provider":"ollama", "models":{"commit":"old-model"}})).unwrap();
        assert_eq!(selected_model(&legacy, "commit").unwrap(), "old-model");
        assert_eq!(selected_model(&legacy, "test").unwrap(), "old-model");
        assert!(selected_model(&legacy, "pr").is_err());
    }

    #[test]
    fn validates_nested_composition_and_refuses_incomplete_or_wrong_types() {
        let schema = json!({
            "type":"object", "properties":{"commits":{"type":"array","items":{
                "type":"object", "properties":{"summary":{"type":"string"},"files":{"type":"array","items":{"type":"string"}}},
                "required":["summary","files"],"additionalProperties":false
            }}}, "required":["commits"], "additionalProperties":false
        });
        let valid = json!({"commits":[{"summary":"Update file","files":["a.rs"]}]});
        let fence = char::from(96).to_string().repeat(3);
        let answer = format!("<think>reasoning</think>\n{fence}json\n{valid}\n{fence}");
        assert_eq!(parse_answer(&answer, &schema, "custom").unwrap(), valid);
        for bad in [json!({}), json!({"commits":[{"summary":1,"files":[]}]}), json!({"commits":[{"summary":"x","files":[false]}]})] {
            assert!(parse_answer(&bad.to_string(), &schema, "custom").is_err());
        }
        assert!(parse_answer("", &schema, "custom").is_err());
    }

    /// Loopback-only fake server; no external provider or credential store is contacted.
    fn server(replies: Vec<(u16, Value)>) -> (String, std::thread::JoinHandle<Vec<(String, Value)>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/v1/chat/completions", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, reply) in replies {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                let stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(10)),
                        Err(e) => panic!("Expected local request: {e}"),
                    }
                };
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut reader = std::io::BufReader::new(stream);
                let mut headers = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() { break; }
                    if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") { length = n.trim().parse::<usize>().unwrap(); }
                    headers.push_str(&line);
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                requests.push((headers, serde_json::from_slice(&body).unwrap()));
                let body = reply.to_string();
                let mut stream = reader.into_inner();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        (url, handle)
    }

    #[test]
    fn keyless_server_falls_back_to_prompt_json_and_preserves_request() {
        let schema = json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false});
        let rejection = json!({"error":{"message":"response_format is not supported"}});
        let (url, server) = server(vec![
            (400, rejection.clone()), (422, rejection),
            (200, json!({"choices":[{"message":{"content":"{\"ok\":true}"},"finish_reason":"stop"}]})),
        ]);
        let c = local_config();
        assert_eq!(ask_openai_style(&c, &url, "local-model", None, "instructions", "changes", &schema).unwrap(), json!({"ok":true}));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].1["response_format"]["type"], "json_schema");
        assert_eq!(requests[1].1["response_format"]["type"], "json_object");
        assert!(requests[2].1.get("response_format").is_none());
        for (headers, body) in requests {
            assert!(!headers.to_lowercase().contains("authorization:"));
            assert_eq!(body["model"], "local-model");
            assert_eq!(body["messages"][1]["content"], "changes");
            assert!(body["messages"][0]["content"].as_str().unwrap().contains("instructions"));
            assert!(body["messages"][0]["content"].as_str().unwrap().contains("\"ok\""));
        }
    }

    #[test]
    fn authenticated_server_and_explicit_modes_work_without_retry() {
        let (url, server) = server(vec![(200, json!({"choices":[{"message":{"content":"{\"ok\":true}"}}]}))]);
        let c = AiConfig { response_format: "text".into(), ..local_config() };
        let schema = json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"]});
        assert!(ask_openai_style(&c, &url, "model", Some("test-key"), "system", "prompt", &schema).is_ok());
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].0.to_lowercase().contains("authorization: bearer test-key"));
        assert!(requests[0].1.get("response_format").is_none());
        let format_error = json!({"error":{"message":"response_format unsupported"}});
        for status in [401, 403, 404, 429, 500] { assert!(!unsupported_format(status, &format_error)); }
        assert!(!unsupported_format(400, &json!({"error":{"message":"model not found"}})));
    }
}
