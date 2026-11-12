//! Local, read-only session discovery for Claude Code and Codex.
//! Codex JSONL: session_meta + response_item messages + task_started/task_complete events.
use serde::Serialize;
use serde_json::Value;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Clone, Debug)]
pub struct AgentSession {
    pub id: String,
    pub agent: String,
    pub path: String,
    pub title: String,
    pub modified: i64,
    pub messages: usize,
    /// Log evidence only: never claim a live process based on file modification time.
    pub status: String,
}

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from).unwrap_or_default()
}

fn config_dir(explicit: Option<&str>, variable: &str, fallback: &str) -> PathBuf {
    let value = explicit.filter(|p| !p.trim().is_empty()).map(PathBuf::from)
        .or_else(|| std::env::var_os(variable).map(PathBuf::from))
        .unwrap_or_else(|| home().join(fallback));
    let s = value.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        home().join(rest)
    } else { value }
}

fn normalized(path: &str) -> String {
    let s = path.replace('\\', "/");
    let s = if let Some(unc) = s.strip_prefix("//?/UNC/") { format!("//{unc}") } else { s.strip_prefix("//?/").unwrap_or(&s).to_string() };
    let s = s.trim_end_matches('/');
    if cfg!(windows) { s.to_lowercase() } else { s.to_string() }
}

fn belongs(cwd: &str, root: &str) -> bool {
    let cwd = normalized(cwd);
    let root = normalized(root);
    cwd == root || cwd.strip_prefix(&root).is_some_and(|tail| tail.starts_with('/'))
}

fn project_name(path: &str) -> String {
    path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn jsonl_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() && depth > 0 { jsonl_files(&entry.path(), depth - 1, out); }
        else if kind.is_file() && entry.path().extension().is_some_and(|x| x == "jsonl") { out.push(entry.path()); }
    }
}

fn message_text(content: &Value) -> String {
    if let Some(s) = content.as_str() { return s.to_string() }
    content.as_array().map(|items| items.iter().filter_map(|v| v["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
}

fn title_from(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') || text.starts_with("# AGENTS.md") { return None }
    Some(text.lines().next().unwrap_or_default().chars().take(120).collect())
}

fn read_session(file: &Path, agent: &str, roots: &[String], fallback: Option<&str>) -> Option<AgentSession> {
    let meta = fs::metadata(file).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    // A corrupt or enormous log cannot allocate unbounded memory. Partial JSON lines are ignored.
    let mut reader = BufReader::new(File::open(file).ok()?.take(32 * 1024 * 1024));
    let mut line = String::new();
    let mut cwd = fallback.unwrap_or_default().to_string();
    let mut id = if agent == "claude" { file.file_stem()?.to_string_lossy().into_owned() } else { String::new() };
    let mut title = String::new();
    let mut messages = 0;
    let mut finished = false;
    let mut has_meta = agent == "claude";
    while reader.read_line(&mut line).ok()? > 0 {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if agent == "codex" {
                let p = &v["payload"];
                if v["type"] == "session_meta" && !has_meta {
                    cwd = p["cwd"].as_str()?.to_string();
                    id = p["id"].as_str()?.to_string();
                    has_meta = true;
                    if !roots.iter().any(|r| belongs(&cwd, r)) { return None }
                }
                if v["type"] == "response_item" && p["type"] == "message" {
                    if p["role"] == "user" || p["role"] == "assistant" { messages += 1; }
                    if title.is_empty() && p["role"] == "user" {
                        if let Some(t) = title_from(&message_text(&p["content"])) { title = t; }
                    }
                }
                if v["type"] == "event_msg" {
                    match p["type"].as_str().unwrap_or_default() {
                        "task_started" | "turn_started" => finished = false,
                        "task_complete" | "turn_complete" | "turn_aborted" => finished = true,
                        "user_message" if title.is_empty() => {
                            if let Some(t) = p["message"].as_str().and_then(title_from) { title = t; }
                        }
                        _ => {}
                    }
                }
            } else {
                if let Some(path) = v["cwd"].as_str() { cwd = path.to_string(); }
                if v["type"] == "user" || v["type"] == "assistant" { messages += 1; }
                if v["type"] == "user" {
                    finished = false;
                    if title.is_empty() {
                        if let Some(t) = title_from(&message_text(&v["message"]["content"])) { title = t; }
                    }
                }
                if v["type"] == "assistant" {
                    finished = matches!(v["message"]["stop_reason"].as_str(), Some("end_turn" | "stop_sequence"));
                }
            }
        }
        line.clear();
    }
    if !has_meta || id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { return None }
    let root = roots.iter().filter(|r| belongs(&cwd, r)).max_by_key(|r| r.len())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(AgentSession {
        id, agent: agent.into(), path: root.clone(),
        title: if title.is_empty() { "(untitled session)".into() } else { title },
        modified, messages,
        status: if finished && meta.len() <= 32 * 1024 * 1024 { "finished" } else if now.saturating_sub(modified) < 120 { "recent" } else { "idle" }.into(),
    })
}

/// Only sessions whose recorded working directory is in one of this repository's worktrees.
pub fn sessions(roots: &[String], claude_dir: Option<&str>, codex_dir: Option<&str>) -> Vec<AgentSession> {
    let claude = config_dir(claude_dir, "CLAUDE_CONFIG_DIR", ".claude");
    let codex = config_dir(codex_dir, "CODEX_HOME", ".codex");
    let mut out = Vec::new();
    for root in roots {
        let mut names = vec![project_name(root)];
        if let Ok(canonical) = fs::canonicalize(root) {
            let plain = canonical.to_string_lossy().trim_start_matches(r"\\?\").to_string();
            names.push(project_name(&plain));
        }
        names.sort();
        names.dedup();
        for name in names {
            let mut files = Vec::new();
            jsonl_files(&claude.join("projects").join(name), 0, &mut files);
            for file in files {
                if let Some(session) = read_session(&file, "claude", roots, Some(root)) { out.push(session); }
            }
        }
    }
    let mut files = Vec::new();
    jsonl_files(&codex.join("sessions"), 3, &mut files);
    for file in files {
        if let Some(session) = read_session(&file, "codex", roots, None) { out.push(session); }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert((s.agent.clone(), s.id.clone())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn discovers_both_agents_and_filters_worktrees_and_unrelated_folders() {
        let temp = std::env::temp_dir().join(format!("gitcraft-agent-discovery-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let repo = temp.join("repo").to_string_lossy().into_owned();
        let worktree = temp.join("repo-agent").to_string_lossy().into_owned();
        let roots = vec![repo.clone(), worktree.clone()];
        let claude = temp.join("claude");
        let codex = temp.join("codex");
        let cdir = claude.join("projects").join(project_name(&repo));
        let xdir = codex.join("sessions/2026/11/12");
        fs::create_dir_all(&cdir).unwrap();
        fs::create_dir_all(&xdir).unwrap();
        fs::write(cdir.join("claude-id.jsonl"), format!("{}\n{}\ninvalid tail",
            json!({"type":"user","cwd":repo,"message":{"content":[{"type":"text","text":"Fix the menu"}]}}),
            json!({"type":"assistant","message":{"content":"Done","stop_reason":"end_turn"}}))).unwrap();
        let rollout = |id: &str, cwd: &str| format!("{}\n{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":id,"cwd":cwd}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Add the banner"}]}}),
            json!({"type":"event_msg","payload":{"type":"task_complete"}}));
        fs::write(xdir.join("rollout-one.jsonl"), rollout("codex-id", &format!("{worktree}/src"))).unwrap();
        fs::write(xdir.join("rollout-other.jsonl"), rollout("other-id", &format!("{repo}-other"))).unwrap();
        let found = sessions(&roots, claude.to_str(), codex.to_str());
        assert_eq!(found.len(), 2, "{found:?}");
        let c = found.iter().find(|s| s.agent == "claude").unwrap();
        assert_eq!((&*c.title, &*c.status, c.messages), ("Fix the menu", "finished", 2));
        let x = found.iter().find(|s| s.agent == "codex").unwrap();
        assert_eq!((&*x.path, &*x.title, &*x.status), (&*worktree, "Add the banner", "finished"));
        assert!(!belongs(&format!("{repo}-other"), &repo));
        fs::remove_dir_all(temp).unwrap();
    }
}
