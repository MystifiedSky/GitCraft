//! Stage 6: GitHub and GitLab — pull requests (merge requests), issues and the Launchpad.
//! Tokens live in the operating system's credential store (Windows Credential Manager,
//! macOS Keychain, Linux keyring), never in GitCraft's settings file.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SERVICE: &str = "GitCraft";

pub fn secret_get(name: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, name).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn secret_set(name: &str, value: &str) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
    if value.is_empty() {
        let _ = e.delete_credential();
        Ok(())
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Gitlab,
}

/// Where a remote lives: provider, API host and the project path ("owner/repo" or "group/sub/project").
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RemoteRepo {
    pub provider: Provider,
    pub host: String,
    pub path: String,
    pub web: String,
}

/// Understands https://host/owner/repo(.git), git@host:owner/repo(.git) and ssh://git@host/owner/repo.
pub fn parse_remote(url: &str, gitlab_host: &str) -> Option<RemoteRepo> {
    let u = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let (host, path) = if let Some(rest) = u.strip_prefix("git@") {
        let (h, p) = rest.split_once(':')?;
        (h.to_string(), p.to_string())
    } else {
        let rest = u.split_once("://").map(|(_, r)| r).unwrap_or(u);
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (h, p) = rest.split_once('/')?;
        (h.split(':').next().unwrap_or(h).to_string(), p.to_string())
    };
    let provider = if host == "github.com" {
        Provider::Github
    } else if host == "gitlab.com" || (!gitlab_host.is_empty() && host == gitlab_host) {
        Provider::Gitlab
    } else {
        return None;
    };
    Some(RemoteRepo { web: format!("https://{host}/{path}"), provider, host, path })
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub source: String,
    pub target: String,
    pub draft: bool,
    pub state: String,
    pub url: String,
    pub updated: String,
    pub body: String,
    /// "owner/repo" — set on Launchpad results that span repositories.
    pub repo: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub state: String,
    pub url: String,
    pub labels: Vec<String>,
    pub updated: String,
    pub body: String,
    pub repo: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub created: String,
}

fn token_name(p: &Provider) -> &'static str {
    match p {
        Provider::Github => "github-token",
        Provider::Gitlab => "gitlab-token",
    }
}

fn api_base(r: &RemoteRepo) -> String {
    match r.provider {
        Provider::Github => "https://api.github.com".into(),
        Provider::Gitlab => format!("https://{}/api/v4", r.host),
    }
}

fn enc(path: &str) -> String {
    path.replace('/', "%2F")
}

fn request(provider: &Provider, method: &str, url: &str, body: Option<Value>) -> Result<Value, String> {
    let token = secret_get(token_name(provider)).ok_or_else(|| {
        format!("Connect {} first: add a personal access token in Accounts.", if *provider == Provider::Github { "GitHub" } else { "GitLab" })
    })?;
    let mut req = ureq::request(method, url).set("User-Agent", "GitCraft").timeout(std::time::Duration::from_secs(30));
    req = match provider {
        Provider::Github => req.set("Authorization", &format!("Bearer {token}")).set("Accept", "application/vnd.github+json").set("X-GitHub-Api-Version", "2022-11-28"),
        Provider::Gitlab => req.set("PRIVATE-TOKEN", &token),
    };
    let res = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    };
    match res {
        Ok(r) => {
            let text = r.into_string().map_err(|e| e.to_string())?;
            Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).map_err(|e| e.to_string())? })
        }
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["message"].as_str().map(|s| s.to_string())).unwrap_or(text);
            Err(format!("{code}: {}", msg.chars().take(300).collect::<String>()))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn github_pr(v: &Value) -> PullRequest {
    PullRequest {
        number: v["number"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["user"]["login"]),
        source: s(&v["head"]["ref"]),
        target: s(&v["base"]["ref"]),
        draft: v["draft"].as_bool().unwrap_or(false),
        state: if v["merged_at"].is_string() { "merged".into() } else { s(&v["state"]) },
        url: s(&v["html_url"]),
        updated: s(&v["updated_at"]),
        body: s(&v["body"]),
        repo: s(&v["base"]["repo"]["full_name"]),
    }
}

fn gitlab_mr(v: &Value) -> PullRequest {
    PullRequest {
        number: v["iid"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["author"]["username"]),
        source: s(&v["source_branch"]),
        target: s(&v["target_branch"]),
        draft: v["draft"].as_bool().unwrap_or(false) || v["work_in_progress"].as_bool().unwrap_or(false),
        state: s(&v["state"]),
        url: s(&v["web_url"]),
        updated: s(&v["updated_at"]),
        body: s(&v["description"]),
        repo: s(&v["references"]["full"]).split('!').next().unwrap_or_default().to_string(),
    }
}

fn github_issue(v: &Value) -> Issue {
    Issue {
        number: v["number"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["user"]["login"]),
        state: s(&v["state"]),
        url: s(&v["html_url"]),
        labels: v["labels"].as_array().map(|a| a.iter().map(|l| s(&l["name"])).collect()).unwrap_or_default(),
        updated: s(&v["updated_at"]),
        body: s(&v["body"]),
        repo: s(&v["repository_url"]).rsplitn(3, '/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"),
    }
}

fn gitlab_issue(v: &Value) -> Issue {
    Issue {
        number: v["iid"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["author"]["username"]),
        state: s(&v["state"]),
        url: s(&v["web_url"]),
        labels: v["labels"].as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default(),
        updated: s(&v["updated_at"]),
        body: s(&v["description"]),
        repo: s(&v["references"]["full"]).split('#').next().unwrap_or_default().to_string(),
    }
}

pub fn pull_requests(r: &RemoteRepo) -> Result<Vec<PullRequest>, String> {
    let b = api_base(r);
    match r.provider {
        Provider::Github => {
            let v = request(&r.provider, "GET", &format!("{b}/repos/{}/pulls?state=open&per_page=50", r.path), None)?;
            Ok(v.as_array().map(|a| a.iter().map(github_pr).collect()).unwrap_or_default())
        }
        Provider::Gitlab => {
            let v = request(&r.provider, "GET", &format!("{b}/projects/{}/merge_requests?state=opened&per_page=50", enc(&r.path)), None)?;
            Ok(v.as_array().map(|a| a.iter().map(gitlab_mr).collect()).unwrap_or_default())
        }
    }
}

pub fn issues(r: &RemoteRepo) -> Result<Vec<Issue>, String> {
    let b = api_base(r);
    match r.provider {
        Provider::Github => {
            let v = request(&r.provider, "GET", &format!("{b}/repos/{}/issues?state=open&per_page=50", r.path), None)?;
            // GitHub lists pull requests as issues too; keep real issues only.
            Ok(v.as_array().map(|a| a.iter().filter(|i| i.get("pull_request").is_none()).map(github_issue).collect()).unwrap_or_default())
        }
        Provider::Gitlab => {
            let v = request(&r.provider, "GET", &format!("{b}/projects/{}/issues?state=opened&per_page=50", enc(&r.path)), None)?;
            Ok(v.as_array().map(|a| a.iter().map(gitlab_issue).collect()).unwrap_or_default())
        }
    }
}

pub fn comments(r: &RemoteRepo, number: u64) -> Result<Vec<Comment>, String> {
    let b = api_base(r);
    let v = match r.provider {
        Provider::Github => request(&r.provider, "GET", &format!("{b}/repos/{}/issues/{number}/comments?per_page=100", r.path), None)?,
        Provider::Gitlab => request(&r.provider, "GET", &format!("{b}/projects/{}/merge_requests/{number}/notes?sort=asc&per_page=100", enc(&r.path)), None)?,
    };
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter(|c| !c["system"].as_bool().unwrap_or(false))
                .map(|c| Comment {
                    author: if r.provider == Provider::Github { s(&c["user"]["login"]) } else { s(&c["author"]["username"]) },
                    body: s(&c["body"]),
                    created: s(&c["created_at"]),
                })
                .collect()
        })
        .unwrap_or_default())
}

pub fn add_comment(r: &RemoteRepo, number: u64, body: &str) -> Result<(), String> {
    let b = api_base(r);
    match r.provider {
        Provider::Github => request(&r.provider, "POST", &format!("{b}/repos/{}/issues/{number}/comments", r.path), Some(json!({ "body": body }))),
        Provider::Gitlab => request(&r.provider, "POST", &format!("{b}/projects/{}/merge_requests/{number}/notes", enc(&r.path)), Some(json!({ "body": body }))),
    }
    .map(|_| ())
}

pub fn create_pr(r: &RemoteRepo, source: &str, target: &str, title: &str, body: &str, draft: bool) -> Result<PullRequest, String> {
    let b = api_base(r);
    match r.provider {
        Provider::Github => {
            let v = request(&r.provider, "POST", &format!("{b}/repos/{}/pulls", r.path), Some(json!({ "title": title, "head": source, "base": target, "body": body, "draft": draft })))?;
            Ok(github_pr(&v))
        }
        Provider::Gitlab => {
            let title = if draft { format!("Draft: {title}") } else { title.to_string() };
            let v = request(
                &r.provider,
                "POST",
                &format!("{b}/projects/{}/merge_requests", enc(&r.path)),
                Some(json!({ "source_branch": source, "target_branch": target, "title": title, "description": body })),
            )?;
            Ok(gitlab_mr(&v))
        }
    }
}

/// method: "merge", "squash" or "rebase".
pub fn merge_pr(r: &RemoteRepo, number: u64, method: &str) -> Result<(), String> {
    let b = api_base(r);
    match r.provider {
        Provider::Github => request(&r.provider, "PUT", &format!("{b}/repos/{}/pulls/{number}/merge", r.path), Some(json!({ "merge_method": method }))).map(|_| ()),
        Provider::Gitlab => {
            if method == "rebase" {
                request(&r.provider, "PUT", &format!("{b}/projects/{}/merge_requests/{number}/rebase", enc(&r.path)), None)?;
            }
            request(&r.provider, "PUT", &format!("{b}/projects/{}/merge_requests/{number}/merge", enc(&r.path)), Some(json!({ "squash": method == "squash" }))).map(|_| ())
        }
    }
}

/// Who the token belongs to (to show "Connected as …").
pub fn whoami(provider: &Provider, gitlab_host: &str) -> Result<String, String> {
    let url = match provider {
        Provider::Github => "https://api.github.com/user".to_string(),
        Provider::Gitlab => format!("https://{}/api/v4/user", if gitlab_host.is_empty() { "gitlab.com" } else { gitlab_host }),
    };
    let v = request(provider, "GET", &url, None)?;
    Ok(if *provider == Provider::Github { s(&v["login"]) } else { s(&v["username"]) })
}

#[derive(Serialize, Default)]
pub struct Launchpad {
    pub pull_requests: Vec<PullRequest>,
    pub issues: Vec<Issue>,
    pub errors: Vec<String>,
}

/// Open pull requests and issues that involve you, across every connected service.
pub fn launchpad(gitlab_host: &str) -> Launchpad {
    let mut out = Launchpad::default();
    if secret_get(token_name(&Provider::Github)).is_some() {
        let base = "https://api.github.com/search/issues?per_page=50&sort=updated&q=";
        match request(&Provider::Github, "GET", &format!("{base}is:open+is:pr+involves:@me+archived:false"), None) {
            Ok(v) => out.pull_requests.extend(v["items"].as_array().into_iter().flatten().map(|i| {
                let mut p = PullRequest { number: i["number"].as_u64().unwrap_or(0), title: s(&i["title"]), author: s(&i["user"]["login"]), state: "open".into(), url: s(&i["html_url"]), updated: s(&i["updated_at"]), ..Default::default() };
                p.repo = github_issue(i).repo;
                p.draft = i["draft"].as_bool().unwrap_or(false);
                p
            })),
            Err(e) => out.errors.push(format!("GitHub: {e}")),
        }
        match request(&Provider::Github, "GET", &format!("{base}is:open+is:issue+assignee:@me+archived:false"), None) {
            Ok(v) => out.issues.extend(v["items"].as_array().into_iter().flatten().map(github_issue)),
            Err(e) => out.errors.push(format!("GitHub: {e}")),
        }
    }
    if secret_get(token_name(&Provider::Gitlab)).is_some() {
        let host = if gitlab_host.is_empty() { "gitlab.com" } else { gitlab_host };
        for scope in ["created_by_me", "assigned_to_me"] {
            match request(&Provider::Gitlab, "GET", &format!("https://{host}/api/v4/merge_requests?state=opened&scope={scope}&per_page=50"), None) {
                Ok(v) => out.pull_requests.extend(v.as_array().into_iter().flatten().map(gitlab_mr)),
                Err(e) => out.errors.push(format!("GitLab: {e}")),
            }
        }
        match request(&Provider::Gitlab, "GET", &format!("https://{host}/api/v4/issues?state=opened&scope=assigned_to_me&per_page=50"), None) {
            Ok(v) => out.issues.extend(v.as_array().into_iter().flatten().map(gitlab_issue)),
            Err(e) => out.errors.push(format!("GitLab: {e}")),
        }
    }
    out.pull_requests.sort_by(|a, b| b.updated.cmp(&a.updated));
    out.pull_requests.dedup_by(|a, b| a.url == b.url);
    out.issues.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

/// "Create branch from issue": issue-123-short-title.
pub fn branch_for_issue(number: u64, title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    format!("issue-{number}-{slug}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understands_remote_urls() {
        let gh = parse_remote("https://github.com/octo/hello.git", "").unwrap();
        assert_eq!((gh.provider.clone(), gh.path.as_str(), gh.web.as_str()), (Provider::Github, "octo/hello", "https://github.com/octo/hello"));
        assert_eq!(parse_remote("git@github.com:octo/hello.git", "").unwrap().path, "octo/hello");
        let gl = parse_remote("ssh://git@gitlab.com/group/sub/project.git", "").unwrap();
        assert_eq!((gl.provider, gl.path.as_str()), (Provider::Gitlab, "group/sub/project"));
        assert_eq!(parse_remote("https://git.example.org/team/app", "git.example.org").unwrap().provider, Provider::Gitlab);
        assert!(parse_remote("https://bitbucket.org/a/b.git", "").is_none());
        assert!(parse_remote("C:/repos/local.git", "").is_none());
    }

    #[test]
    fn issue_branch_names() {
        assert_eq!(branch_for_issue(42, "Crash when saving: file > 2 GB!"), "issue-42-crash-when-saving-file-2-gb");
    }

    #[test]
    fn maps_api_answers() {
        let pr = github_pr(&json!({ "number": 7, "title": "Add x", "user": { "login": "sam" }, "head": { "ref": "feat" }, "base": { "ref": "main", "repo": { "full_name": "o/r" } }, "draft": true, "state": "open", "html_url": "u" }));
        assert_eq!((pr.number, pr.source.as_str(), pr.target.as_str(), pr.draft, pr.repo.as_str()), (7, "feat", "main", true, "o/r"));
        let mr = gitlab_mr(&json!({ "iid": 3, "title": "Fix", "author": { "username": "jo" }, "source_branch": "a", "target_branch": "b", "state": "opened", "references": { "full": "g/p!3" } }));
        assert_eq!((mr.number, mr.repo.as_str()), (3, "g/p"));
        let issue = github_issue(&json!({ "number": 9, "title": "Bug", "user": { "login": "x" }, "repository_url": "https://api.github.com/repos/o/r", "labels": [{ "name": "bug" }] }));
        assert_eq!((issue.repo.as_str(), issue.labels.clone()), ("o/r", vec!["bug".to_string()]));
    }
}
