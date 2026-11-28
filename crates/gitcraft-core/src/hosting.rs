//! Integrations (Preferences > Integrations): GitHub, GitHub Enterprise Server, GitLab,
//! GitLab Self-Managed, Bitbucket, Bitbucket Data Center, Azure DevOps (hosting services) and Jira Cloud,
//! Jira Data Center, Trello (issue trackers). This file: connections, sign-in, tokens for Git, remotes.
//! hosting_api.rs: pull requests, issues, Launchpad, repositories to clone, SSH keys.
//! Tokens live in the operating system's credential store, never in GitCraft's settings file; hosts, user
//! names and organisations are ordinary settings passed in with `set_config`.

pub use crate::hosting_api::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::{Mutex, OnceLock, RwLock}, time::{Duration, Instant}};

const SERVICE: &str = "GitCraft";

type SecretCache = HashMap<String, (Instant, Option<String>)>;
fn secret_cache() -> &'static Mutex<SecretCache> {
    static CACHE: OnceLock<Mutex<SecretCache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn keychain_error(error: keyring::Error) -> String {
    let advice = if cfg!(target_os = "macos") {
        "Unlock your login keychain and allow GitCraft to access it."
    } else if cfg!(windows) {
        "Check access to Windows Credential Manager and sign in to your Windows session."
    } else {
        "Start and unlock a Secret Service keyring in your desktop session."
    };
    format!("GitCraft could not access the system credential store. {advice} ({error})")
}

fn secret_lookup(name: &str) -> Result<Option<String>, String> {
    let mut cache = secret_cache().lock().unwrap_or_else(|error| error.into_inner());
    if let Some((_, secret)) = cache.get(name).filter(|(expires, _)| *expires > Instant::now()) {
        return Ok(secret.clone());
    }
    let entry = keyring::Entry::new(SERVICE, name).map_err(keychain_error)?;
    let secret = match entry.get_password() {
        Ok(secret) if !secret.is_empty() => Some(secret),
        Ok(_) | Err(keyring::Error::NoEntry) => None,
        Err(error) => return Err(keychain_error(error)),
    };
    let lifetime = Duration::from_secs(if secret.is_some() { 300 } else { 30 });
    cache.insert(name.to_owned(), (Instant::now() + lifetime, secret.clone()));
    Ok(secret)
}

pub fn secret_get(name: &str) -> Option<String> {
    secret_lookup(name).ok().flatten()
}

pub fn has_connection(id: &str) -> Result<bool, String> {
    secret_lookup(&secret_name(id)).map(|token| token.is_some())
}

pub fn secret_set(name: &str, value: &str) -> Result<(), String> {
    let mut cache = secret_cache().lock().unwrap_or_else(|error| error.into_inner());
    let entry = keyring::Entry::new(SERVICE, name).map_err(keychain_error)?;
    let result = if value.is_empty() {
        match entry.delete_credential() { Ok(()) | Err(keyring::Error::NoEntry) => Ok(()), Err(error) => Err(error) }
    } else {
        entry.set_password(value)
    };
    cache.remove(name);
    result.map_err(keychain_error)
}

static MANAGED_SIGN_IN: Mutex<()> = Mutex::new(());

fn managed_name(id: &str) -> String { format!("{id}-managed-sign-in") }

fn managed_supported(id: &str) -> bool { matches!(id, "github" | "gitlab" | "bitbucket" | "azure") }

pub fn save_secret_manually(name: &str, value: &str) -> Result<(), String> {
    let _guard = crate::read_jobs::lock(&MANAGED_SIGN_IN)?;
    if let Some(id) = name.strip_suffix("-token").filter(|id| managed_supported(id)) {
        secret_set(&managed_name(id), "")?;
    }
    secret_set(name, value)
}

pub fn save_sign_in(id: &str, token: &str, managed: bool) -> Result<(), String> {
    if !SERVICES.contains(&id) { return Err("Unknown integration".into()); }
    let _guard = crate::read_jobs::lock(&MANAGED_SIGN_IN)?;
    if managed_supported(id) { secret_set(&managed_name(id), "")?; }
    secret_set(&secret_name(id), token)?;
    if managed && managed_supported(id) { secret_set(&managed_name(id), "gcm")?; }
    Ok(())
}

/// The supported integrations, in display order.
pub const SERVICES: [&str; 14] = ["github", "github-enterprise", "gitlab", "gitlab-self", "bitbucket", "bitbucket-server", "azure", "azure-server", "gitea", "forgejo", "codeberg", "jira", "jira-server", "trello"];

pub fn label(id: &str) -> &'static str {
    match id {
        "github" => "GitHub",
        "github-enterprise" => "GitHub Enterprise Server",
        "gitlab" => "GitLab",
        "gitlab-self" => "GitLab Self-Managed",
        "bitbucket" => "Bitbucket",
        "bitbucket-server" => "Bitbucket Data Center",
        "azure" => "Azure DevOps",
        "azure-server" => "Azure DevOps Server",
        "gitea" => "Gitea",
        "forgejo" => "Forgejo",
        "codeberg" => "Codeberg",
        "jira" => "Jira Cloud",
        "jira-server" => "Jira Data Center",
        "trello" => "Trello",
        _ => "this service",
    }
}

/// Name of a service's token in the credential store ("github-token" and "gitlab-token" as before).
pub fn secret_name(id: &str) -> String {
    format!("{id}-token")
}

/// Hosts, user names and organisations for the services (not secret).
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub github_enterprise_host: String,
    pub gitlab_host: String,
    /// Bitbucket user name for an app password / API token (empty: the token is an OAuth / access token).
    pub bitbucket_user: String,
    pub bitbucket_server_host: String,
    pub bitbucket_server_user: String,
    pub azure_org: String,
    pub azure_server_collection: String,
    pub gitea_host: String,
    pub forgejo_host: String,
    pub gitea_user: String,
    pub forgejo_user: String,
    pub codeberg_user: String,
    /// Jira Cloud site, e.g. mycompany.atlassian.net, and the account e-mail its API token belongs to.
    pub jira_site: String,
    pub jira_email: String,
    pub jira_server_host: String,
    /// Trello API key (the token is the secret).
    pub trello_key: String,
}

static CONFIG: RwLock<Option<Config>> = RwLock::new(None);

pub fn set_config(c: Config) {
    if let Ok(mut w) = CONFIG.write() {
        *w = Some(c);
    }
}

pub fn config() -> Config {
    CONFIG.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}

fn clean_host(h: &str) -> String {
    h.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string()
}

fn configured_https(value: &str) -> Option<url::Url> {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() { return None; }
    let url = if value.contains("://") { value.to_owned() } else { format!("https://{value}") };
    url::Url::parse(&url).ok().filter(|url| url.scheme() == "https" && url.host_str().is_some() &&
        url.username().is_empty() && url.password().is_none() && url.query().is_none() && url.fragment().is_none())
}

fn url_authority(url: &url::Url) -> &str {
    url.as_str().strip_prefix("https://").unwrap_or_default().split('/').next().unwrap_or_default()
}

pub fn azure_server_base() -> Option<String> {
    let url = configured_https(&config().azure_server_collection)?;
    if url.path() == "/" { return None; }
    Some(url.as_str().trim_end_matches('/').to_owned())
}

/// The web host of a service ("" when it isn't set up).
pub fn host_of(id: &str) -> String {
    let c = config();
    match id {
        "github" => "github.com".into(),
        "github-enterprise" => clean_host(&c.github_enterprise_host),
        "gitlab" => "gitlab.com".into(),
        "gitlab-self" => clean_host(&c.gitlab_host),
        "bitbucket" => "bitbucket.org".into(),
        "bitbucket-server" => clean_host(&c.bitbucket_server_host),
        "azure" => "dev.azure.com".into(),
        "azure-server" => configured_https(&c.azure_server_collection).filter(|url| url.path() != "/").map(|url| url_authority(&url).to_owned()).unwrap_or_default(),
        "gitea" | "forgejo" => configured_https(if id == "gitea" { &c.gitea_host } else { &c.forgejo_host }).filter(|url| url.path() == "/").map(|url| url_authority(&url).to_owned()).unwrap_or_default(),
        "codeberg" => "codeberg.org".into(),
        "jira" => clean_host(&c.jira_site),
        "jira-server" => clean_host(&c.jira_server_host),
        "trello" => "trello.com".into(),
        _ => String::new(),
    }
}

pub fn api_base(id: &str) -> String {
    let h = host_of(id);
    match id {
        "github" => "https://api.github.com".into(),
        "github-enterprise" => format!("https://{h}/api/v3"),
        "gitlab" | "gitlab-self" => format!("https://{h}/api/v4"),
        "bitbucket" => "https://api.bitbucket.org/2.0".into(),
        "bitbucket-server" => format!("https://{h}/rest/api/1.0"),
        "azure" => format!("https://dev.azure.com/{}", config().azure_org.trim()),
        "azure-server" => azure_server_base().unwrap_or_default(),
        "gitea" | "forgejo" | "codeberg" => format!("https://{h}/api/v1"),
        "jira" => format!("https://{h}/rest/api/3"),
        "jira-server" => format!("https://{h}/rest/api/2"),
        "trello" => "https://api.trello.com/1".into(),
        _ => String::new(),
    }
}

pub enum Auth {
    Bearer(String),
    Header(&'static str, String),
    Basic(String, String),
    Trello(String, String),
    Managed(String, String),
}

pub fn auth(id: &str) -> Result<Auth, String> {
    let token = secret_lookup(&secret_name(id))?.ok_or_else(|| format!("Connect {} first (Preferences > Integrations).", label(id)))?;
    if managed_supported(id) && secret_lookup(&managed_name(id))?.as_deref() == Some("gcm") {
        return Ok(Auth::Managed(id.to_owned(), token));
    }
    let c = config();
    Ok(match id {
        "github" | "github-enterprise" | "bitbucket-server" | "jira-server" => Auth::Bearer(token),
        "gitea" | "forgejo" | "codeberg" => Auth::Header("Authorization", format!("token {token}")),
        "gitlab" | "gitlab-self" => Auth::Header("PRIVATE-TOKEN", token),
        "bitbucket" if c.bitbucket_user.trim().is_empty() => Auth::Bearer(token),
        "bitbucket" => Auth::Basic(c.bitbucket_user.trim().to_string(), token),
        // A personal access token, or (after a browser sign-in through Git Credential Manager) an OAuth JWT.
        "azure" if token.starts_with("eyJ") => Auth::Bearer(token),
        "azure" | "azure-server" => Auth::Basic(String::new(), token),
        "jira" => Auth::Basic(c.jira_email.trim().to_string(), token),
        "trello" => Auth::Trello(c.trello_key.trim().to_string(), token),
        _ => return Err("Unknown integration".into()),
    })
}

fn b64(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

/// One HTTPS request to a service's REST API; JSON in and out.
pub struct HttpPage { pub value: Value, pub link: Option<String>, pub next_page: Option<String>, pub continuation: Option<String> }

pub fn http(method: &str, url: &str, auth: &Auth, body: Option<Value>) -> Result<Value, String> {
    http_page(method, url, auth, body).map(|page| page.value)
}

pub fn http_page(method: &str, url: &str, auth: &Auth, body: Option<Value>) -> Result<HttpPage, String> {
    let result = if let Auth::Managed(id, token) = auth {
        let can_retry = matches!(method, "GET" | "HEAD");
        let replay = if can_retry { body.clone() } else { None };
        let result = http_page_request(method, url, auth, body);
        if result.as_ref().is_err_and(|error| error.starts_with("401:")) {
            match refresh_managed_sign_in(id, token) {
                Ok(Some(refreshed)) => {
                    if !can_retry {
                        return Err("The integration sign-in was refreshed. Try the action again.".into());
                    }
                    let updated = Auth::Managed(id.clone(), refreshed);
                    return redact_http_error(http_page_request(method, url, &updated, replay), &updated);
                }
                Ok(None) => result,
                Err(error) => return Err(error),
            }
        } else { result }
    } else { http_page_request(method, url, auth, body) };
    redact_http_error(result, auth)
}

fn redact_http_error(result: Result<HttpPage, String>, auth: &Auth) -> Result<HttpPage, String> {
    result.map_err(|mut error| {
        let secrets = match auth {
            Auth::Managed(id, token) if id == "azure" && !token.starts_with("eyJ") => vec![token.clone(), b64(&format!(":{token}"))],
            Auth::Header("Authorization", value) if value.starts_with("token ") => vec![value[6..].to_owned(), value.clone()],
            Auth::Bearer(value) | Auth::Header(_, value) | Auth::Managed(_, value) => vec![value.clone()],
            Auth::Basic(user, password) => vec![password.clone(), b64(&format!("{user}:{password}"))],
            Auth::Trello(key, token) => vec![key.clone(), token.clone()],
        };
        for secret in secrets.into_iter().filter(|s| !s.is_empty()) {
            error = error.replace(&crate::hosting_api::q(&secret), "[redacted]").replace(&secret, "[redacted]");
        }
        error
    })
}

fn http_page_request(method: &str, url: &str, auth: &Auth, body: Option<Value>) -> Result<HttpPage, String> {
    crate::read_jobs::check()?;
    let context = crate::read_jobs::context();
    let origin = reqwest::Url::parse(url).map_err(|_| "Invalid service URL")?.origin().ascii_serialization();
    static BACKOFF: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, i64>>> = std::sync::OnceLock::new();
    static SLOTS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let backoff = BACKOFF.get_or_init(Default::default);
    let agent = http_client();
    let mut request = agent.request(method.parse().map_err(|_| "Invalid HTTP method")?, url)
        .header("User-Agent", "GitCraft").timeout(std::time::Duration::from_secs(30));
    request = match auth {
        Auth::Managed(id, token) if id == "azure" && !token.starts_with("eyJ") => request.basic_auth("", Some(token)),
        Auth::Bearer(token) | Auth::Managed(_, token) => request.bearer_auth(token),
        Auth::Header(name, token) => request.header(*name, token),
        Auth::Basic(user, password) => request.basic_auth(user, Some(password)),
        Auth::Trello(key, token) => {
            let key = crate::hosting_api::q(key);
            let token = crate::hosting_api::q(token);
            request.header("Authorization", format!("OAuth oauth_consumer_key=\"{key}\", oauth_token=\"{token}\""))
        },
    };
    request = if url.contains("api.github.com") || url.contains("/api/v3/") {
        request.header("Accept", "application/vnd.github+json").header("X-GitHub-Api-Version", "2022-11-28")
    } else { request.header("Accept", "application/json") };
    if let Some(body) = body { request = request.json(&body); }
    // Dropping the request interrupts connect, header, body and DNS waits.
    crate::runtime().block_on(async {
        let operation = async {
            let _slot = SLOTS.get_or_init(|| tokio::sync::Semaphore::new(8)).acquire().await.map_err(|_| "HTTP scheduler closed".to_string())?;
            let now = chrono::Utc::now().timestamp();
            {
                let mut limits = backoff.lock().unwrap_or_else(|error| error.into_inner());
                limits.retain(|_, until| *until > now);
                if let Some(until) = limits.get(&origin) { return Err(rate_limit_message(*until)); }
            }
            let mut response = request.send().await.map_err(transport_error)?;
            let status = response.status().as_u16();
            if (300..400).contains(&status) { return Err("The hosting service redirected this authenticated request. Update its configured address; credentials were not forwarded.".into()); }
            let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
            let link = header("Link"); let next_page = header("X-Next-Page"); let continuation = header("X-MS-ContinuationToken");
            let retry_after = header("Retry-After");
            let quota_exhausted = header("X-RateLimit-Remaining").as_deref() == Some("0");
            let quota_reset = header("X-RateLimit-Reset").and_then(|value| value.parse::<i64>().ok());
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| "Could not read the hosting response.".to_string())? {
                if bytes.len() + chunk.len() > 10 * 1024 * 1024 { return Err("The hosting response exceeded its 10 MB budget; narrow the query.".into()); }
                bytes.extend_from_slice(&chunk);
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let value = if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::String(text.clone())) };
            if status >= 400 {
                let message = value["message"].as_str().or(value["error"]["message"].as_str()).or(value["errorMessages"][0].as_str()).or(value["errors"][0]["message"].as_str()).unwrap_or(&text);
                if status == 429 || status == 403 && (retry_after.is_some() || quota_exhausted || message.to_ascii_lowercase().contains("rate limit")) {
                    let now = chrono::Utc::now().timestamp();
                    let retry = retry_after.as_deref().and_then(|value| {
                        value.parse::<i64>().ok().map(|seconds| now.saturating_add(seconds))
                            .or_else(|| chrono::DateTime::parse_from_rfc2822(value).ok().map(|date| date.timestamp()))
                    });
                    let until = retry.or(quota_reset.filter(|_| quota_exhausted)).unwrap_or(now + 60).clamp(now + 1, now + 86_400);
                    backoff.lock().unwrap_or_else(|error| error.into_inner()).insert(origin, until);
                    return Err(rate_limit_message(until));
                }
                let hint = match status { 401 => " - the token was not accepted; reconnect in Preferences > Integrations", 403 => " - the token may be missing a permission", _ => "" };
                return Err(format!("{status}: {}{hint}", message.chars().take(300).collect::<String>()));
            }
            Ok(HttpPage { value, link, next_page, continuation })
        };
        crate::read_jobs::wait_http(operation, context, std::time::Duration::from_secs(30)).await
    })
}

// Keep native DNS (including system VPN and split-DNS settings) off the async
// runtime. A cancelled request releases its waiter, but the slot stays occupied
// until the OS lookup finishes, so stalled lookups cannot accumulate threads.
struct SystemDnsResolver(std::sync::Arc<tokio::sync::Semaphore>);

impl SystemDnsResolver {
    async fn lookup(
        slots: std::sync::Arc<tokio::sync::Semaphore>,
        lookup: impl FnOnce() -> std::io::Result<reqwest::dns::Addrs> + Send + 'static,
    ) -> Result<reqwest::dns::Addrs, Box<dyn std::error::Error + Send + Sync>> {
        let permit = slots.acquire_owned().await?;
        let (complete, result) = tokio::sync::oneshot::channel();
        std::thread::Builder::new().name("gitcraft-dns".into()).spawn(move || {
            let _permit = permit;
            let _ = complete.send(lookup());
        })?;
        Ok(result.await??)
    }
}

impl reqwest::dns::Resolve for SystemDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        use std::net::ToSocketAddrs;
        let hostname = name.as_str().to_owned();
        Box::pin(Self::lookup(self.0.clone(), move || {
            Ok(Box::new((hostname.as_str(), 0).to_socket_addrs()?))
        }))
    }
}

pub(crate) fn http_client() -> &'static reqwest::Client {
    static TRANSPORT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    TRANSPORT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .dns_resolver(SystemDnsResolver(std::sync::Arc::new(tokio::sync::Semaphore::new(8))))
            .build().expect("HTTP transport")
    })
}

pub(crate) fn transport_error(error: reqwest::Error) -> String {
    use std::error::Error;
    if error.is_timeout() { return "The request timed out.".into(); }
    if error.is_builder() { return "The request contains an invalid address or header.".into(); }
    let mut cause = error.source();
    while let Some(current) = cause {
        let text = current.to_string().to_ascii_lowercase();
        if text.contains("certificate") || text.contains("tls") {
            return "TLS verification failed. Check the server certificate and the system trust store.".into();
        }
        if text.contains("dns") || text.contains("hostname") {
            return "DNS lookup failed. Check the server hostname.".into();
        }
        if text.contains("proxy") || text.contains("tunnel") {
            return "The proxy connection failed. Check the system proxy settings.".into();
        }
        cause = current.source();
    }
    if error.is_connect() { "The server connection failed. Check the address and connection.".into() }
    else { "The server response could not be read.".into() }
}

fn rate_limit_message(until: i64) -> String {
    let time = chrono::DateTime::from_timestamp(until, 0).expect("bounded timestamp").with_timezone(&chrono::Local);
    format!("The service is rate limited until {}. Try again then.", time.format("%H:%M:%S %Z"))
}

fn s(v: &Value) -> String {
    v.as_str().map(|x| x.to_string()).unwrap_or_default()
}

fn no_window(c: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let _ = c;
}

#[derive(Serialize, Default, Clone)]
pub struct ServiceStatus {
    pub id: String,
    pub label: String,
    pub host: String,
    pub connected: bool,
    pub account: Option<String>,
    pub error: Option<String>,
}

/// Who the saved token belongs to ("Connected as ...").
pub fn whoami_id(id: &str) -> Result<String, String> {
    let get = |path: &str| http("GET", &format!("{}{path}", api_base(id)), &auth(id)?, None);
    let v = match id {
        "github" | "github-enterprise" | "gitea" | "forgejo" | "codeberg" => return get("/user").map(|v| s(&v["login"])),
        "gitlab" | "gitlab-self" => return get("/user").map(|v| s(&v["username"])),
        "bitbucket" => get("/user")?,
        "bitbucket-server" => {
            get("/application-properties")?;
            let u = config().bitbucket_server_user;
            return Ok(if u.trim().is_empty() { "token accepted".into() } else { u });
        }
        "azure" => http("GET", "https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1", &auth(id)?, None)?,
        "azure-server" => get("/_apis/connectionData")?["authenticatedUser"].clone(),
        "jira" | "jira-server" => get("/myself")?,
        "trello" => get("/members/me")?,
        _ => return Err("Unknown integration".into()),
    };
    let name = [&v["display_name"], &v["displayName"], &v["fullName"], &v["username"], &v["emailAddress"], &v["name"]].into_iter().map(s).find(|x| !x.is_empty());
    Ok(name.unwrap_or_else(|| "connected".into()))
}

pub fn status(id: &str) -> ServiceStatus {
    let mut st = ServiceStatus { id: id.into(), label: label(id).into(), host: host_of(id), ..Default::default() };
    match secret_lookup(&secret_name(id)) {
        Ok(Some(_)) => {}
        Ok(None) => return st,
        Err(error) => { st.error = Some(error); return st; }
    }
    st.connected = true;
    match whoami_id(id) {
        Ok(a) => st.account = Some(a),
        Err(e) => st.error = Some(e),
    }
    st
}

fn gcm(host: &str, operation: &str, input: String, interactive: bool) -> crate::git::GitOut {
    let mut command = std::process::Command::new(crate::prefs::git_program());
    crate::git::english_diagnostics(&mut command);
    command.args(["-c", &format!("credential.https://{host}.helper="), "-c", &format!("credential.https://{host}.helper=manager"), "credential", operation]);
    command.env_remove("GIT_CONFIG_COUNT");
    if !interactive { command.env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never"); }
    no_window(&mut command);
    let timeout = Duration::from_secs(if interactive { 180 } else { 15 });
    crate::read_jobs::run_bounded(command, Some(input.into_bytes()), timeout, "Git Credential Manager timed out.")
}

fn credential_field<'a>(output: &'a str, field: &str) -> &'a str {
    output.lines().find_map(|line| line.split_once('=').filter(|(key, _)| *key == field).map(|(_, value)| value)).unwrap_or("")
}

fn gcm_host(id: &str) -> Result<String, String> {
    if managed_supported(id) { Ok(host_of(id)) }
    else { Err(format!("{} does not support browser sign-in. Use a personal access token.", label(id))) }
}

/// Browser sign-in through Git Credential Manager, which keeps its own refresh credential.
pub fn browser_sign_in(id: &str) -> Result<String, String> {
    let host = gcm_host(id)?;
    let output = gcm(&host, "fill", format!("protocol=https\nhost={host}\n\n"), true);
    let password = credential_field(&output.stdout, "password");
    if !output.ok || password.is_empty() {
        let error = output.stderr.trim();
        return Err(if error.contains("not a git command") || error.contains("credential-manager' is not") {
            "Git Credential Manager is unavailable. Install it for your platform or paste an access token.".into()
        } else if error.is_empty() { "The sign-in was cancelled.".into() }
        else { crate::git::redact_diagnostic(error) });
    }
    let username = credential_field(&output.stdout, "username");
    let _ = gcm(&host, "approve", format!("protocol=https\nhost={host}\nusername={username}\npassword={password}\n\n"), false);
    Ok(password.to_owned())
}

fn refresh_managed_sign_in(id: &str, expired: &str) -> Result<Option<String>, String> {
    crate::read_jobs::check()?;
    let _guard = crate::read_jobs::lock(&MANAGED_SIGN_IN)?;
    if secret_lookup(&managed_name(id))?.as_deref() != Some("gcm") { return Ok(None); }
    let key = secret_name(id);
    if let Some(current) = secret_lookup(&key)?.filter(|current| current != expired) { return Ok(Some(current)); }
    let host = gcm_host(id)?;
    let output = gcm(&host, "fill", format!("protocol=https\nhost={host}\n\n"), false);
    let token = credential_field(&output.stdout, "password");
    if !output.ok || token.is_empty() || token == expired { return Ok(None); }
    if secret_lookup(&managed_name(id))?.as_deref() != Some("gcm") { return Ok(None); }
    secret_set(&key, token)?;
    sync_git_credentials("");
    Ok(Some(token.to_owned()))
}

/// The GitHub CLI's sign-in (`gh auth token`), for people already logged in with `gh`.
pub fn gh_cli_token() -> Result<String, String> {
    let mut c = std::process::Command::new("gh");
    c.args(["auth", "token"]);
    no_window(&mut c);
    let o = c.output().map_err(|_| "The GitHub CLI (gh) isn't installed.".to_string())?;
    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if o.status.success() && !t.is_empty() {
        Ok(t)
    } else {
        Err("The GitHub CLI isn't signed in (run `gh auth login`).".into())
    }
}

/// Give Git the tokens of the connected hosting services, for HTTPS remotes.
pub fn sync_git_credentials(gitlab_host: &str) {
    if !gitlab_host.trim().is_empty() {
        let mut c = config();
        if c.gitlab_host.trim().is_empty() {
            c.gitlab_host = gitlab_host.trim().to_string();
            set_config(c);
        }
    }
    let c = config();
    let mut t = Vec::new();
    for id in ["github", "github-enterprise", "gitlab", "gitlab-self", "bitbucket", "bitbucket-server", "azure", "azure-server", "gitea", "forgejo", "codeberg"] {
        let host = host_of(id);
        let Some(tok) = secret_get(&secret_name(id)) else { continue };
        if host.is_empty() {
            continue;
        }
        let user = match id {
            "github" | "github-enterprise" => "x-access-token".to_string(),
            "gitlab" | "gitlab-self" => "oauth2".to_string(),
            "bitbucket" if c.bitbucket_user.trim().is_empty() => "x-token-auth".to_string(),
            "bitbucket" if c.bitbucket_user.contains('@') => "x-bitbucket-api-token-auth".to_string(),
            "bitbucket" => c.bitbucket_user.trim().to_string(),
            "bitbucket-server" => c.bitbucket_server_user.trim().to_string(),
            "gitea" => c.gitea_user.trim().to_owned(),
            "forgejo" => c.forgejo_user.trim().to_owned(),
            "codeberg" => c.codeberg_user.trim().to_owned(),
            _ => "gitcraft".to_string(),
        };
        let credential_scope = if id == "azure-server" { azure_server_base().and_then(|base| base.strip_prefix("https://").map(str::to_owned)).unwrap_or_default() } else { host };
        if !credential_scope.is_empty() && (!matches!(id, "gitea" | "forgejo" | "codeberg") || !user.is_empty()) {
            t.push((credential_scope, user, tok));
        }
    }
    crate::git::set_host_tokens(t);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Gitlab,
    Bitbucket,
    Azure,
    Gitea,
}

/// Where a remote lives: provider, the integration it belongs to, host and project path.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RemoteRepo {
    pub provider: Provider,
    /// Integration id (github, github-enterprise, gitlab, gitlab-self, bitbucket, bitbucket-server, azure).
    pub service: String,
    pub host: String,
    /// "owner/repo", "group/sub/project", Bitbucket Data Center "PROJECT/repo", Azure "org/project/repo".
    pub path: String,
    pub web: String,
}

/// Understands https://host/owner/repo(.git), git@host:owner/repo(.git), ssh://git@host[:port]/owner/repo, and the
/// Azure DevOps and Bitbucket Data Center address styles.
pub fn parse_remote(url: &str, gitlab_host: &str) -> Option<RemoteRepo> {
    let u = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let (mut host, path) = if let Some(rest) = u.strip_prefix("git@") {
        let (h, p) = rest.split_once(':')?;
        (h.to_string(), p.to_string())
    } else {
        let rest = u.split_once("://").map(|(_, r)| r).unwrap_or(u);
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (h, p) = rest.split_once('/')?;
        (if u.starts_with("ssh://") { h.split(':').next().unwrap_or(h).to_string() } else { h.to_string() }, p.to_string())
    };
    let c = config();
    let gl_self = if gitlab_host.trim().is_empty() { clean_host(&c.gitlab_host) } else { clean_host(gitlab_host) };
    let mk = |provider: Provider, service: &str, host: &str, path: String, web: String| Some(RemoteRepo { provider, service: service.into(), host: host.into(), path, web });
    // Azure DevOps: dev.azure.com/{org}/{project}/_git/{repo}, ssh.dev.azure.com:v3/{org}/{project}/{repo},
    // {org}.visualstudio.com/{project}/_git/{repo}.
    if host == "dev.azure.com" || host == "ssh.dev.azure.com" || host.ends_with(".visualstudio.com") {
        let parts: Vec<&str> = path.trim_start_matches("v3/").split('/').filter(|p| *p != "_git").collect();
        let (org, project, repo) = if host.ends_with(".visualstudio.com") {
            (host.trim_end_matches(".visualstudio.com").to_string(), parts.first()?.to_string(), parts.get(1)?.to_string())
        } else {
            (parts.first()?.to_string(), parts.get(1)?.to_string(), parts.get(2)?.to_string())
        };
        let web = format!("https://dev.azure.com/{org}/{project}/_git/{repo}");
        return mk(Provider::Azure, "azure", "dev.azure.com", format!("{org}/{project}/{repo}"), web);
    }
    if u.starts_with("git@") || u.starts_with("ssh://") {
        let hostname = |authority: &str| authority.split('/').next().unwrap_or(authority).split(':').next().unwrap_or(authority).to_ascii_lowercase();
        for authority in [&gl_self, &clean_host(&c.github_enterprise_host), &clean_host(&c.bitbucket_server_host)] {
            if !authority.is_empty() && hostname(authority) == hostname(&host) { host = authority.split('/').next().unwrap_or(authority).to_owned(); break; }
        }
    }
    if let Some(base) = azure_server_base() {
        let configured = url::Url::parse(&base).ok()?;
        let prefix = configured.path().trim_matches('/');
        if url.starts_with("https://") && host.eq_ignore_ascii_case(url_authority(&configured)) {
            if let Some((project, repo)) = path.strip_prefix(prefix).and_then(|path| path.strip_prefix('/')).and_then(|path| path.split_once("/_git/")) {
                if !project.is_empty() && !repo.is_empty() && !project.contains('/') && !repo.contains('/') {
                    let web = format!("{base}/{project}/_git/{repo}");
                    return mk(Provider::Azure, "azure-server", &host, format!("{prefix}/{project}/{repo}"), web);
                }
            }
        }
    }
    let simple = |provider: Provider, service: &str| mk(provider, service, &host, path.clone(), format!("https://{host}/{path}"));
    match host.as_str() {
        "github.com" => return simple(Provider::Github, "github"),
        "gitlab.com" => return simple(Provider::Gitlab, "gitlab"),
        "bitbucket.org" => return simple(Provider::Bitbucket, "bitbucket"),
        "codeberg.org" => return path.split_once('/').filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
            .and_then(|_| simple(Provider::Gitea, "codeberg")),
        _ => {}
    }
    if !gl_self.is_empty() && host == gl_self {
        return simple(Provider::Gitlab, "gitlab-self");
    }
    for (service, configured) in [("gitea", &c.gitea_host), ("forgejo", &c.forgejo_host)] {
        if let Some(domain) = configured_https(configured).filter(|domain| domain.path() == "/") {
            let authority = url_authority(&domain);
            if (host.eq_ignore_ascii_case(authority) || ((u.starts_with("ssh://") || u.starts_with("git@")) && domain.host_str() == Some(host.as_str())))
                && path.split_once('/').is_some_and(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/')) {
                    return mk(Provider::Gitea, service, authority, path.clone(), format!("https://{authority}/{path}"));
                }
        }
    }
    let ghe = clean_host(&c.github_enterprise_host);
    if !ghe.is_empty() && host == ghe {
        return simple(Provider::Github, "github-enterprise");
    }
    let bbs = clean_host(&c.bitbucket_server_host);
    if !bbs.is_empty() && host == bbs.split('/').next().unwrap_or(&bbs) {
        // https://host/scm/PROJECT/repo or ssh://git@host:7999/PROJECT/repo
        let p = path.trim_start_matches("scm/").to_string();
        let (proj, repo) = p.split_once('/')?;
        let web = format!("https://{bbs}/projects/{}/repos/{repo}", proj.to_uppercase());
        return mk(Provider::Bitbucket, "bitbucket-server", &bbs, format!("{proj}/{repo}"), web);
    }
    None
}

/// The remote a push / pull talks to, and whether it signs in with SSH.
#[derive(Serialize, Default, Debug)]
pub struct RemoteAuth {
    pub remote: String,
    pub url: String,
    pub ssh: bool,
    pub host: String,
    /// The same repository's HTTPS address (for "Use HTTPS instead").
    pub https_url: Option<String>,
    /// GitCraft has a token for this host from Preferences > Integrations.
    pub connected: bool,
    /// The integration this remote belongs to, if any.
    pub service: String,
}

pub fn https_url(url: &str) -> Option<String> {
    let u = url.trim();
    let (host, path) = if let Some(rest) = u.strip_prefix("git@") {
        rest.split_once(':')?
    } else {
        let rest = u.strip_prefix("ssh://")?;
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (h, p) = rest.split_once('/')?;
        (h.split(':').next().unwrap_or(h), p)
    };
    let path = path.trim_matches('/').trim_end_matches(".git");
    if host == "ssh.dev.azure.com" {
        let p: Vec<&str> = path.trim_start_matches("v3/").split('/').collect();
        if p.len() == 3 {
            return Some(format!("https://dev.azure.com/{}/{}/_git/{}", p[0], p[1], p[2]));
        }
    }
    (!host.is_empty() && !path.is_empty()).then(|| format!("https://{host}/{path}.git"))
}

pub fn remote_auth(dir: &std::path::Path, gitlab_host: &str) -> RemoteAuth {
    let up = crate::git::run(dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
    let remotes: Vec<String> = crate::git::run(dir, &["remote"]).stdout.lines().map(|s| s.to_string()).collect();
    let remote = up
        .ok
        .then(|| up.stdout.trim().split('/').next().unwrap_or_default().to_string())
        .filter(|r| remotes.contains(r))
        .or_else(|| remotes.iter().find(|r| *r == "origin").cloned())
        .or_else(|| remotes.first().cloned())
        .unwrap_or_default();
    if remote.is_empty() {
        return RemoteAuth::default();
    }
    let url = crate::git::run(dir, &["remote", "get-url", "--push", &remote]).stdout.trim().to_string();
    let ssh = url.starts_with("git@") || url.starts_with("ssh://");
    let r = parse_remote(&url, gitlab_host);
    let service = r.as_ref().map(|r| r.service.clone()).unwrap_or_default();
    let connected = !service.is_empty() && secret_get(&secret_name(&service)).is_some();
    RemoteAuth { https_url: https_url(&url), remote, url, ssh, host: r.map(|r| r.host).unwrap_or_default(), connected, service }
}

/// Who the token belongs to, for the provider of a remote (kept for the older Accounts screen).
pub fn whoami(provider: &Provider, gitlab_host: &str) -> Result<String, String> {
    match provider {
        Provider::Github => whoami_id("github"),
        Provider::Gitlab if gitlab_host.trim().is_empty() || gitlab_host.trim() == "gitlab.com" => whoami_id("gitlab"),
        Provider::Gitlab => whoami_id("gitlab-self"),
        Provider::Bitbucket => whoami_id("bitbucket"),
        Provider::Azure => whoami_id("azure"),
        Provider::Gitea => Err("Specify a connected Gitea, Forgejo or Codeberg service.".into()),
    }
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
    fn ssh_addresses_become_https() {
        assert_eq!(https_url("git@github.com:MystifiedSky/GitCraft.git").as_deref(), Some("https://github.com/MystifiedSky/GitCraft.git"));
        assert_eq!(https_url("ssh://git@gitlab.example.com:2222/team/app").as_deref(), Some("https://gitlab.example.com/team/app.git"));
        assert_eq!(https_url("git@ssh.dev.azure.com:v3/org/proj/repo").as_deref(), Some("https://dev.azure.com/org/proj/_git/repo"));
        assert_eq!(https_url("https://github.com/a/b.git"), None);
    }

    #[test]
    fn tokens_reach_git_through_the_environment_only() {
        let env = crate::git::credential_env(&[("github.com".into(), "x-access-token".into(), "secret123".into())]);
        let get = |k: &str| env.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
        assert_eq!(get("GIT_CONFIG_COUNT"), "2");
        assert_eq!(get("GIT_CONFIG_KEY_0"), "credential.https://github.com.helper");
        assert_eq!(get("GIT_CONFIG_VALUE_0"), "");
        assert!(get("GIT_CONFIG_VALUE_1").contains("$GITCRAFT_TOKEN_0") && !get("GIT_CONFIG_VALUE_1").contains("secret123"));
        assert_eq!(get("GITCRAFT_TOKEN_0"), "secret123");
        // Git really reads that helper: ask it for credentials of a made-up repository.
        let mut c = std::process::Command::new("git");
        c.args(["credential", "fill"]).envs(env.iter().cloned()).env("GIT_TERMINAL_PROMPT", "0");
        c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        let mut ch = c.spawn().unwrap();
        use std::io::Write;
        ch.stdin.take().unwrap().write_all(b"protocol=https\nhost=github.com\npath=x/y.git\n\n").unwrap();
        let out = String::from_utf8_lossy(&ch.wait_with_output().unwrap().stdout).into_owned();
        assert!(out.contains("username=x-access-token") && out.contains("password=secret123"), "{out}");
    }

    #[test]
    fn understands_remote_urls_and_services() {
        let gh = parse_remote("https://github.com/octo/hello.git", "").unwrap();
        assert_eq!((gh.provider.clone(), gh.path.as_str(), gh.web.as_str()), (Provider::Github, "octo/hello", "https://github.com/octo/hello"));
        assert_eq!(parse_remote("git@github.com:octo/hello.git", "").unwrap().path, "octo/hello");
        let gl = parse_remote("ssh://git@gitlab.com/group/sub/project.git", "").unwrap();
        assert_eq!((gl.provider, gl.path.as_str()), (Provider::Gitlab, "group/sub/project"));
        let own = parse_remote("https://git.example.org/team/app", "git.example.org").unwrap();
        assert_eq!((own.provider, own.service.as_str()), (Provider::Gitlab, "gitlab-self"));
        let bb = parse_remote("git@bitbucket.org:team/app.git", "").unwrap();
        assert_eq!((bb.provider, bb.service.as_str(), bb.path.as_str()), (Provider::Bitbucket, "bitbucket", "team/app"));
        for url in ["https://dev.azure.com/org/Proj/_git/repo", "https://org@dev.azure.com/org/Proj/_git/repo", "git@ssh.dev.azure.com:v3/org/Proj/repo", "https://org.visualstudio.com/Proj/_git/repo"] {
            let az = parse_remote(url, "").unwrap();
            assert_eq!((az.service.as_str(), az.path.as_str()), ("azure", "org/Proj/repo"), "{url}");
        }
        assert!(parse_remote("C:/repos/local.git", "").is_none());
        assert!(parse_remote("https://example.com/a/b", "").is_none());
        // Self-hosted services come from the Integrations settings.
        set_config(Config { github_enterprise_host: "https://git.corp.example/".into(), jira_site: "acme.atlassian.net".into(), azure_org: "acme".into(), ..Default::default() });
        assert_eq!(host_of("github-enterprise"), "git.corp.example");
        assert_eq!(api_base("github-enterprise"), "https://git.corp.example/api/v3");
        assert_eq!(api_base("jira"), "https://acme.atlassian.net/rest/api/3");
        assert_eq!(api_base("azure"), "https://dev.azure.com/acme");
        let ghe = parse_remote("git@git.corp.example:team/app.git", "").unwrap();
        assert_eq!((ghe.provider, ghe.service.as_str()), (Provider::Github, "github-enterprise"));
        assert_eq!(ssh_keys_page("github"), "https://github.com/settings/ssh/new");
        set_config(Config::default());
        assert_eq!(secret_name("github"), "github-token");
    }

    #[test]
    fn issue_branch_names() {
        assert_eq!(branch_for_issue(42, "Crash when saving: file > 2 GB!"), "issue-42-crash-when-saving-file-2-gb");
    }
}

/// Match the configured push destination, including renamed upstream branches.
pub fn pull_request_destination(dir: &std::path::Path, branch: &str, gitlab_host: &str) -> Result<(RemoteRepo, String), String> {
    let remotes = crate::git::run(dir, &["remote"]).stdout;
    let configured = crate::git::run(dir, &["config", "--get", &format!("branch.{branch}.remote")]).stdout.trim().to_owned();
    let remote = if !configured.is_empty() { configured } else if remotes.lines().any(|r| r == "origin") { "origin".into() } else { remotes.lines().next().unwrap_or_default().into() };
    if remote.is_empty() || !remotes.lines().any(|r| r == remote) { return Err("Choose a server remote for this branch before starting a pull request.".into()); }
    let merge = crate::git::run(dir, &["config", "--get", &format!("branch.{branch}.merge")]).stdout.trim().to_owned();
    let destination = if merge.is_empty() { branch.to_owned() } else { merge.strip_prefix("refs/heads/").ok_or("The upstream must point to a branch.")?.to_owned() };
    let url = crate::git::run(dir, &["remote", "get-url", "--push", &remote]);
    if !url.ok { return Err(url.message()); }
    let repository = parse_remote(url.stdout.trim(), gitlab_host).ok_or("The push remote is not a supported hosting service.")?;
    Ok((repository, destination))
}


#[cfg(test)]
mod http_safety_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    #[test]
    fn cancelled_http_body_releases_the_connection_promptly() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap(); let address = server.local_addr().unwrap();
        let (ready, wait) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap(); stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut bytes = [0;8192]; let _ = stream.read(&mut bytes);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 99999\r\n\r\n{").unwrap(); ready.send(()).unwrap();
            let started = std::time::Instant::now(); let closed = stream.read(&mut bytes).map(|n| n == 0).unwrap_or(true);
            assert!(closed && started.elapsed() < std::time::Duration::from_secs(2));
        });
        let request = std::thread::spawn(move || crate::read_jobs::scope(Some("stalled-http".into()), || http_page("GET", &format!("http://{address}/stalled"), &Auth::Bearer("fixture".into()), None)));
        wait.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let started = std::time::Instant::now(); crate::read_jobs::cancel("stalled-http");
        assert!(request.join().unwrap().err().unwrap().contains("cancelled"));
        assert!(started.elapsed() < std::time::Duration::from_secs(2)); worker.join().unwrap();
    }

    #[test]
    fn cancelled_dns_wait_does_not_free_a_running_lookup_slot() {
        crate::runtime().block_on(async {
            let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
            let (started, ready) = tokio::sync::oneshot::channel();
            let (finish, release) = std::sync::mpsc::channel();
            let task = tokio::spawn(SystemDnsResolver::lookup(slots.clone(), move || {
                let _ = started.send(());
                let _ = release.recv_timeout(Duration::from_secs(5));
                Ok(Box::new(std::iter::empty()))
            }));
            tokio::time::timeout(Duration::from_secs(3), ready).await.unwrap().unwrap();
            task.abort();
            assert!(task.await.is_err());
            assert!(slots.clone().try_acquire_owned().is_err());
            finish.send(()).unwrap();
            assert!(tokio::time::timeout(Duration::from_secs(3), slots.acquire()).await.unwrap().is_ok());
        });
    }

    #[test]
    fn transport_reuses_connections_without_reusing_credentials_and_reconnects() {
        use std::io::BufRead;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            let mut connections = 0;
            while requests.len() < 6 {
                let (stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                let mut reader = std::io::BufReader::new(stream);
                connections += 1;
                while requests.len() < 6 {
                    let mut request = String::new();
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" { break; }
                        request.push_str(&line);
                    }
                    if request.is_empty() { break; }
                    assert_eq!(line, "\r\n", "truncated HTTP headers");
                    requests.push(request);
                    let close = requests.len() == 5;
                    let extra = if close { "Connection: close\r\n" } else { "" };
                    write!(reader.get_mut(), "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n{extra}\r\n{{}}").unwrap();
                    if close { break; }
                }
            }
            (requests, connections)
        });
        for index in 0..6 {
            let token = format!("fixture-account-{index}");
            assert!(http_page("GET", &format!("http://{address}/collection"), &Auth::Bearer(token), None).is_ok());
        }
        let (requests, connections) = worker.join().unwrap();
        assert!((2..requests.len()).contains(&connections), "transport did not both reuse and reopen a connection");
        for (index, request) in requests.iter().enumerate() {
            assert!(request.contains(&format!("Bearer fixture-account-{index}")));
            assert_eq!(request.to_ascii_lowercase().matches("authorization:").count(), 1);
        }
    }

    #[test]
    fn rate_limit_backoff_and_header_only_trello_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            use std::io::BufRead;
            let (stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut reader = std::io::BufReader::new(stream);
            let mut lines = Vec::new();
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" { break; }
                lines.push(line);
            }
            assert_eq!(lines[0].trim_end(), "GET /cards HTTP/1.1");
            assert!(lines.iter().any(|line| line.eq_ignore_ascii_case("authorization: OAuth oauth_consumer_key=\"key\", oauth_token=\"token\"\r\n")));
            reader.get_mut().write_all(b"HTTP/1.1 403 Forbidden\r\nRetry-After: 60\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            listener
        });
        let url = format!("http://{address}/cards");
        let auth = Auth::Trello("key".into(), "token".into());
        let first = http_page("GET", &url, &auth, None).err().unwrap();
        let listener = worker.join().unwrap();
        listener.set_nonblocking(true).unwrap();
        let second = http_page("GET", &url, &auth, None).err().unwrap();
        assert!(first.contains("rate limited until"), "{first}");
        assert_eq!(second, first);
        assert!(listener.accept().is_err());
    }

    #[test]
    fn credentials_do_not_follow_redirects_or_appear_in_errors() {
        let destination=TcpListener::bind("127.0.0.1:0").unwrap();destination.set_nonblocking(true).unwrap();
        let destination_port=destination.local_addr().unwrap().port();
        for auth in [Auth::Bearer("synthetic-secret".into()),Auth::Header("PRIVATE-TOKEN","synthetic-secret".into()),Auth::Basic("synthetic-user".into(),"synthetic-secret".into()),Auth::Trello("synthetic-key".into(),"synthetic-secret".into())] {
            let server=TcpListener::bind("127.0.0.1:0").unwrap();let address=server.local_addr().unwrap();
            let worker=std::thread::spawn(move || {let (mut stream,_)=server.accept().unwrap();let mut bytes=[0u8;8192];let _=stream.read(&mut bytes);write!(stream,"HTTP/1.1 302 Found\r\nLocation: http://localhost:{destination_port}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();});
            let result=http_page("GET",&format!("http://{address}/cards"),&auth,None);
            assert!(result.is_err());worker.join().unwrap();assert!(destination.accept().is_err());
            let unavailable=TcpListener::bind("127.0.0.1:0").unwrap();let address=unavailable.local_addr().unwrap();drop(unavailable);
            let error=http_page("GET",&format!("http://{address}/cards"),&auth,None).err().unwrap();
            assert!(!error.contains("synthetic-secret"));assert!(!error.contains("synthetic-key"));
        }
        let server=TcpListener::bind("127.0.0.1:0").unwrap();let address=server.local_addr().unwrap();
        let worker=std::thread::spawn(move || {let(mut stream,_)=server.accept().unwrap();let mut bytes=[0u8;8192];let _=stream.read(&mut bytes);let body=r#"{"message":"synthetic-key synthetic-secret"}"#;write!(stream,"HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();});
        let error=http_page("GET",&format!("http://{address}/cards"),&Auth::Trello("synthetic-key".into(),"synthetic-secret".into()),None).err().unwrap();worker.join().unwrap();
        assert!(!error.contains("synthetic-"));assert!(error.contains("400"));
    }
}
