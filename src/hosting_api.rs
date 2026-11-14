//! Pull requests, issues, comments, Launchpad, repositories to clone and SSH keys for every integration
//! (see hosting.rs for connections, sign-in and remotes).

use crate::hosting::{api_base, auth, config, host_of, http, label, secret_get, secret_name, Auth, RemoteRepo};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Clone, Debug, Default)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub source: String,
    pub source_url: String,
    pub source_sha: String,
    pub target: String,
    pub draft: bool,
    pub state: String,
    pub url: String,
    pub updated: String,
    pub body: String,
    /// "owner/repo" - set on Launchpad results that span repositories.
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
    /// Tracker key when it isn't just a number (Jira "ABC-123", Trello "#12").
    pub key: String,
    /// Which tracker it came from (github, gitlab, jira, trello, azure, bitbucket).
    pub tracker: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub created: String,
}

fn get(id: &str, path: &str) -> Result<Value, String> {
    crate::pagination::collect(&format!("{}{path}", api_base(id)), |url| crate::hosting::http_page("GET", url, &auth(id)?, None))
}

fn send(id: &str, method: &str, path: &str, body: Value) -> Result<Value, String> {
    http(method, &format!("{}{path}", api_base(id)), &auth(id)?, Some(body))
}

fn s(v: &Value) -> String {
    v.as_str().map(|x| x.to_string()).unwrap_or_else(|| if v.is_number() { v.to_string() } else { String::new() })
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn ms_to_iso(v: &Value) -> String {
    v.as_i64().and_then(chrono::DateTime::from_timestamp_millis).map(|d| d.to_rfc3339()).unwrap_or_default()
}

fn enc(path: &str) -> String {
    path.replace('/', "%2F")
}

pub(crate) fn q(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(crate) fn github_pr(v: &Value) -> PullRequest {
    PullRequest {
        number: v["number"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["user"]["login"]),
        source: s(&v["head"]["ref"]),
        source_url: s(&v["head"]["repo"]["clone_url"]), source_sha: s(&v["head"]["sha"]),
        target: s(&v["base"]["ref"]),
        draft: v["draft"].as_bool().unwrap_or(false),
        state: if v["merged_at"].is_string() { "merged".into() } else { s(&v["state"]) },
        url: s(&v["html_url"]),
        updated: s(&v["updated_at"]),
        body: s(&v["body"]),
        repo: s(&v["base"]["repo"]["full_name"]),
    }
}

pub(crate) fn gitlab_mr(v: &Value) -> PullRequest {
    PullRequest {
        number: v["iid"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["author"]["username"]),
        source: s(&v["source_branch"]),
        source_url: String::new(), source_sha: v["sha"].as_str().or(v["diff_refs"]["head_sha"].as_str()).unwrap_or_default().into(),
        target: s(&v["target_branch"]),
        draft: v["draft"].as_bool().unwrap_or(false) || v["work_in_progress"].as_bool().unwrap_or(false),
        state: s(&v["state"]),
        url: s(&v["web_url"]),
        updated: s(&v["updated_at"]),
        body: s(&v["description"]),
        repo: s(&v["references"]["full"]).split('!').next().unwrap_or_default().to_string(),
    }
}

pub(crate) fn bitbucket_pr(v: &Value) -> PullRequest {
    PullRequest {
        number: v["id"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["author"]["display_name"]),
        source: s(&v["source"]["branch"]["name"]),
        source_url: clone_link(&v["source"]["repository"]["links"]["clone"], "https"), source_sha: s(&v["source"]["commit"]["hash"]),
        target: s(&v["destination"]["branch"]["name"]),
        draft: v["draft"].as_bool().unwrap_or(false),
        state: s(&v["state"]).to_lowercase(),
        url: s(&v["links"]["html"]["href"]),
        updated: s(&v["updated_on"]),
        body: s(&v["description"]),
        repo: s(&v["destination"]["repository"]["full_name"]),
    }
}

fn bitbucket_server_pr(v: &Value) -> PullRequest {
    PullRequest {
        number: v["id"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["author"]["user"]["displayName"]),
        source: s(&v["fromRef"]["displayId"]),
        source_url: clone_link(&v["fromRef"]["repository"]["links"]["clone"], "http"), source_sha: s(&v["fromRef"]["latestCommit"]),
        target: s(&v["toRef"]["displayId"]),
        draft: v["draft"].as_bool().unwrap_or(false),
        state: s(&v["state"]).to_lowercase(),
        url: s(&v["links"]["self"][0]["href"]),
        updated: ms_to_iso(&v["updatedDate"]),
        body: s(&v["description"]),
        repo: format!("{}/{}", s(&v["toRef"]["repository"]["project"]["key"]), s(&v["toRef"]["repository"]["slug"])),
    }
}

pub(crate) fn azure_pr(v: &Value, web: &str) -> PullRequest {
    let id = v["pullRequestId"].as_u64().unwrap_or(0);
    PullRequest {
        number: id,
        title: s(&v["title"]),
        author: s(&v["createdBy"]["displayName"]),
        source: s(&v["sourceRefName"]).trim_start_matches("refs/heads/").to_string(),
        source_url: v["forkSource"]["repository"]["remoteUrl"].as_str().or(v["repository"]["remoteUrl"].as_str()).unwrap_or_default().into(), source_sha: s(&v["lastMergeSourceCommit"]["commitId"]),
        target: s(&v["targetRefName"]).trim_start_matches("refs/heads/").to_string(),
        draft: v["isDraft"].as_bool().unwrap_or(false),
        state: s(&v["status"]),
        url: if web.is_empty() { String::new() } else { format!("{web}/pullrequest/{id}") },
        updated: s(&v["creationDate"]),
        body: s(&v["description"]),
        repo: s(&v["repository"]["name"]),
    }
}

pub(crate) fn github_issue(v: &Value) -> Issue {
    Issue {
        number: v["number"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["user"]["login"]),
        state: s(&v["state"]),
        url: s(&v["html_url"]),
        labels: list(&v["labels"]).iter().map(|l| s(&l["name"])).collect(),
        updated: s(&v["updated_at"]),
        body: s(&v["body"]),
        repo: s(&v["repository_url"]).rsplitn(3, '/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"),
        tracker: "github".into(),
        ..Default::default()
    }
}

fn gitlab_issue(v: &Value) -> Issue {
    Issue {
        number: v["iid"].as_u64().unwrap_or(0),
        title: s(&v["title"]),
        author: s(&v["author"]["username"]),
        state: s(&v["state"]),
        url: s(&v["web_url"]),
        labels: list(&v["labels"]).iter().map(s).collect(),
        updated: s(&v["updated_at"]),
        body: s(&v["description"]),
        repo: s(&v["references"]["full"]).split('#').next().unwrap_or_default().to_string(),
        tracker: "gitlab".into(),
        ..Default::default()
    }
}

/// Jira Cloud (API 3) descriptions are Atlassian documents; keep their plain text.
fn adf_text(v: &Value, out: &mut String) {
    if let Some(t) = v["text"].as_str() {
        out.push_str(t);
    }
    for c in list(&v["content"]) {
        adf_text(&c, out);
    }
    if v["type"] == "paragraph" {
        out.push('\n');
    }
}

pub(crate) fn jira_issue(v: &Value, id: &str) -> Issue {
    let key = s(&v["key"]);
    let f = &v["fields"];
    let mut body = String::new();
    if f["description"].is_string() {
        body = s(&f["description"]);
    } else {
        adf_text(&f["description"], &mut body);
    }
    Issue {
        number: key.rsplit('-').next().and_then(|n| n.parse().ok()).unwrap_or(0),
        title: s(&f["summary"]),
        author: s(&f["reporter"]["displayName"]),
        state: s(&f["status"]["name"]),
        url: format!("https://{}/browse/{key}", host_of(id)),
        labels: list(&f["labels"]).iter().map(s).collect(),
        updated: s(&f["updated"]),
        body: body.trim().to_string(),
        repo: s(&f["project"]["key"]),
        key,
        tracker: id.into(),
    }
}

pub(crate) fn trello_issue(v: &Value) -> Issue {
    Issue {
        number: v["idShort"].as_u64().unwrap_or(0),
        title: s(&v["name"]),
        state: if v["closed"].as_bool().unwrap_or(false) { "closed".into() } else { "open".into() },
        url: s(&v["url"]),
        labels: list(&v["labels"]).iter().map(|l| s(&l["name"])).filter(|n| !n.is_empty()).collect(),
        updated: s(&v["dateLastActivity"]),
        body: s(&v["desc"]),
        key: format!("#{}", v["idShort"].as_u64().unwrap_or(0)),
        tracker: "trello".into(),
        ..Default::default()
    }
}

fn azure_parts(r: &RemoteRepo) -> (String, String, String) {
    let p: Vec<&str> = r.path.split('/').collect();
    (p.first().unwrap_or(&"").to_string(), p.get(1).unwrap_or(&"").to_string(), p.get(2).unwrap_or(&"").to_string())
}

/// Azure DevOps calls go to the remote's own organisation.
fn azure_url(r: &RemoteRepo, rest: &str) -> String {
    let (org, project, repo) = azure_parts(r);
    format!("https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repo}{rest}")
}

fn bbs_path(r: &RemoteRepo) -> String {
    let (p, repo) = r.path.split_once('/').unwrap_or((&r.path, ""));
    format!("/projects/{p}/repos/{repo}")
}

fn rget(r: &RemoteRepo, url: &str) -> Result<Value, String> {
    crate::pagination::collect(url, |next| crate::hosting::http_page("GET", next, &auth(&r.service)?, None))
}

fn rsend(r: &RemoteRepo, method: &str, url: &str, body: Value) -> Result<Value, String> {
    http(method, url, &auth(&r.service)?, Some(body))
}

pub fn pull_requests(r: &RemoteRepo) -> Result<Vec<PullRequest>, String> {
    let b = api_base(&r.service);
    Ok(match r.service.as_str() {
        "github" | "github-enterprise" => list(&rget(r, &format!("{b}/repos/{}/pulls?state=open&per_page=50", r.path))?).iter().map(github_pr).collect(),
        "gitlab" | "gitlab-self" => list(&rget(r, &format!("{b}/projects/{}/merge_requests?state=opened&per_page=50", enc(&r.path)))?).iter().map(gitlab_mr).collect(),
        "bitbucket" => list(&rget(r, &format!("{b}/repositories/{}/pullrequests?state=OPEN&pagelen=50", r.path))?["values"]).iter().map(bitbucket_pr).collect(),
        "bitbucket-server" => list(&rget(r, &format!("{b}{}/pull-requests?state=OPEN&limit=50", bbs_path(r)))?["values"]).iter().map(bitbucket_server_pr).collect(),
        "azure" => list(&rget(r, &azure_url(r, "/pullrequests?searchCriteria.status=active&api-version=7.1"))?["value"]).iter().map(|v| azure_pr(v, &r.web)).collect(),
        _ => vec![],
    })
}

pub fn pull_request(r: &RemoteRepo, number: u64) -> Result<PullRequest, String> {
    let base = api_base(&r.service);
    Ok(match r.service.as_str() {
        "github" | "github-enterprise" => github_pr(&rget(r, &format!("{base}/repos/{}/pulls/{number}", r.path))?),
        "gitlab" | "gitlab-self" => gitlab_mr(&rget(r, &format!("{base}/projects/{}/merge_requests/{number}", enc(&r.path)))?),
        "bitbucket" => bitbucket_pr(&rget(r, &format!("{base}/repositories/{}/pullrequests/{number}", r.path))?),
        "bitbucket-server" => bitbucket_server_pr(&rget(r, &format!("{base}{}/pull-requests/{number}", bbs_path(r)))?),
        "azure" => azure_pr(&rget(r, &azure_url(r, &format!("/pullrequests/{number}?api-version=7.1")))?, &r.web),
        _ => return Err("This hosting service does not support pull-request checkout.".into()),
    })
}

#[derive(Serialize)]
pub struct PrCheckout { pub url: String, pub reference: String, pub expected: String }

pub fn pr_checkout(r: &RemoteRepo, pr: &PullRequest) -> Result<PrCheckout, String> {
    let reference = match r.service.as_str() {
        "github" | "github-enterprise" => format!("refs/pull/{}/head", pr.number),
        "gitlab" | "gitlab-self" => format!("refs/merge-requests/{}/head", pr.number),
        "bitbucket-server" => format!("refs/pull-requests/{}/from", pr.number),
        _ => pr.source_sha.clone(),
    };
    let url = if matches!(r.service.as_str(), "github" | "github-enterprise" | "gitlab" | "gitlab-self" | "bitbucket-server") { r.web.clone() } else { pr.source_url.clone() };
    if !url.starts_with("https://") || reference.is_empty() || pr.source_sha.is_empty() {
        return Err("The pull request has no available source repository or commit. Refresh it or open it in the browser.".into());
    }
    Ok(PrCheckout { url, reference, expected: pr.source_sha.clone() })
}

/// A repository's issue tracker (Preferences > Issue Tracker); kind "" uses the hosting service.
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Tracker {
    pub kind: String,
    /// Jira project key, Trello board id, or Azure DevOps project.
    pub project: String,
}

pub fn issues(r: &RemoteRepo) -> Result<Vec<Issue>, String> {
    let b = api_base(&r.service);
    Ok(match r.service.as_str() {
        "github" | "github-enterprise" => list(&rget(r, &format!("{b}/repos/{}/issues?state=open&per_page=50", r.path))?).iter().filter(|i| i.get("pull_request").is_none()).map(github_issue).collect(),
        "gitlab" | "gitlab-self" => list(&rget(r, &format!("{b}/projects/{}/issues?state=opened&per_page=50", enc(&r.path)))?).iter().map(gitlab_issue).collect(),
        "bitbucket" => match rget(r, &format!("{b}/repositories/{}/issues?pagelen=50&q={}", r.path, q("state=\"new\" OR state=\"open\""))) {
            Ok(v) => list(&v["values"])
                .iter()
                .map(|i| Issue {
                    number: i["id"].as_u64().unwrap_or(0),
                    title: s(&i["title"]),
                    author: s(&i["reporter"]["display_name"]),
                    state: s(&i["state"]),
                    url: s(&i["links"]["html"]["href"]),
                    updated: s(&i["updated_on"]),
                    body: s(&i["content"]["raw"]),
                    tracker: "bitbucket".into(),
                    ..Default::default()
                })
                .collect(),
            Err(e) if e.starts_with("404") => vec![], // the repository has no issue tracker
            Err(e) => return Err(e),
        },
        "azure" => {
            let (_, project, _) = azure_parts(r);
            tracker_issues(&Tracker { kind: "azure".into(), project }, false)?
        }
        _ => vec![],
    })
}

/// Issues from a separate tracker (Jira, Trello, Azure Boards): for a project / board, or the ones assigned to you.
pub fn tracker_issues(t: &Tracker, mine: bool) -> Result<Vec<Issue>, String> {
    let id = t.kind.as_str();
    match id {
        "jira" | "jira-server" => {
            let jql = if !mine && !t.project.trim().is_empty() {
                format!("project = \"{}\" AND resolution = Unresolved ORDER BY updated DESC", t.project.trim())
            } else {
                "assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC".to_string()
            };
            let fields = "summary,status,updated,reporter,labels,description,project";
            let path = if id == "jira" { format!("/search/jql?jql={}&maxResults=50&fields={fields}", q(&jql)) } else { format!("/search?jql={}&maxResults=50&fields={fields}", q(&jql)) };
            Ok(list(&get(id, &path)?["issues"]).iter().map(|i| jira_issue(i, id)).collect())
        }
        "trello" => {
            let path = if !mine && !t.project.trim().is_empty() { format!("/boards/{}/cards/open", t.project.trim()) } else { "/members/me/cards?filter=open".into() };
            Ok(list(&get(id, &path)?).iter().map(trello_issue).collect())
        }
        "azure" => {
            let org = config().azure_org.trim().to_string();
            if org.is_empty() {
                return Err("Set your Azure DevOps organization in Preferences > Integrations.".into());
            }
            let scope = if mine || t.project.trim().is_empty() { "[System.AssignedTo] = @Me".to_string() } else { format!("[System.TeamProject] = '{}'", t.project.replace('\'', "''")) };
            let wiql = format!("Select [System.Id] From WorkItems Where {scope} And [System.State] Not In ('Closed','Done','Removed') Order By [System.ChangedDate] Desc");
            let a = auth("azure")?;
            let v = http("POST", &format!("https://dev.azure.com/{org}/_apis/wit/wiql?api-version=7.1&$top=50"), &a, Some(json!({ "query": wiql })))?;
            let ids: Vec<String> = list(&v["workItems"]).iter().take(50).map(|w| s(&w["id"])).collect();
            if ids.is_empty() {
                return Ok(vec![]);
            }
            let fields = "System.Title,System.State,System.ChangedDate,System.Tags,System.CreatedBy,System.TeamProject";
            let items = http("GET", &format!("https://dev.azure.com/{org}/_apis/wit/workitems?ids={}&fields={fields}&api-version=7.1", ids.join(",")), &a, None)?;
            Ok(list(&items["value"])
                .iter()
                .map(|w| {
                    let f = &w["fields"];
                    let n = w["id"].as_u64().unwrap_or(0);
                    Issue {
                        number: n,
                        title: s(&f["System.Title"]),
                        author: s(&f["System.CreatedBy"]["displayName"]),
                        state: s(&f["System.State"]),
                        url: format!("https://dev.azure.com/{org}/_workitems/edit/{n}"),
                        labels: s(&f["System.Tags"]).split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                        updated: s(&f["System.ChangedDate"]),
                        repo: s(&f["System.TeamProject"]),
                        tracker: "azure".into(),
                        ..Default::default()
                    }
                })
                .collect())
        }
        _ => Err("Choose an issue tracker".into()),
    }
}

/// "Create Issue" (IssueTrackerCreateForm): GitHub, GitLab, Bitbucket and Jira.
pub fn create_issue(r: Option<&RemoteRepo>, t: &Tracker, title: &str, body: &str) -> Result<Issue, String> {
    match t.kind.as_str() {
        "jira" | "jira-server" => {
            let id = t.kind.as_str();
            let description = if id == "jira" {
                json!({ "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": if body.is_empty() { " " } else { body } }] }] })
            } else {
                json!(body)
            };
            let v = send(id, "POST", "/issue", json!({ "fields": { "project": { "key": t.project.trim() }, "summary": title, "description": description, "issuetype": { "name": "Task" } } }))?;
            let key = s(&v["key"]);
            Ok(Issue { title: title.into(), url: format!("https://{}/browse/{key}", host_of(id)), number: key.rsplit('-').next().and_then(|n| n.parse().ok()).unwrap_or(0), key, tracker: id.into(), ..Default::default() })
        }
        _ => {
            let r = r.ok_or("This repository's remote isn't on a connected service.")?;
            let b = api_base(&r.service);
            match r.service.as_str() {
                "github" | "github-enterprise" => Ok(github_issue(&rsend(r, "POST", &format!("{b}/repos/{}/issues", r.path), json!({ "title": title, "body": body }))?)),
                "gitlab" | "gitlab-self" => Ok(gitlab_issue(&rsend(r, "POST", &format!("{b}/projects/{}/issues", enc(&r.path)), json!({ "title": title, "description": body }))?)),
                "bitbucket" => {
                    let v = rsend(r, "POST", &format!("{b}/repositories/{}/issues", r.path), json!({ "title": title, "content": { "raw": body } }))?;
                    Ok(Issue { number: v["id"].as_u64().unwrap_or(0), title: title.into(), url: s(&v["links"]["html"]["href"]), tracker: "bitbucket".into(), ..Default::default() })
                }
                _ => Err("Creating issues isn't available for this service.".into()),
            }
        }
    }
}

pub fn comments(r: &RemoteRepo, number: u64) -> Result<Vec<Comment>, String> {
    let b = api_base(&r.service);
    let c = |author: String, body: String, created: String| Comment { author, body, created };
    Ok(match r.service.as_str() {
        "github" | "github-enterprise" => list(&rget(r, &format!("{b}/repos/{}/issues/{number}/comments?per_page=100", r.path))?)
            .iter()
            .map(|x| c(s(&x["user"]["login"]), s(&x["body"]), s(&x["created_at"])))
            .collect(),
        "gitlab" | "gitlab-self" => list(&rget(r, &format!("{b}/projects/{}/merge_requests/{number}/notes?sort=asc&per_page=100", enc(&r.path)))?)
            .iter()
            .filter(|x| !x["system"].as_bool().unwrap_or(false))
            .map(|x| c(s(&x["author"]["username"]), s(&x["body"]), s(&x["created_at"])))
            .collect(),
        "bitbucket" => list(&rget(r, &format!("{b}/repositories/{}/pullrequests/{number}/comments?pagelen=100", r.path))?["values"])
            .iter()
            .filter(|x| !x["deleted"].as_bool().unwrap_or(false))
            .map(|x| c(s(&x["user"]["display_name"]), s(&x["content"]["raw"]), s(&x["created_on"])))
            .collect(),
        "bitbucket-server" => list(&rget(r, &format!("{b}{}/pull-requests/{number}/activities?limit=100", bbs_path(r)))?["values"])
            .iter()
            .filter(|x| x["action"] == "COMMENTED")
            .map(|x| c(s(&x["comment"]["author"]["displayName"]), s(&x["comment"]["text"]), ms_to_iso(&x["comment"]["createdDate"])))
            .rev()
            .collect(),
        "azure" => list(&rget(r, &azure_url(r, &format!("/pullRequests/{number}/threads?api-version=7.1")))?["value"])
            .iter()
            .flat_map(|t| list(&t["comments"]))
            .filter(|x| x["commentType"] == "text")
            .map(|x| c(s(&x["author"]["displayName"]), s(&x["content"]), s(&x["publishedDate"])))
            .collect(),
        _ => vec![],
    })
}

pub fn add_comment(r: &RemoteRepo, number: u64, body: &str) -> Result<(), String> {
    let b = api_base(&r.service);
    match r.service.as_str() {
        "github" | "github-enterprise" => rsend(r, "POST", &format!("{b}/repos/{}/issues/{number}/comments", r.path), json!({ "body": body })),
        "gitlab" | "gitlab-self" => rsend(r, "POST", &format!("{b}/projects/{}/merge_requests/{number}/notes", enc(&r.path)), json!({ "body": body })),
        "bitbucket" => rsend(r, "POST", &format!("{b}/repositories/{}/pullrequests/{number}/comments", r.path), json!({ "content": { "raw": body } })),
        "bitbucket-server" => rsend(r, "POST", &format!("{b}{}/pull-requests/{number}/comments", bbs_path(r)), json!({ "text": body })),
        "azure" => rsend(r, "POST", &azure_url(r, &format!("/pullRequests/{number}/threads?api-version=7.1")), json!({ "comments": [{ "parentCommentId": 0, "content": body, "commentType": 1 }], "status": 1 })),
        _ => Err("Not supported for this service".into()),
    }
    .map(|_| ())
}

pub fn create_pr(r: &RemoteRepo, source: &str, target: &str, title: &str, body: &str, draft: bool) -> Result<PullRequest, String> {
    let b = api_base(&r.service);
    match r.service.as_str() {
        "github" | "github-enterprise" => Ok(github_pr(&rsend(r, "POST", &format!("{b}/repos/{}/pulls", r.path), json!({ "title": title, "head": source, "base": target, "body": body, "draft": draft }))?)),
        "gitlab" | "gitlab-self" => {
            let title = if draft { format!("Draft: {title}") } else { title.to_string() };
            Ok(gitlab_mr(&rsend(r, "POST", &format!("{b}/projects/{}/merge_requests", enc(&r.path)), json!({ "source_branch": source, "target_branch": target, "title": title, "description": body }))?))
        }
        "bitbucket" => Ok(bitbucket_pr(&rsend(
            r,
            "POST",
            &format!("{b}/repositories/{}/pullrequests", r.path),
            json!({ "title": title, "description": body, "draft": draft, "source": { "branch": { "name": source } }, "destination": { "branch": { "name": target } } }),
        )?)),
        "bitbucket-server" => Ok(bitbucket_server_pr(&rsend(
            r,
            "POST",
            &format!("{b}{}/pull-requests", bbs_path(r)),
            json!({ "title": title, "description": body, "draft": draft, "fromRef": { "id": format!("refs/heads/{source}") }, "toRef": { "id": format!("refs/heads/{target}") } }),
        )?)),
        "azure" => Ok(azure_pr(
            &rsend(
                r,
                "POST",
                &azure_url(r, "/pullrequests?api-version=7.1"),
                json!({ "sourceRefName": format!("refs/heads/{source}"), "targetRefName": format!("refs/heads/{target}"), "title": title, "description": body, "isDraft": draft }),
            )?,
            &r.web,
        )),
        _ => Err("Not supported for this service".into()),
    }
}

/// method: "merge", "squash" or "rebase".
static MERGE_WAITERS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Weak<std::sync::atomic::AtomicBool>>>> = std::sync::OnceLock::new();
fn merge_key(r: &RemoteRepo, number: u64) -> String { format!("{}|{}|{}|{number}", r.service, r.host, r.path) }
struct MergeWait { key: String, cancelled: std::sync::Arc<std::sync::atomic::AtomicBool> }
impl MergeWait {
    fn new(r: &RemoteRepo, number: u64) -> Result<Self, String> {
        let key=merge_key(r,number);let mut waiters=MERGE_WAITERS.get_or_init(Default::default).lock().unwrap();
        if waiters.get(&key).and_then(std::sync::Weak::upgrade).is_some() { return Err("This pull request already has a merge request running.".into()); }
        let cancelled=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));waiters.insert(key.clone(),std::sync::Arc::downgrade(&cancelled));Ok(Self {key,cancelled})
    }
    fn is_cancelled(&self) -> bool { self.cancelled.load(std::sync::atomic::Ordering::Acquire) }
}
impl Drop for MergeWait { fn drop(&mut self) { MERGE_WAITERS.get_or_init(Default::default).lock().unwrap().remove(&self.key); } }
pub fn cancel_merge(r: &RemoteRepo, number: u64) -> bool {
    let flag=MERGE_WAITERS.get_or_init(Default::default).lock().unwrap().get(&merge_key(r,number)).and_then(std::sync::Weak::upgrade);
    if let Some(flag)=flag { flag.store(true,std::sync::atomic::Ordering::Release); true } else { false }
}

fn await_gitlab_rebase(mut read: impl FnMut() -> Result<Value,String>, mut pause: impl FnMut(), cancelled: impl Fn() -> bool, expected: &Value, timeout: std::time::Duration) -> Result<Value,String> {
    let start=std::time::Instant::now();
    loop {
        if cancelled() { return Err("Stopped waiting for the rebase. The server may still finish the queued rebase; no merge was sent by this operation.".into()); }
        if start.elapsed() >= timeout { return Err("The server rebase is still running. Refresh the pull request before merging.".into()); }
        pause();
        if cancelled() { continue; }
        let current=read()?;
        for field in ["source_project_id","source_branch","target_branch"] {
            if current[field] != expected[field] { return Err("The merge request source or target changed during rebase. Refresh it before merging.".into()); }
        }
        if let Some(error)=current["merge_error"].as_str().filter(|error|!error.is_empty()) { return Err(format!("The server could not rebase this merge request: {error}")); }
        match current["rebase_in_progress"].as_bool() {
            Some(true) => {},
            Some(false) if current["sha"].as_str().is_some_and(|sha| !sha.is_empty()) => return Ok(current),
            _ => return Err("The server did not report a completed rebase and source commit. Refresh before merging.".into()),
        }
    }
}

pub fn merge_pr(r: &RemoteRepo, number: u64, method: &str) -> Result<(), String> {
    let waiting = MergeWait::new(r, number)?;
    let b = api_base(&r.service);
    match r.service.as_str() {
        "github" | "github-enterprise" => rsend(r, "PUT", &format!("{b}/repos/{}/pulls/{number}/merge", r.path), json!({ "merge_method": method })).map(|_| ()),
        "gitlab" | "gitlab-self" => {
            let url = format!("{b}/projects/{}/merge_requests/{number}", enc(&r.path));
            let mut body = json!({ "squash": method == "squash" });
            if method == "rebase" {
                let expected = rget(r, &url)?;
                http("PUT", &format!("{url}/rebase"), &auth(&r.service)?, None)?;
                let completed = await_gitlab_rebase(|| rget(r, &format!("{url}?include_rebase_in_progress=true")), || {
                    for _ in 0..10 { if waiting.is_cancelled() { break; } std::thread::sleep(std::time::Duration::from_millis(100)); }
                }, || waiting.is_cancelled(), &expected, std::time::Duration::from_secs(60))?;
                body["sha"] = completed["sha"].clone();
            }
            if waiting.is_cancelled() { return Err("Merge cancelled before it was sent.".into()); }
            rsend(r, "PUT", &format!("{url}/merge"), body).map(|_| ())
        }
        "bitbucket" => {
            let strategy = match method {
                "squash" => "squash",
                "rebase" => "fast_forward",
                _ => "merge_commit",
            };
            rsend(r, "POST", &format!("{b}/repositories/{}/pullrequests/{number}/merge", r.path), json!({ "merge_strategy": strategy })).map(|_| ())
        }
        "bitbucket-server" => {
            let pr = rget(r, &format!("{b}{}/pull-requests/{number}", bbs_path(r)))?;
            let version = pr["version"].as_i64().unwrap_or(0);
            rsend(r, "POST", &format!("{b}{}/pull-requests/{number}/merge?version={version}", bbs_path(r)), json!({})).map(|_| ())
        }
        "azure" => {
            let pr = rget(r, &azure_url(r, &format!("/pullrequests/{number}?api-version=7.1")))?;
            let strategy = match method {
                "squash" => "squash",
                "rebase" => "rebase",
                _ => "noFastForward",
            };
            let body = json!({ "status": "completed", "lastMergeSourceCommit": pr["lastMergeSourceCommit"], "completionOptions": { "mergeStrategy": strategy } });
            rsend(r, "PATCH", &azure_url(r, &format!("/pullrequests/{number}?api-version=7.1")), body).map(|_| ())
        }
        _ => Err("Not supported for this service".into()),
    }
}

#[derive(Serialize, Default)]
pub struct Launchpad {
    pub pull_requests: Vec<PullRequest>,
    pub issues: Vec<Issue>,
    pub errors: Vec<String>,
}

/// Open pull requests and issues that involve you, across every connected service.
pub fn launchpad(_gitlab_host: &str) -> Launchpad {
    parallel_launchpad(&crate::hosting::SERVICES, launchpad_service)
}

fn parallel_launchpad(ids: &[&str], work: impl Fn(&str) -> Launchpad + Sync) -> Launchpad {
    let mut out = Launchpad::default();
    for group in ids.chunks(4) {
        std::thread::scope(|scope| {
            let tasks: Vec<_> = group.iter().map(|id| { let work = &work; scope.spawn(move || work(id)) }).collect();
            for task in tasks {
                match task.join() {
                    Ok(result) => { out.pull_requests.extend(result.pull_requests); out.issues.extend(result.issues); out.errors.extend(result.errors); },
                    Err(_) => out.errors.push("A connected service stopped unexpectedly; other results are available.".into()),
                }
            }
        });
    }
    out.pull_requests.sort_by(|a, b| b.updated.cmp(&a.updated));
    out.pull_requests.dedup_by(|a, b| a.url == b.url);
    out.issues.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

fn launchpad_service(only: &str) -> Launchpad {
    let mut out = Launchpad::default();
    let connected = |id: &str| id == only && secret_get(&secret_name(id)).is_some() && !host_of(id).is_empty();
    for id in ["github", "github-enterprise"] {
        if !connected(id) {
            continue;
        }
        match get(id, &format!("/search/issues?per_page=50&sort=updated&q={}", q("is:open is:pr involves:@me archived:false"))) {
            Ok(v) => out.pull_requests.extend(list(&v["items"]).iter().map(|i| {
                let mut p = PullRequest { number: i["number"].as_u64().unwrap_or(0), title: s(&i["title"]), author: s(&i["user"]["login"]), state: "open".into(), url: s(&i["html_url"]), updated: s(&i["updated_at"]), ..Default::default() };
                p.repo = github_issue(i).repo;
                p.draft = i["draft"].as_bool().unwrap_or(false);
                p
            })),
            Err(e) => out.errors.push(format!("{}: {e}", label(id))),
        }
        match get(id, &format!("/search/issues?per_page=50&sort=updated&q={}", q("is:open is:issue assignee:@me archived:false"))) {
            Ok(v) => out.issues.extend(list(&v["items"]).iter().map(github_issue)),
            Err(e) => out.errors.push(format!("{}: {e}", label(id))),
        }
    }
    for id in ["gitlab", "gitlab-self"] {
        if !connected(id) {
            continue;
        }
        for scope in ["created_by_me", "assigned_to_me"] {
            match get(id, &format!("/merge_requests?state=opened&scope={scope}&per_page=50")) {
                Ok(v) => out.pull_requests.extend(list(&v).iter().map(gitlab_mr)),
                Err(e) => out.errors.push(format!("{}: {e}", label(id))),
            }
        }
        match get(id, "/issues?state=opened&scope=assigned_to_me&per_page=50") {
            Ok(v) => out.issues.extend(list(&v).iter().map(gitlab_issue)),
            Err(e) => out.errors.push(format!("{}: {e}", label(id))),
        }
    }
    if connected("bitbucket") {
        match get("bitbucket", "/user").map(|u| s(&u["uuid"])) {
            Ok(uuid) => match get("bitbucket", &format!("/pullrequests/{}?state=OPEN&pagelen=50", q(&uuid))) {
                Ok(v) => out.pull_requests.extend(list(&v["values"]).iter().map(bitbucket_pr)),
                Err(e) => out.errors.push(format!("Bitbucket: {e}")),
            },
            Err(e) => out.errors.push(format!("Bitbucket: {e}")),
        }
    }
    if connected("bitbucket-server") {
        match get("bitbucket-server", "/dashboard/pull-requests?state=OPEN&limit=50") {
            Ok(v) => out.pull_requests.extend(list(&v["values"]).iter().map(bitbucket_server_pr)),
            Err(e) => out.errors.push(format!("Bitbucket Data Center: {e}")),
        }
    }
    let org = config().azure_org.trim().to_string();
    if connected("azure") && !org.is_empty() {
        if let Ok(a) = auth("azure") {
            match http("GET", &format!("https://dev.azure.com/{org}/_apis/git/pullrequests?searchCriteria.status=active&$top=50&api-version=7.1"), &a, None) {
                Ok(v) => out.pull_requests.extend(list(&v["value"]).iter().map(|p| {
                    let mut pr = azure_pr(p, "");
                    pr.url = format!("https://dev.azure.com/{org}/{}/_git/{}/pullrequest/{}", s(&p["repository"]["project"]["name"]), s(&p["repository"]["name"]), pr.number);
                    pr
                })),
                Err(e) => out.errors.push(format!("Azure DevOps: {e}")),
            }
        }
        match tracker_issues(&Tracker { kind: "azure".into(), project: String::new() }, true) {
            Ok(i) => out.issues.extend(i),
            Err(e) => out.errors.push(format!("Azure DevOps: {e}")),
        }
    }
    for id in ["jira", "jira-server", "trello"] {
        if !connected(id) {
            continue;
        }
        match tracker_issues(&Tracker { kind: id.into(), project: String::new() }, true) {
            Ok(i) => out.issues.extend(i),
            Err(e) => out.errors.push(format!("{}: {e}", label(id))),
        }
    }
    out.pull_requests.sort_by(|a, b| b.updated.cmp(&a.updated));
    out.pull_requests.dedup_by(|a, b| a.url == b.url);
    out.issues.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct HostedRepo {
    pub name: String,
    pub full_name: String,
    pub description: String,
    pub private: bool,
    pub https_url: String,
    pub ssh_url: String,
    pub updated: String,
}

fn clone_link(v: &Value, name: &str) -> String {
    list(v).iter().find(|l| l["name"] == name).map(|l| s(&l["href"])).unwrap_or_default()
}

/// Your repositories on a connected service, for finding a repository to clone.
pub fn repositories(id: &str) -> Result<Vec<HostedRepo>, String> {
    let repo = |name: String, full_name: String, description: String, private: bool, https_url: String, ssh_url: String, updated: String| HostedRepo { name, full_name, description, private, https_url, ssh_url, updated };
    Ok(match id {
        "github" | "github-enterprise" => list(&get(id, "/user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member")?)
            .iter()
            .map(|r| repo(s(&r["name"]), s(&r["full_name"]), s(&r["description"]), r["private"].as_bool().unwrap_or(false), s(&r["clone_url"]), s(&r["ssh_url"]), s(&r["pushed_at"])))
            .collect(),
        "gitlab" | "gitlab-self" => list(&get(id, "/projects?membership=true&per_page=100&order_by=last_activity_at")?)
            .iter()
            .map(|r| repo(s(&r["path"]), s(&r["path_with_namespace"]), s(&r["description"]), r["visibility"] != "public", s(&r["http_url_to_repo"]), s(&r["ssh_url_to_repo"]), s(&r["last_activity_at"])))
            .collect(),
        "bitbucket" => list(&get(id, "/repositories?role=member&pagelen=100&sort=-updated_on")?["values"])
            .iter()
            .map(|r| {
                let l = &r["links"]["clone"];
                repo(s(&r["slug"]), s(&r["full_name"]), s(&r["description"]), r["is_private"].as_bool().unwrap_or(false), clone_link(l, "https"), clone_link(l, "ssh"), s(&r["updated_on"]))
            })
            .collect(),
        "bitbucket-server" => list(&get(id, "/repos?limit=100")?["values"])
            .iter()
            .map(|r| {
                let l = &r["links"]["clone"];
                let full = format!("{}/{}", s(&r["project"]["key"]), s(&r["slug"]));
                repo(s(&r["slug"]), full, s(&r["description"]), !r["public"].as_bool().unwrap_or(false), clone_link(l, "http"), clone_link(l, "ssh"), String::new())
            })
            .collect(),
        "azure" => list(&get(id, "/_apis/git/repositories?api-version=7.1")?["value"])
            .iter()
            .map(|r| {
                let full = format!("{}/{}", s(&r["project"]["name"]), s(&r["name"]));
                repo(s(&r["name"]), full, String::new(), r["project"]["visibility"] != "public", s(&r["remoteUrl"]), s(&r["sshUrl"]), String::new())
            })
            .collect(),
        _ => return Err(format!("{} doesn't host repositories.", label(id))),
    })
}

/// The service's page for adding an SSH key by hand.
pub fn ssh_keys_page(id: &str) -> String {
    let h = host_of(id);
    match id {
        "github" | "github-enterprise" => format!("https://{h}/settings/ssh/new"),
        "gitlab" | "gitlab-self" => format!("https://{h}/-/user_settings/ssh_keys"),
        "bitbucket" => "https://bitbucket.org/account/settings/ssh-keys/".into(),
        "bitbucket-server" => format!("https://{h}/plugins/servlet/ssh/account/keys"),
        "azure" => format!("https://dev.azure.com/{}/_usersSettings/keys", config().azure_org.trim()),
        _ => String::new(),
    }
}

/// "Add SSH key to {service}" (Services-AddToService). GitHub needs a token with the write:public_key scope.
pub fn add_ssh_key(id: &str, title: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    match id {
        "github" | "github-enterprise" | "gitlab" | "gitlab-self" => send(id, "POST", "/user/keys", json!({ "title": title, "key": key })).map(|_| ()),
        "bitbucket" => {
            let uuid = s(&get(id, "/user")?["uuid"]);
            send(id, "POST", &format!("/users/{}/ssh-keys", q(&uuid)), json!({ "key": key, "label": title })).map(|_| ())
        }
        "bitbucket-server" => http("POST", &format!("https://{}/rest/ssh/1.0/keys", host_of(id)), &auth(id)?, Some(json!({ "text": key, "label": title }))).map(|_| ()),
        _ => Err(format!("{} doesn't accept SSH keys from other apps; copy the key and paste it on its page.", label(id))),
    }
}

#[allow(dead_code)]
fn _uses(_: Auth) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_api_answers() {
        let pr = github_pr(&json!({ "number": 7, "title": "Add x", "user": { "login": "sam" }, "head": { "ref": "feat" }, "base": { "ref": "main", "repo": { "full_name": "o/r" } }, "draft": true, "state": "open", "html_url": "u" }));
        assert_eq!((pr.number, pr.source.as_str(), pr.target.as_str(), pr.draft, pr.repo.as_str()), (7, "feat", "main", true, "o/r"));
        let mr = gitlab_mr(&json!({ "iid": 3, "title": "Fix", "author": { "username": "jo" }, "source_branch": "a", "target_branch": "b", "state": "opened", "references": { "full": "g/p!3" } }));
        assert_eq!((mr.number, mr.repo.as_str()), (3, "g/p"));
        let issue = github_issue(&json!({ "number": 9, "title": "Bug", "user": { "login": "x" }, "repository_url": "https://api.github.com/repos/o/r", "labels": [{ "name": "bug" }] }));
        assert_eq!((issue.repo.as_str(), issue.labels.clone()), ("o/r", vec!["bug".to_string()]));
        let bb = bitbucket_pr(&json!({ "id": 5, "title": "T", "author": { "display_name": "Ann" }, "source": { "branch": { "name": "f" } }, "destination": { "branch": { "name": "main" }, "repository": { "full_name": "t/a" } }, "state": "OPEN", "links": { "html": { "href": "h" } } }));
        assert_eq!((bb.number, bb.source.as_str(), bb.state.as_str(), bb.repo.as_str()), (5, "f", "open", "t/a"));
        let az = azure_pr(&json!({ "pullRequestId": 12, "title": "T", "sourceRefName": "refs/heads/feat", "targetRefName": "refs/heads/main", "isDraft": false, "status": "active" }), "https://dev.azure.com/o/p/_git/r");
        assert_eq!((az.source.as_str(), az.target.as_str(), az.url.as_str()), ("feat", "main", "https://dev.azure.com/o/p/_git/r/pullrequest/12"));
        let j = jira_issue(&json!({ "key": "ABC-42", "fields": { "summary": "Do it", "status": { "name": "To Do" }, "description": { "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hello" }] }] } } }), "jira");
        assert_eq!((j.number, j.key.as_str(), j.state.as_str(), j.body.as_str()), (42, "ABC-42", "To Do", "Hello"));
        let t = trello_issue(&json!({ "idShort": 4, "name": "Card", "url": "u", "closed": false, "labels": [{ "name": "ui" }, { "name": "" }] }));
        assert_eq!((t.key.as_str(), t.labels.clone()), ("#4", vec!["ui".to_string()]));
        assert_eq!(q("a b/c"), "a%20b%2Fc");
    }
}


#[cfg(test)]
mod concurrency_tests {
    use super::*;
    #[test]
    fn launchpad_bounds_workers_and_preserves_partial_errors() {
        use std::sync::atomic::{AtomicUsize,Ordering};
        let active=AtomicUsize::new(0);let peak=AtomicUsize::new(0);
        let result=parallel_launchpad(&["a","b","c","d","e","f"],|id| {
            let count=active.fetch_add(1,Ordering::SeqCst)+1;peak.fetch_max(count,Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(25));active.fetch_sub(1,Ordering::SeqCst);
            Launchpad {errors:vec![id.into()],..Default::default()}
        });
        assert_eq!(result.errors.len(),6);assert!(peak.load(Ordering::SeqCst)>1);assert!(peak.load(Ordering::SeqCst)<=4);
    }
}


#[cfg(test)]
mod rebase_wait_tests {
    use super::*;
    fn state(running:bool)->Value {json!({"source_project_id":1,"source_branch":"feature","target_branch":"main","rebase_in_progress":running,"sha":"rebased"})}
    #[test]
    fn waits_for_rebase_and_rejects_errors_cancellation_timeout_and_changed_source() {
        let expected=state(true);let mut polls=0;
        let completed=await_gitlab_rebase(||{polls+=1;Ok(state(polls<3))},||{},||false,&expected,std::time::Duration::from_secs(5)).unwrap();assert_eq!(polls,3);assert_eq!(completed["sha"],"rebased");
        let mut failed=state(false);failed["merge_error"]=json!("conflict");assert!(await_gitlab_rebase(||Ok(failed.clone()),||{},||false,&expected,std::time::Duration::from_secs(5)).unwrap_err().contains("conflict"));
        assert!(await_gitlab_rebase(||panic!("cancelled"),||{},||true,&expected,std::time::Duration::from_secs(5)).is_err());
        assert!(await_gitlab_rebase(||panic!("expired"),||{},||false,&expected,std::time::Duration::ZERO).is_err());
        let mut changed=state(false);changed["source_branch"]=json!("other");assert!(await_gitlab_rebase(||Ok(changed.clone()),||{},||false,&expected,std::time::Duration::from_secs(5)).is_err());
    }
}
