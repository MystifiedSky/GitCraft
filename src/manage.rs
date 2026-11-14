//! Stage 4: repository management — remotes, worktrees, submodules, branch clean-up and Gitflow.
//! Gitflow uses these config keys:
//! `gitflow.branch.master` / `gitflow.branch.develop` / `gitflow.prefix.*`, defaults main, develop,
//! feature/, release/, hotfix/ and an empty version-tag prefix.

use crate::git::{self, GitOut};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const GITFLOW_DEFAULTS: [(&str, &str); 6] = [
    ("gitflow.branch.master", "main"),
    ("gitflow.branch.develop", "develop"),
    ("gitflow.prefix.feature", "feature/"),
    ("gitflow.prefix.release", "release/"),
    ("gitflow.prefix.hotfix", "hotfix/"),
    ("gitflow.prefix.versiontag", ""),
];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GitflowConfig {
    pub master: String,
    pub develop: String,
    pub feature: String,
    pub release: String,
    pub hotfix: String,
    pub versiontag: String,
    /// Gitflow has been initialised in this repository (its keys exist).
    pub initialized: bool,
}

pub fn gitflow_config(dir: &Path) -> GitflowConfig {
    let get = |k: &str| {
        let o = git::run(dir, &["config", "--get", k]);
        o.ok.then(|| o.stdout.trim_end_matches(['\r', '\n']).to_string())
    };
    let initialized = get("gitflow.branch.develop").is_some();
    let val = |k: &str| get(k).unwrap_or_else(|| GITFLOW_DEFAULTS.iter().find(|(n, _)| *n == k).unwrap().1.to_string());
    GitflowConfig {
        master: val("gitflow.branch.master"),
        develop: val("gitflow.branch.develop"),
        feature: val("gitflow.prefix.feature"),
        release: val("gitflow.prefix.release"),
        hotfix: val("gitflow.prefix.hotfix"),
        versiontag: val("gitflow.prefix.versiontag"),
        initialized,
    }
}

fn exists(dir: &Path, branch: &str) -> bool {
    git::run(dir, &["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")]).ok
}

fn err(msg: impl Into<String>) -> GitOut {
    GitOut { ok: false, stdout: String::new(), stderr: msg.into() }
}

fn ok() -> GitOut {
    GitOut { ok: true, stdout: String::new(), stderr: String::new() }
}

/// "Initialize Gitflow": store the branch names and prefixes, and create develop from main if needed.
pub fn gitflow_init(dir: &Path, cfg: &GitflowConfig, run: &dyn Fn(&[&str]) -> GitOut) -> GitOut {
    let pairs = [
        ("gitflow.branch.master", &cfg.master),
        ("gitflow.branch.develop", &cfg.develop),
        ("gitflow.prefix.feature", &cfg.feature),
        ("gitflow.prefix.release", &cfg.release),
        ("gitflow.prefix.hotfix", &cfg.hotfix),
        ("gitflow.prefix.versiontag", &cfg.versiontag),
    ];
    for (k, v) in pairs {
        let o = run(&["config", k, v.as_str()]);
        if !o.ok {
            return o;
        }
    }
    if !exists(dir, &cfg.master) {
        return err(format!("{} branch is required", cfg.master));
    }
    if !exists(dir, &cfg.develop) {
        return run(&["branch", &cfg.develop, &cfg.master]);
    }
    ok()
}

/// Start a feature (from develop), release (from develop) or hotfix (from main) and check it out.
pub fn gitflow_start(dir: &Path, kind: &str, name: &str, run: &dyn Fn(&[&str]) -> GitOut) -> GitOut {
    let cfg = gitflow_config(dir);
    let name = name.trim().replace(' ', "-");
    if name.is_empty() {
        return err(format!("{} name is required", capital(kind)));
    }
    let (prefix, from) = match kind {
        "feature" => (&cfg.feature, &cfg.develop),
        "release" => (&cfg.release, &cfg.develop),
        "hotfix" => (&cfg.hotfix, &cfg.master),
        _ => return err("unknown Gitflow branch type"),
    };
    if !exists(dir, from) {
        return err(format!("{from} branch is required to start {kind}"));
    }
    run(&["checkout", "-q", "-b", &format!("{prefix}{name}"), from])
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Finish: a feature merges into develop; a release or hotfix merges into main (tagged with the
/// version-tag prefix + name), then into develop. The branch is deleted unless `keep`.
pub fn gitflow_finish(dir: &Path, kind: &str, name: &str, keep: bool, tag_message: &str, run: &dyn Fn(&[&str]) -> GitOut) -> GitOut {
    let cfg = gitflow_config(dir);
    let prefix = match kind {
        "feature" => &cfg.feature,
        "release" => &cfg.release,
        "hotfix" => &cfg.hotfix,
        _ => return err("unknown Gitflow branch type"),
    };
    let branch = format!("{prefix}{name}");
    if !exists(dir, &branch) {
        return err(format!("No {kind} to finish"));
    }
    let merge_into = |target: &str| -> GitOut {
        let o = run(&["checkout", "-q", target]);
        if !o.ok {
            return o;
        }
        let args = crate::extras::merge_args(dir, "merge", &["--no-ff", "--no-edit", &branch]);
        run(&args.iter().map(String::as_str).collect::<Vec<_>>())
    };
    if kind == "feature" {
        if !exists(dir, &cfg.develop) {
            return err(format!("{} branch is required to finish feature", cfg.develop));
        }
        let o = merge_into(&cfg.develop);
        if !o.ok {
            return o;
        }
    } else {
        for b in [&cfg.master, &cfg.develop] {
            if !exists(dir, b) {
                return err(format!("{b} branch is required to finish {kind}"));
            }
        }
        let o = merge_into(&cfg.master);
        if !o.ok {
            return o;
        }
        let tag = format!("{}{name}", cfg.versiontag);
        let msg = if tag_message.trim().is_empty() { tag.clone() } else { tag_message.trim().to_string() };
        let o = run(&["tag", "-a", &tag, "-m", &msg]);
        if !o.ok {
            return o;
        }
        let o = merge_into(&cfg.develop);
        if !o.ok {
            return o;
        }
    }
    if !keep {
        return run(&["branch", "-d", &branch]);
    }
    ok()
}

#[derive(Serialize, Clone, Debug)]
pub struct CleanupBranch {
    pub name: String,
    pub time: i64,
    pub upstream: String,
    /// Fully merged into the checked-out branch.
    pub merged: bool,
}

/// Local branches for "Clean Up Local Branches" (the checked-out branch is left out).
pub fn cleanup_list(dir: &Path) -> Vec<CleanupBranch> {
    let merged: Vec<String> = git::run(dir, &["branch", "--merged", "HEAD", "--format=%(refname:short)"]).stdout.lines().map(|s| s.to_string()).collect();
    let head = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
    git::run(dir, &["for-each-ref", "--format=%(refname:short)%1f%(committerdate:unix)%1f%(upstream:short)", "refs/heads"])
        .stdout
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\x1f').collect();
            (f.len() == 3 && f[0] != head).then(|| CleanupBranch {
                name: f[0].to_string(),
                time: f[1].parse().unwrap_or(0),
                upstream: f[2].to_string(),
                merged: merged.iter().any(|m| m == f[0]),
            })
        })
        .collect()
}

#[derive(Serialize, Clone, Debug)]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

pub fn remotes(dir: &Path) -> Vec<Remote> {
    let mut out: Vec<Remote> = Vec::new();
    for l in git::run(dir, &["remote", "-v"]).stdout.lines() {
        let mut it = l.split_whitespace();
        let (Some(name), Some(url), Some(kind)) = (it.next(), it.next(), it.next()) else { continue };
        let r = match out.iter_mut().find(|r| r.name == name) {
            Some(r) => r,
            None => {
                out.push(Remote { name: name.into(), fetch_url: String::new(), push_url: String::new() });
                out.last_mut().unwrap()
            }
        };
        if kind.contains("fetch") {
            r.fetch_url = url.into();
        } else {
            r.push_url = url.into();
        }
    }
    out
}

#[derive(Serialize, Clone, Debug)]
pub struct Submodule {
    pub path: String,
    pub sha: String,
    /// in-sync, out-of-sync, uninitialized or conflict (from `git submodule status`).
    pub state: String,
    pub url: String,
}

pub fn submodules(dir: &Path) -> Vec<Submodule> {
    git::run(dir, &["submodule", "status"])
        .stdout
        .lines()
        .filter_map(|l| {
            let state = match l.chars().next()? {
                '-' => "uninitialized",
                '+' => "out-of-sync",
                'U' => "conflict",
                _ => "in-sync",
            };
            let mut it = l[1..].split_whitespace();
            let sha = it.next()?.to_string();
            let path = it.next()?.to_string();
            let url = git::run(dir, &["config", "--file", ".gitmodules", "--get", &format!("submodule.{path}.url")]).stdout.trim().to_string();
            Some(Submodule { path, sha, state: state.into(), url })
        })
        .collect()
}


/// Git LFS: whether it's installed, the tracked patterns and the current locks.
#[derive(Serialize, Default)]
pub struct LfsInfo {
    pub available: bool,
    pub version: String,
    /// LFS hooks are installed in this repository.
    pub enabled: bool,
    pub patterns: Vec<String>,
    pub locks: Vec<LfsLock>,
}

#[derive(Serialize, Clone)]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: String,
}

pub fn lfs_info(dir: &Path) -> LfsInfo {
    let v = git::run(dir, &["lfs", "version"]);
    if !v.ok {
        return LfsInfo::default();
    }
    let gd = git::run(dir, &["rev-parse", "--absolute-git-dir"]).stdout.trim().to_string();
    let enabled = std::fs::read_to_string(Path::new(&gd).join("hooks").join("pre-push")).map(|s| s.contains("git lfs")).unwrap_or(false);
    let patterns = std::fs::read_to_string(dir.join(".gitattributes"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("filter=lfs"))
        .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
        .collect();
    // Locks need a server that supports them; ask without failing if it doesn't.
    let locks = git::run(dir, &["lfs", "locks", "--json"]);
    let locks = serde_json::from_str::<Vec<serde_json::Value>>(locks.stdout.trim())
        .unwrap_or_default()
        .iter()
        .map(|l| LfsLock {
            id: l["id"].as_str().unwrap_or_default().to_string(),
            path: l["path"].as_str().unwrap_or_default().to_string(),
            owner: l["owner"]["name"].as_str().unwrap_or_default().to_string(),
        })
        .collect();
    LfsInfo { available: true, version: v.stdout.trim().to_string(), enabled, patterns, locks }
}

/// Add a pattern to .gitignore unless it's already there.
pub fn ignore(dir: &Path, pattern: &str) -> Result<bool, String> {
    let p = dir.join(".gitignore");
    let cur = std::fs::read_to_string(&p).unwrap_or_default();
    if cur.lines().any(|l| l.trim() == pattern) {
        return Ok(false);
    }
    let mut next = cur.clone();
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(pattern);
    next.push('\n');
    std::fs::write(p, next).map_err(|e| e.to_string())?;
    Ok(true)
}

#[derive(Serialize, Default)]
pub struct SparseInfo {
    pub enabled: bool,
    pub patterns: Vec<String>,
}

pub fn sparse_info(dir: &Path) -> SparseInfo {
    let on = git::run(dir, &["config", "--bool", "core.sparseCheckout"]).stdout.trim() == "true";
    let patterns = if on { git::run(dir, &["sparse-checkout", "list"]).stdout.lines().map(|s| s.to_string()).collect() } else { Vec::new() };
    SparseInfo { enabled: on, patterns }
}

/// .gitignore templates offered when creating a repository (GitCraft's own short versions).
pub const GITIGNORE_TEMPLATES: [(&str, &str); 8] = [
    ("Node", "node_modules/\ndist/\nbuild/\ncoverage/\n.env\n.env.*\nnpm-debug.log*\nyarn-error.log*\n.DS_Store\n"),
    ("Rust", "/target/\n**/*.rs.bk\n*.pdb\n"),
    ("Python", "__pycache__/\n*.py[cod]\n.venv/\nvenv/\n.env\n*.egg-info/\ndist/\nbuild/\n.pytest_cache/\n"),
    ("Java", "*.class\n*.jar\n*.war\ntarget/\nbuild/\n.gradle/\nout/\n.idea/\n*.iml\n"),
    ("VisualStudio", "bin/\nobj/\n.vs/\n*.user\n*.suo\n*.cache\npackages/\nTestResults/\n"),
    ("Go", "/bin/\n*.exe\n*.test\n*.out\nvendor/\n"),
    ("Unity", "/[Ll]ibrary/\n/[Tt]emp/\n/[Oo]bj/\n/[Bb]uild/\n/[Bb]uilds/\n/[Ll]ogs/\n/[Uu]ser[Ss]ettings/\n*.csproj\n*.sln\n"),
    ("macOS / Windows", ".DS_Store\nThumbs.db\nDesktop.ini\n"),
];

/// The MIT licence, filled in with the year and name.
pub fn mit_license(year: i32, name: &str) -> String {
    format!(
        "MIT License\n\nCopyright (c) {year} {name}\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n"
    )
}

#[derive(Deserialize, Default)]
pub struct InitOptions {
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub gitignore: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub readme: bool,
    /// Set up Git LFS for the new repository (`git lfs install --local`).
    #[serde(default)]
    pub lfs: bool,
}

/// Write the starting files and make the first commit if there are any.
pub fn init_files(dir: &Path, name: &str, opts: &InitOptions, run: &dyn Fn(&[&str]) -> GitOut) -> GitOut {
    let mut wrote = false;
    let mut ignore = String::new();
    for t in &opts.gitignore {
        if let Some((n, body)) = GITIGNORE_TEMPLATES.iter().find(|(n, _)| n == t) {
            ignore.push_str(&format!("# {n}\n{body}\n"));
        }
    }
    if !ignore.is_empty() {
        if let Err(e) = std::fs::write(dir.join(".gitignore"), ignore) {
            return err(e.to_string());
        }
        wrote = true;
    }
    if opts.license == "MIT" {
        let who = git::run(dir, &["config", "user.name"]).stdout.trim().to_string();
        let year: i32 = chrono::Local::now().format("%Y").to_string().parse().unwrap_or(2026);
        if let Err(e) = std::fs::write(dir.join("LICENSE"), mit_license(year, if who.is_empty() { "the authors" } else { &who })) {
            return err(e.to_string());
        }
        wrote = true;
    }
    if opts.readme {
        if let Err(e) = std::fs::write(dir.join("README.md"), format!("# {name}\n")) {
            return err(e.to_string());
        }
        wrote = true;
    }
    if !wrote {
        // The first commit is made even with nothing to add; when Git has no author name yet this is skipped quietly.
        let _ = run(&["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
        return ok();
    }
    let o = run(&["add", "-A"]);
    if !o.ok {
        return o;
    }
    run(&["commit", "-q", "-m", "Initial commit"])
}
