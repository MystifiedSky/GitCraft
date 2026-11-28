//! Create an empty hosted repository, connect it, and publish one reviewed branch.
use crate::{git, hosting, hosting_api::q};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    pub label: String,
}

pub fn targets(service: &str) -> Result<Vec<Target>, String> {
    let base = hosting::api_base(service);
    if hosting::host_of(service).is_empty() || base.is_empty() || (service == "azure" && base.ends_with('/')) {
        return Err("Configure the server or organization in Preferences > Integrations.".into());
    }
    let get = |path: &str| {
        hosting::http(
            "GET",
            &format!("{base}{path}"),
            &hosting::auth(service)?,
            None,
        )
    };
    let collect = |path: &str| {
        let url = format!("{base}{path}");
        crate::pagination::collect(&url, |url| hosting::http_page("GET", url, &hosting::auth(service)?, None))
    };
    let pairs: Vec<(String, String)> = match service {
        "gitea" | "forgejo" | "codeberg" => {
            let profile = get("/user")?;
            let name = profile["login"].as_str().ok_or("Missing hosting account")?.to_owned();
            let mut destinations = vec![(name.clone(), format!("{name} (personal)"))];
            let groups = collect("/user/orgs?limit=50")?;
            destinations.extend(groups.as_array().into_iter().flatten().filter_map(|group| {
                let name = group["username"].as_str().or(group["name"].as_str())?;
                Some((format!("org:{name}"), name.to_owned()))
            }));
            destinations
        }
        "github" | "github-enterprise" => {
            let v = get("/user")?;
            let name = v["login"]
                .as_str()
                .ok_or("Missing GitHub account")?
                .to_string();
            let mut destinations = vec![(name.clone(), format!("{name} (personal)"))];
            let organizations = collect("/user/orgs?per_page=100")?;
            destinations.extend(organizations.as_array().into_iter().flatten().filter_map(|org| {
                let login = org["login"].as_str()?;
                Some((format!("org:{login}"), login.to_owned()))
            }));
            destinations
        }
        "gitlab" | "gitlab-self" => {
            let v = get("/user")?;
            let name = v["username"]
                .as_str()
                .ok_or("Missing GitLab account")?
                .to_string();
            let mut destinations = vec![(name.clone(), format!("{name} (personal)"))];
            let groups = collect("/groups?min_access_level=30&per_page=100")?;
            destinations.extend(groups.as_array().into_iter().flatten().filter_map(|group| {
                Some((format!("group:{}", group["id"].as_u64()?), group["full_path"].as_str()?.to_owned()))
            }));
            destinations
        }
        "bitbucket" => {
            let v = collect("/user/workspaces?pagelen=100")?;
            v["values"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| {
                    let workspace = &v["workspace"];
                    let slug = workspace["slug"].as_str()?;
                    Some((slug.into(), workspace["name"].as_str().unwrap_or(slug).into()))
                })
                .collect()
        }
        "bitbucket-server" => {
            let projects = collect("/projects?limit=100")?;
            projects["values"].as_array().into_iter().flatten()
                .filter_map(|project| Some((project["key"].as_str()?.into(), project["name"].as_str()?.into())))
                .collect()
        }
        "azure" | "azure-server" => {
            let version = if service == "azure" { "7.1" } else { "7.0" };
            let v = collect(&format!("/_apis/projects?api-version={version}"))?;
            v["value"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| Some((v["id"].as_str()?.into(), v["name"].as_str()?.into())))
                .collect()
        }
        _ => return Err("Choose a supported hosting service.".into()),
    };
    Ok(pairs
        .into_iter()
        .map(|(id, label)| Target { id, label })
        .collect())
}

#[derive(Clone, Deserialize)]
pub struct PublishRequest {
    pub service: String,
    pub target: String,
    pub name: String,
    pub description: String,
    pub visibility: String,
    pub remote: String,
    pub branch: String,
    pub head: String,
}

#[derive(Serialize)]
pub struct PublishResult {
    pub url: String,
    pub connected: bool,
    pub pushed: bool,
    pub error: Option<String>,
}

pub fn visibility_options(service: &str, target: &str) -> &'static [(&'static str, &'static str)] {
    match service {
        "azure" | "azure-server" => &[("private", "Inherited from the project")],
        "bitbucket-server" => &[("private", "Private")],
        "gitlab-self" => &[("private", "Private"), ("public", "Public"), ("internal", "Internal")],
        "github" | "github-enterprise" if target.starts_with("org:") => {
            &[("private", "Private"), ("public", "Public"), ("internal", "Internal")]
        }
        _ => &[("private", "Private"), ("public", "Public")],
    }
}

pub fn supports_description(service: &str) -> bool {
    !matches!(service, "azure" | "azure-server" | "bitbucket-server")
}

fn create_request(r: &PublishRequest) -> Result<(String, Value), String> {
    if r.name.trim().is_empty()
        || r.name.starts_with('.')
        || !r
            .name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err(
            "Repository names may contain letters, numbers, hyphens, underscores and periods."
                .into(),
        );
    }
    if !visibility_options(&r.service, &r.target).iter().any(|(id, _)| *id == r.visibility) {
        return Err("This destination does not support the selected repository visibility.".into());
    }
    if !r.description.is_empty() && !supports_description(&r.service) {
        return Err("This destination does not support repository descriptions.".into());
    }
    let private = r.visibility == "private";
    Ok(match r.service.as_str() {
        "github" | "github-enterprise" => {
            let endpoint = if let Some(org) = r.target.strip_prefix("org:") {
                if org.is_empty() || !org.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') {
                    return Err("Choose a valid organization.".into());
                }
                format!("/orgs/{}/repos", q(org))
            } else { "/user/repos".into() };
            let mut body = json!({"name":r.name,"description":r.description,"auto_init":false});
            body["private"] = json!(private || r.visibility == "internal");
            if r.visibility == "internal" { body["visibility"] = json!("internal"); }
            (endpoint, body)
        }
        "gitea" | "forgejo" | "codeberg" => {
            let endpoint = r.target.strip_prefix("org:").map(|org| format!("/orgs/{}/repos", q(org))).unwrap_or_else(|| "/user/repos".into());
            let object_format = if r.head.len() == 64 { "sha256" } else { "sha1" };
            (endpoint, json!({"name":r.name,"description":r.description,"private":private,"auto_init":false,"object_format_name":object_format}))
        }
        "gitlab" | "gitlab-self" => {
            let mut body = json!({"name":r.name,"path":r.name,"description":r.description,"visibility":r.visibility,"initialize_with_readme":false});
            if let Some(group) = r.target.strip_prefix("group:") {
                body["namespace_id"] = json!(group.parse::<u64>().ok().filter(|id| *id > 0).ok_or("Choose a valid GitLab group.")?);
            }
            ("/projects".into(), body)
        }
        "bitbucket" => (format!("/repositories/{}/{}",q(&r.target),q(&r.name)),json!({"scm":"git","name":r.name,"description":r.description,"is_private":private})),
        "bitbucket-server" if !r.target.is_empty() => (
            format!("/projects/{}/repos", q(&r.target)),
            json!({"name":r.name,"scmId":"git","public":false}),
        ),
        "azure" | "azure-server" => {
            let version = if r.service == "azure" { "7.1" } else { "7.0" };
            (format!("/{}/_apis/git/repositories?api-version={version}", q(&r.target)),json!({"name":r.name,"project":{"id":r.target}}))
        }
        _ => return Err("Choose a supported hosting service.".into()),
    })
}

fn created_url(service: &str, v: &Value) -> Result<String, String> {
    let url=match service {
        "github" | "github-enterprise" | "gitea" | "forgejo" | "codeberg" => v["clone_url"].as_str(),
        "gitlab" | "gitlab-self" => v["http_url_to_repo"].as_str(),
        "bitbucket" | "bitbucket-server" => v["links"]["clone"].as_array().into_iter().flatten()
            .filter_map(|link| link["href"].as_str()).find(|href| href.starts_with("https://")),
        "azure" | "azure-server" => v["remoteUrl"].as_str(),
        _ => None,
    }.ok_or("The service created the repository but returned no clone URL. Check your account before trying again.")?;
    let mut parsed = url::Url::parse(url).map_err(|_| {
        "The created repository returned an invalid URL. Check your account before trying again."
    })?;
    let authority = url::Url::parse(&format!("https://{}", hosting::host_of(service))).ok();
    if parsed.scheme() != "https" || parsed.password().is_some() || parsed.query().is_some() || parsed.fragment().is_some() ||
        authority.is_none_or(|expected| expected.origin() != parsed.origin()) {
        return Err("The created repository returned an unexpected URL. Check your account before trying again.".into());
    }
    let _ = parsed.set_username("");
    if service == "azure-server" && hosting::azure_server_base().is_none_or(|base| !parsed.as_str().strip_prefix(&base).is_some_and(|path| path.starts_with('/'))) {
        return Err("The created repository belongs to a different Azure collection.".into());
    }
    Ok(parsed.into())
}

fn preflight(dir: &Path, r: &PublishRequest) -> Result<(), String> {
    let remote_ref = format!("refs/remotes/{}/{}", r.remote, r.branch);
    if r.remote.is_empty()
        || r.remote.starts_with('-')
        || r.remote.contains('/')
        || !git::run(dir, &["check-ref-format", &remote_ref]).ok
    {
        return Err("Enter a valid remote name.".into());
    }
    let branch = git::run(dir, &["symbolic-ref", "-q", "HEAD"]);
    let head = git::run(dir, &["rev-parse", "--verify", "HEAD"]);
    if !branch.ok
        || branch.stdout.trim() != format!("refs/heads/{}", r.branch)
        || (if r.head.is_empty() { head.ok } else { !head.ok || head.stdout.trim() != r.head })
    {
        return Err("The checked-out branch or commit changed. Close this panel and review the branch before publishing.".into());
    }
    Ok(())
}

pub fn publish(
    dir: &Path,
    r: &PublishRequest,
    jobs: &crate::jobs::Jobs,
) -> Result<PublishResult, String> {
    publish_with(dir, r, jobs, || {
        let (path, body) = create_request(r)?;
        if !targets(&r.service)?.iter().any(|t| t.id == r.target) {
            return Err(
                "The selected account, workspace or project is no longer available.".into(),
            );
        }
        let response=hosting::http("POST",&format!("{}{path}",hosting::api_base(&r.service)),&hosting::auth(&r.service)?,Some(body))
            .map_err(|error|format!("{error}\nCheck your account for the repository before retrying; a lost response can occur after creation."))?;
        let url = created_url(&r.service, &response)?;
        if !visibility_matches(r, &response) {
            return Err(format!("The service created {url} but did not confirm the requested {} visibility. Verify the repository's access before connecting or pushing.", r.visibility));
        }
        Ok(url)
    })
}

fn visibility_matches(r: &PublishRequest, response: &Value) -> bool {
    let private = r.visibility == "private";
    match r.service.as_str() {
        "azure" | "azure-server" => true,
        "gitlab" | "gitlab-self" => response["visibility"] == r.visibility,
        "github" | "github-enterprise" if r.visibility == "internal" => response["visibility"] == "internal",
        "github" | "github-enterprise" => response["private"] == private
            && response["visibility"].as_str().is_none_or(|visibility| visibility == r.visibility),
        "bitbucket" => response["is_private"] == private,
        "bitbucket-server" => response["public"] == !private,
        _ => response["private"] == private,
    }
}

fn publish_with(
    dir: &Path,
    r: &PublishRequest,
    jobs: &crate::jobs::Jobs,
    create: impl FnOnce() -> Result<String, String>,
) -> Result<PublishResult, String> {
    let lock = crate::operation::lock_for(dir);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    create_request(r)?;
    preflight(dir, r)?;
    let remotes = git::run(dir, &["remote"]);
    if !remotes.ok {
        return Err(remotes.message());
    }
    if remotes.stdout.lines().any(|name| name == r.remote) {
        return Err("That remote already exists. Choose another name or use the URL tab.".into());
    }
    let url = create()?;
    Ok(finish(dir, r, &url, jobs))
}

/// Resume only local connection and push; never repeat the repository creation request.
pub fn resume(
    dir: &Path,
    r: &PublishRequest,
    url: &str,
    jobs: &crate::jobs::Jobs,
) -> PublishResult {
    let lock = crate::operation::lock_for(dir);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    finish(dir, r, url, jobs)
}

fn finish(dir: &Path, r: &PublishRequest, url: &str, jobs: &crate::jobs::Jobs) -> PublishResult {
    let mut result = PublishResult {
        url: url.into(),
        connected: false,
        pushed: false,
        error: None,
    };
    let run = || -> Result<(), String> {
        preflight(dir, r)?;
        let remotes = git::run(dir, &["remote"]);
        if !remotes.ok {
            return Err(remotes.message());
        }
        if remotes.stdout.lines().any(|name| name == r.remote) {
            let current = git::run(dir, &["remote", "get-url", "--push", "--all", &r.remote]);
            let fetch = git::run(dir, &["remote", "get-url", "--all", &r.remote]);
            if !current.ok
                || !fetch.ok
                || current.stdout.trim() != url
                || fetch.stdout.trim() != url
            {
                return Err("The remote URL changed. Review it before pushing.".into());
            }
        } else {
            let out = git::run(dir, &["remote", "add", "--", &r.remote, url]);
            if !out.ok {
                return Err(out.message());
            }
        }
        Ok(())
    };
    if let Err(error) = run() {
        result.error = Some(error);
        return result;
    }
    result.connected = true;
    if r.head.is_empty() {
        return result;
    }
    // Pin the object and destination even if another Git process changes branches during the request.
    let destination = format!("refs/heads/{}", r.branch);
    let spec = format!("{}:{destination}", r.head);
    let out = jobs.run(
        &format!("{}|push", dir.to_string_lossy()),
        dir,
        &["push", "--progress", "--no-follow-tags", "--", url, &spec],
    );
    if !out.ok {
        result.error = Some(out.message());
        return result;
    }
    result.pushed = true;
    for args in [
        vec![
            "update-ref".into(),
            format!("refs/remotes/{}/{}", r.remote, r.branch),
            r.head.clone(),
        ],
        vec![
            "config".into(),
            format!("branch.{}.remote", r.branch),
            r.remote.clone(),
        ],
        vec![
            "config".into(),
            format!("branch.{}.merge", r.branch),
            destination,
        ],
    ] {
        let out = git::run(dir, &args.iter().map(String::as_str).collect::<Vec<_>>());
        if !out.ok {
            result.error = Some(format!(
                "The branch was pushed, but its upstream could not be saved: {}",
                out.message()
            ));
            break;
        }
    }
    result
}

/// Initialize the local repository before creating the remote. Keep the local copy and
/// report the stage that failed if the service cannot be reached or the first push fails.
pub fn initialize(
    engine: &crate::actions::Engine,
    parent: &str,
    options: crate::manage::InitOptions,
    request: PublishRequest,
) -> Result<crate::manage::InitOutcome, String> {
    hosting::auth(&request.service)?;
    initialize_with(engine, parent, options, request, |dir, request| publish(dir, request, &engine.jobs))
}

fn initialize_with(
    engine: &crate::actions::Engine,
    parent: &str,
    options: crate::manage::InitOptions,
    mut request: PublishRequest,
    publish: impl FnOnce(&Path, &PublishRequest) -> Result<PublishResult, String>,
) -> Result<crate::manage::InitOutcome, String> {
    create_request(&request)?;
    let mut local = crate::session::init_repo(engine, parent, &request.name, Some(options))?;
    let dir = Path::new(&local.path);
    let branch = git::run(dir, &["symbolic-ref", "HEAD"]);
    request.branch = branch.stdout.trim().strip_prefix("refs/heads/").unwrap_or_default().to_owned();
    request.head = git::run(dir, &["rev-parse", "--verify", "HEAD"]).stdout.trim().to_owned();
    let warning = match publish(dir, &request) {
        Ok(result) => result.error.map(|error| format!("The hosted repository was created at {}, but setup did not finish. {error}", result.url)),
        Err(error) => Some(format!("The local repository was created. Publishing did not finish. {error}")),
    };
    if let Some(warning) = warning {
        local.warning = Some(match local.warning {
            Some(local) => format!("{warning}\n{local}"),
            None => warning,
        });
    }
    Ok(local)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, PublishRequest) {
        let d = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "Initial",
            ],
        ] {
            assert!(git::run(d.path(), &args).ok);
        }
        let head = git::run(d.path(), &["rev-parse", "HEAD"])
            .stdout
            .trim()
            .to_string();
        (
            d,
            PublishRequest {
                service: "github".into(),
                target: "fixture".into(),
                name: "new-repo".into(),
                description: "A repository".into(),
                visibility: "private".into(),
                remote: "origin".into(),
                branch: "main".into(),
                head,
            },
        )
    }
    #[test]
    fn feature_publishing_encodes_organization_and_group_destinations() {
        let (_, mut request) = fixture();
        request.target = "org:fixture-team".into();
        let (path, body) = create_request(&request).unwrap();
        assert_eq!(path, "/orgs/fixture-team/repos");
        assert_eq!(body["private"], true);
        request.visibility = "internal".into();
        assert_eq!(create_request(&request).unwrap().1["visibility"], "internal");
        request.target = "fixture".into();
        assert!(create_request(&request).is_err());

        request.target = "org:fixture-team".into();
        request.service = "github-enterprise".into();
        let (_, body) = create_request(&request).unwrap();
        assert_eq!(body["visibility"], "internal");
        assert_eq!(body["private"], true);

        request.service = "gitlab-self".into();
        request.target = "group:42".into();
        let (path, body) = create_request(&request).unwrap();
        assert_eq!(path, "/projects");
        assert_eq!(body["namespace_id"], 42);
        assert_eq!(body["visibility"], "internal");

        request.service = "codeberg".into();
        request.visibility = "private".into();
        request.target = "org:fixture-team".into();
        let (path, body) = create_request(&request).unwrap();
        assert_eq!(path, "/orgs/fixture-team/repos");
        assert_eq!(body["private"], true);
    }

    #[test]
    fn creates_empty_repositories_with_explicit_access_for_each_provider() {
        let (_, mut r) = fixture();
        let (path, body) = create_request(&r).unwrap();
        assert_eq!(path, "/user/repos");
        assert_eq!(body["private"], true);
        assert_eq!(body["auto_init"], false);
        r.service = "gitlab".into();
        r.visibility = "public".into();
        let (path, body) = create_request(&r).unwrap();
        assert_eq!(path, "/projects");
        assert_eq!(body["visibility"], "public");
        assert_eq!(body["initialize_with_readme"], false);
        r.service = "bitbucket".into();
        r.target = "workspace a".into();
        let (path, body) = create_request(&r).unwrap();
        assert_eq!(path, "/repositories/workspace%20a/new-repo");
        assert_eq!(body["is_private"], false);
        r.service = "azure".into();
        assert!(create_request(&r).is_err());
        r.visibility = "private".into();
        r.description.clear();
        let (path, body) = create_request(&r).unwrap();
        assert!(path.contains("workspace%20a/_apis/git/repositories"));
        assert_eq!(body["project"]["id"], "workspace a");
        r.service = "bitbucket-server".into();
        r.target = "PROJ".into();
        let (path, body) = create_request(&r).unwrap();
        assert_eq!(path, "/projects/PROJ/repos");
        assert_eq!(body["public"], false);
        assert!(visibility_matches(&r, &json!({"public":false})));
        assert!(!visibility_matches(&r, &json!({"public":true})));
        r.name = "../bad".into();
        assert!(create_request(&r).is_err());
        assert!(created_url(
            "github",
            &json!({"clone_url":"https://unrelated.example/a.git"})
        )
        .is_err());
        assert!(created_url(
            "github",
            &json!({"clone_url":"https://user:secret@github.com/a/b.git"})
        )
        .is_err());
        assert_eq!(
            created_url(
                "gitlab",
                &json!({"http_url_to_repo":"https://gitlab.com/a/b.git"})
            )
            .unwrap(),
            "https://gitlab.com/a/b.git"
        );
        assert_eq!(created_url("bitbucket",&json!({"links":{"clone":[{"name":"ssh","href":"ssh://example"},{"name":"https","href":"https://bitbucket.org/a/b.git"}]}})).unwrap(),"https://bitbucket.org/a/b.git");
    }

    #[test]
    fn initializes_locally_before_publishing_and_preserves_local_copy_on_failure() {
        let (_, request) = fixture();
        let parent = tempfile::tempdir().unwrap();
        let parent_path = parent.path().to_str().unwrap();
        let engine = crate::actions::Engine::default();
        let options = || crate::manage::InitOptions { branch: "custom".into(), ..Default::default() };
        let name = request.name.clone();
        let outcome = initialize_with(&engine, parent_path, options(), request.clone(), |dir, request| {
            assert_eq!(dir, parent.path().join(&name));
            assert_eq!(request.branch, "custom");
            assert!(preflight(dir, request).is_ok());
            Err("The account cannot create repositories.".into())
        }).unwrap();
        assert!(Path::new(&outcome.path).join(".git").is_dir());
        assert!(outcome.warning.unwrap().contains("The local repository was created"));
        assert!(initialize_with(&engine, parent_path, options(), request, |_, _| {
            panic!("must check the local destination before publishing")
        }).is_err());
    }
    #[test]
    fn local_preflight_refuses_stale_branch_and_existing_remote_before_creation() {
        let (d, mut r) = fixture();
        let jobs = crate::jobs::Jobs::default();
        r.head = "0".repeat(40);
        assert!(publish_with(d.path(), &r, &jobs, || panic!("must not create")).is_err());
        r.head = git::run(d.path(), &["rev-parse", "HEAD"])
            .stdout
            .trim()
            .into();
        assert!(
            git::run(
                d.path(),
                &["remote", "add", "origin", "https://fixture.example/a.git"]
            )
            .ok
        );
        assert!(publish_with(d.path(), &r, &jobs, || panic!("must not create")).is_err());
        assert_eq!(
            git::run(d.path(), &["remote", "get-url", "origin"])
                .stdout
                .trim(),
            "https://fixture.example/a.git"
        );
    }
    #[test]
    fn publishes_exact_branch_and_saves_upstream_without_pushing_tags() {
        let (d, r) = fixture();
        let bare = tempfile::tempdir().unwrap();
        assert!(git::run(bare.path(), &["init", "--bare", "-q"]).ok);
        assert!(git::run(d.path(), &["tag", "main"]).ok);
        assert!(
            git::run(
                d.path(),
                &[
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "tag",
                    "-a",
                    "release",
                    "-m",
                    "Release"
                ]
            )
            .ok
        );
        assert!(git::run(d.path(), &["config", "push.followTags", "true"]).ok);
        let result = publish_with(d.path(), &r, &crate::jobs::Jobs::default(), || {
            Ok(bare.path().to_string_lossy().into())
        })
        .unwrap();
        assert!(result.connected && result.pushed, "{:?}", result.error);
        assert!(result.error.is_none());
        assert_eq!(
            git::run(bare.path(), &["rev-parse", "refs/heads/main"])
                .stdout
                .trim(),
            r.head
        );
        assert!(!git::run(bare.path(), &["show-ref", "--verify", "refs/tags/main"]).ok);
        assert!(!git::run(bare.path(), &["show-ref", "--verify", "refs/tags/release"]).ok);
        assert_eq!(
            git::run(d.path(), &["rev-parse", "--abbrev-ref", "@{u}"])
                .stdout
                .trim(),
            "origin/main"
        );
    }
    #[test]
    fn failed_push_keeps_remote_and_resumes_without_creating_again() {
        let (d, r) = fixture();
        let parent = tempfile::tempdir().unwrap();
        let bare = parent.path().join("new.git");
        let url = bare.to_string_lossy().to_string();
        let jobs = crate::jobs::Jobs::default();
        let first = publish_with(d.path(), &r, &jobs, || Ok(url.clone())).unwrap();
        assert!(first.connected && !first.pushed && first.error.is_some());
        assert!(git::run(parent.path(), &["init", "--bare", "-q", "new.git"]).ok);
        let result = resume(d.path(), &r, &url, &jobs);
        assert!(result.pushed && result.error.is_none());
        assert!(
            git::run(
                d.path(),
                &[
                    "remote",
                    "set-url",
                    "origin",
                    "https://fixture.example/other.git"
                ]
            )
            .ok
        );
        let refused = resume(d.path(), &r, &url, &jobs);
        assert!(!refused.pushed);
        assert!(refused.error.unwrap().contains("URL changed"));
    }
    #[test]
    fn creation_failure_leaves_local_configuration_untouched() {
        let (d, r) = fixture();
        let jobs = crate::jobs::Jobs::default();
        assert!(
            publish_with(d.path(), &r, &jobs, || Err("403 missing permission".into())).is_err()
        );
        assert!(git::run(d.path(), &["remote"]).stdout.trim().is_empty());
        let partial = publish_with(d.path(), &r, &jobs, || {
            assert!(git::run(d.path(), &["checkout", "-qb", "other"]).ok);
            Ok("https://github.com/fixture/new-repo.git".into())
        })
        .unwrap();
        assert!(!partial.connected && !partial.pushed);
        assert!(partial.error.unwrap().contains("branch or commit changed"));
        assert!(git::run(d.path(), &["remote"]).stdout.trim().is_empty());
    }
}
