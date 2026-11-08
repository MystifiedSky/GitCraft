//! Builds a small sample repository so GitCraft can be tried (and pictured) without a project.

use crate::git::{run, run_env};
use std::fs;
use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("GitCraft")
}

/// Returns the path of the sample repository, creating it the first time.
pub fn ensure_sample() -> Result<PathBuf, String> {
    let root = data_dir();
    let repo = root.join("sample-project");
    if repo.join(".git").exists() {
        return Ok(repo);
    }
    let _ = fs::remove_dir_all(&repo);
    let origin = root.join("sample-origin.git");
    let _ = fs::remove_dir_all(&origin);
    fs::create_dir_all(&repo).map_err(|e| e.to_string())?;

    let ok = |o: crate::git::GitOut| if o.ok { Ok(()) } else { Err(o.message()) };
    ok(run(&repo, &["init", "-q", "-b", "master"]))?;
    ok(run(&repo, &["config", "user.name", "Sam Rivera"]))?;
    ok(run(&repo, &["config", "user.email", "sam@example.com"]))?;
    ok(run(&repo, &["config", "commit.gpgsign", "false"]))?;

    let t = std::cell::Cell::new(1_760_000_000i64); // fixed dates so the sample always looks the same
    let people = [("Sam Rivera", "sam@example.com"), ("Alex Chen", "alex@example.com"), ("Jo Park", "jo@example.com")];
    let n = std::cell::Cell::new(0usize);
    let commit = |repo: &Path, file: &str, text: &str, msg: &str| -> Result<(), String> {
        let p = repo.join(file);
        if let Some(d) = p.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut cur = fs::read_to_string(&p).unwrap_or_default();
        cur.push_str(text);
        cur.push('\n');
        fs::write(&p, cur).map_err(|e| e.to_string())?;
        ok(run(repo, &["add", "-A"]))?;
        t.set(t.get() + 3_600 * 5);
        n.set(n.get() + 1);
        let (name, mail) = people[n.get() % people.len()];
        let date = format!("{} +0000", t.get());
        ok(run_env(
            repo,
            &["commit", "-q", "-m", msg],
            &[
                ("GIT_AUTHOR_DATE", &date),
                ("GIT_COMMITTER_DATE", &date),
                ("GIT_AUTHOR_NAME", name),
                ("GIT_AUTHOR_EMAIL", mail),
                ("GIT_COMMITTER_NAME", name),
                ("GIT_COMMITTER_EMAIL", mail),
            ],
        ))
    };

    commit(&repo, "README.md", "# Sample Project\n\nA small project to try GitCraft with.", "chore: initial commit")?;
    commit(&repo, "src/main.rs", "fn main() {\n    println!(\"hello\");\n}", "feat(core): add entry point")?;
    commit(&repo, "src/config.rs", "pub struct Config { pub name: String }", "feat(config): add config loader")?;
    commit(&repo, "src/config.rs", "impl Config { pub fn new() -> Self { Config { name: String::new() } } }", "fix(config): default name is empty")?;
    commit(&repo, "docs/guide.md", "# Guide\n\nHow to use the sample.", "docs: add user guide")?;
    ok(run(&repo, &["tag", "v0.1.0"]))?;
    ok(run(&repo, &["checkout", "-q", "-b", "feature/storage"]))?;
    commit(&repo, "src/storage.rs", "pub fn save() {}", "feat(storage): add save function")?;
    commit(&repo, "src/storage.rs", "pub fn load() {}", "feat(storage): add load function")?;
    ok(run(&repo, &["checkout", "-q", "master"]))?;
    commit(&repo, "src/ui.rs", "pub fn draw() {}", "feat(ui): draw main screen")?;
    commit(&repo, "src/ui.rs", "pub fn resize() {}", "fix(ui): handle window resize")?;
    t.set(t.get() + 3_600);
    let date = format!("{} +0000", t.get());
    ok(run_env(
        &repo,
        &["merge", "-q", "--no-ff", "feature/storage", "-m", "Merge branch 'feature/storage'"],
        &[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)],
    ))?;
    commit(&repo, "src/sql.rs", "pub fn query() {}", "fix(sql): escape table names")?;
    commit(&repo, "build.sh", "echo build", "build(deps): update build script")?;
    commit(&repo, "src/search.rs", "pub fn search() {}", "feat(search): add quick search")?;
    commit(&repo, "src/api.rs", "pub fn get() {}", "feat(api): add get endpoint")?;
    commit(&repo, "src/api.rs", "pub fn post() {}", "feat(api): add post endpoint")?;
    commit(&repo, "src/cache.rs", "pub fn clear() {}", "feat(cache): clear cache on start")?;
    ok(run(&repo, &["tag", "v0.2.0"]))?;
    ok(run(&repo, &["checkout", "-q", "-b", "develop"]))?;
    commit(&repo, "src/theme.rs", "pub fn dark() {}", "feat(theme): add dark theme")?;
    ok(run(&repo, &["checkout", "-q", "master"]))?;
    commit(&repo, "src/log.rs", "pub fn info() {}", "feat(log): add info logging")?;
    commit(&repo, "src/config.rs", "// reload support", "fix(config): reload on change")?;
    commit(&repo, "src/main.rs", "// parse args", "feat(cli): parse command line")?;
    commit(&repo, "src/api.rs", "// timeout 30s", "fix(api): add request timeout")?;
    commit(&repo, "CHANGELOG.md", "## 0.3.0", "build(release): prepare 0.3.0")?;

    // A remote so pull and push have somewhere to go.
    ok(run(&root, &["clone", "-q", "--bare", repo.to_string_lossy().as_ref(), origin.to_string_lossy().as_ref()]))?;
    ok(run(&repo, &["remote", "add", "origin", origin.to_string_lossy().as_ref()]))?;
    ok(run(&repo, &["fetch", "-q", "origin"]))?;
    ok(run(&repo, &["branch", "-q", "--set-upstream-to=origin/master", "master"]))?;

    // Two uncommitted changes so the work-in-progress row shows.
    fs::write(repo.join("src/sql.rs"), "pub fn query() {}\npub fn escape() {}\n").map_err(|e| e.to_string())?;
    fs::write(repo.join("src/notes.txt"), "Ideas for the next release\n").map_err(|e| e.to_string())?;
    Ok(repo)
}
