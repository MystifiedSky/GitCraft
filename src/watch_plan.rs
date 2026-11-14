//! Choose directory watches with Git's own ignore rules before registering with the OS.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct WatchPlan {
    pub directories: HashSet<PathBuf>,
    pub configuration: HashSet<PathBuf>,
    pub registrations: HashSet<PathBuf>,
    pub visited: usize,
    settings: HashMap<PathBuf, HashSet<PathBuf>>,
}

fn normalized(path: PathBuf) -> PathBuf { path.canonicalize().unwrap_or(path) }

fn output(root: &Path, args: &[&str]) -> Result<String, String> {
    let result = crate::git::run(root, args);
    if result.ok { Ok(result.stdout) } else { Err(result.message()) }
}

fn defaults() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|home| home.join(".config")));
    let mut files = Vec::new();
    if let Some(global) = std::env::var_os("GIT_CONFIG_GLOBAL") { files.push(global.into()); }
    else {
        if let Some(home) = &home { files.push(home.join(".gitconfig")); }
        if let Some(xdg) = &xdg { files.push(xdg.join("git/config")); }
    }
    if let Some(xdg) = xdg { files.push(xdg.join("git/ignore")); }
    files
}

impl WatchPlan {
    pub fn read(root: &Path) -> Result<Self, String> { Self::build(root, &HashMap::new()) }
    pub fn refresh(root: &Path, previous: &Self) -> Result<Self, String> { Self::build(root, &previous.settings) }
    fn build(root: &Path, cached: &HashMap<PathBuf, HashSet<PathBuf>>) -> Result<Self, String> {
        let root = normalized(root.to_path_buf());
        let mut plan = Self { directories: HashSet::new(), configuration: HashSet::new(), registrations: HashSet::new(), visited: 0, settings: HashMap::new() };
        plan.repository(&root, cached)?;
        for file in defaults() { plan.configuration.insert(normalized(file)); }
        plan.registrations = plan.directories.clone();
        for file in &plan.configuration {
            let mut parent = file.parent();
            while let Some(path) = parent {
                if path.is_dir() { plan.registrations.insert(normalized(path.to_path_buf())); break; }
                parent = path.parent();
            }
        }
        Ok(plan)
    }

    fn repository(&mut self, root: &Path, cached: &HashMap<PathBuf, HashSet<PathBuf>>) -> Result<(), String> {
        // --directory prunes ignored trees inside Git, without listing every ignored file.
        let ignored = output(root, &["ls-files", "--others", "--ignored", "--exclude-standard", "--directory", "-z"])?;
        let ignored: HashSet<_> = ignored.split('\0').filter(|name| name.ends_with('/')).map(|name| root.join(name.trim_end_matches('/'))).collect();
        let tracked = output(root, &["ls-files", "--cached", "-z"])?;
        let mut ancestors = HashSet::new();
        for name in tracked.split('\0').filter(|name| !name.is_empty()) {
            let full = root.join(name);
            let mut directory = if full.is_dir() { Some(full.as_path()) } else { full.parent() };
            while let Some(path) = directory.filter(|path| path.starts_with(root)) {
                ancestors.insert(path.to_path_buf());
                directory = path.parent();
            }
        }
        let files = if let Some(files) = cached.get(root) { files.clone() } else {
        let mut files = HashSet::new();
        let config = crate::git::run(root, &["config", "--list", "--includes", "--show-origin", "-z"]);
        if !config.ok && !config.message().is_empty() { return Err(config.message()); }
        let fields: Vec<_> = config.stdout.split('\0').collect();
        for record in fields.chunks_exact(2) {
            if let Some(file) = record[0].strip_prefix("file:") { files.insert(normalized(root.join(file))); }
        }
        let exclude = crate::git::run(root, &["config", "--path", "--get", "core.excludesFile"]);
        if exclude.ok { files.insert(normalized(root.join(exclude.stdout.trim()))); }
        let gitdir = output(root, &["rev-parse", "--absolute-git-dir"])?;
        let gitdir = PathBuf::from(gitdir.trim());
        let common = std::fs::read_to_string(gitdir.join("commondir")).ok().map(|path| normalized(gitdir.join(path.trim()))).unwrap_or_else(|| gitdir.clone());
        for path in [common.join("info/exclude"), common.join("config"), gitdir.join("config.worktree"), gitdir.join("HEAD"), gitdir.join("index")] { files.insert(normalized(path)); }
        files
        };
        self.configuration.extend(files.iter().cloned());
        self.settings.insert(root.into(), files);
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            if !self.directories.insert(directory.clone()) { continue; }
            self.visited += 1;
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("Cannot watch {}: {error}", directory.display())),
            };
            for entry in entries {
                let entry = entry.map_err(|error| error.to_string())?;
                if entry.file_name() == ".git" || !entry.file_type().map_err(|error| error.to_string())?.is_dir() { continue; }
                let path = entry.path();
                let excluded = path.ancestors().any(|ancestor| ignored.contains(ancestor));
                if excluded && !ancestors.contains(&path) { continue; }
                if path.join(".git").exists() { self.repository(&path, cached)?; }
                else { pending.push(path); }
            }
        }
        Ok(())
    }

    pub fn configuration_changed(&self, path: &Path) -> bool {
        self.configuration.iter().filter(|file| !matches!(file.file_name().and_then(|name| name.to_str()), Some("HEAD" | "index"))).any(|file| file == path || (!self.registrations.contains(path) && !path.is_file() && file.starts_with(path)))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    fn repository() -> tempfile::TempDir {
        let root=tempfile::tempdir().unwrap();
        assert!(crate::git::run(root.path(), &["init","-q"]).ok);
        root
    }
    #[test]
    fn prunes_ignored_trees_but_keeps_tracked_exceptions_and_empty_directories() {
        let folder=repository();let root=folder.path().canonicalize().unwrap();
        std::fs::write(root.join(".gitignore"),"build/\n").unwrap();
        std::fs::create_dir_all(root.join("build/tracked/deep")).unwrap();
        std::fs::write(root.join("build/tracked/deep/keep.txt"),"tracked").unwrap();
        assert!(crate::git::run(&root,&["add","-f","build/tracked/deep/keep.txt"]).ok);
        std::fs::create_dir(root.join("empty")).unwrap();
        for index in 0..600 {std::fs::create_dir_all(root.join(format!("build/ignored-{index}/deep"))).unwrap();}
        let start=std::time::Instant::now();let plan=WatchPlan::read(&root).unwrap();let elapsed=start.elapsed();
        assert_eq!(plan.visited,5);
        assert!(plan.directories.contains(&root.join("build/tracked/deep")));
        assert!(plan.directories.contains(&root.join("empty")));
        assert!(!plan.directories.contains(&root.join("build/ignored-0")));
        eprintln!("Selective registration: 1205 working directories, {} visited/registered, setup {elapsed:?}",plan.visited);
    }
    #[test]
    fn refreshes_ignore_rules_index_and_external_sources() {
        let folder=repository();let root=folder.path().canonicalize().unwrap();
        let external=tempfile::tempdir().unwrap();let exclude=external.path().join("global ignore");std::fs::write(&exclude,"ignored/\n").unwrap();
        assert!(crate::git::run(&root,&["config","core.excludesFile",&exclude.to_string_lossy()]).ok);
        std::fs::create_dir(root.join("ignored")).unwrap();std::fs::write(root.join("ignored/keep"),"data").unwrap();
        let before=WatchPlan::read(&root).unwrap();assert!(!before.directories.contains(&root.join("ignored")));
        assert!(before.configuration.contains(&exclude.canonicalize().unwrap()));
        assert!(crate::git::run(&root,&["add","-f","ignored/keep"]).ok);
        let after=WatchPlan::refresh(&root,&before).unwrap();assert!(after.directories.contains(&root.join("ignored")));
        assert!(crate::git::run(&root,&["rm","--cached","-f","ignored/keep"]).ok);
        let removed=WatchPlan::refresh(&root,&after).unwrap();assert!(!removed.directories.contains(&root.join("ignored")));
        std::fs::write(&exclude,"").unwrap();assert!(WatchPlan::read(&root).unwrap().directories.contains(&root.join("ignored")));
        assert!(!before.configuration_changed(&root.join(".git/index")));
    }
}


#[cfg(test)]
mod event_benchmark {
    use super::*;
    use notify_debouncer_mini::notify::{recommended_watcher, RecursiveMode, Watcher};
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    #[test]
    fn ignored_builds_do_not_enter_the_selective_os_watch_stream() {
        let folder=tempfile::tempdir().unwrap();let root=folder.path().canonicalize().unwrap();
        assert!(crate::git::run(&root,&["init","-q"]).ok);std::fs::write(root.join(".gitignore"),"build/\n").unwrap();
        std::fs::create_dir_all(root.join("build/generated/deep")).unwrap();std::fs::create_dir(root.join("source")).unwrap();
        let plan=WatchPlan::read(&root).unwrap();let recursive_count=Arc::new(AtomicUsize::new(0));let selective_count=Arc::new(AtomicUsize::new(0));
        let count=recursive_count.clone();let mut recursive=recommended_watcher(move |event:Result<notify_debouncer_mini::notify::Event,_>|{if let Ok(event)=event {count.fetch_add(event.paths.len(),Ordering::SeqCst);}}).unwrap();recursive.watch(&root,RecursiveMode::Recursive).unwrap();
        let count=selective_count.clone();let mut selective=recommended_watcher(move |event:Result<notify_debouncer_mini::notify::Event,_>|{if let Ok(event)=event {count.fetch_add(event.paths.len(),Ordering::SeqCst);}}).unwrap();
        for directory in &plan.directories {selective.watch(directory,RecursiveMode::NonRecursive).unwrap();}
        let start=std::time::Instant::now();for index in 0..500 {std::fs::write(root.join(format!("build/generated/deep/{index}.tmp")),"build output").unwrap();}
        std::thread::sleep(std::time::Duration::from_secs(2));
        eprintln!("Ignored 500-file build: recursive {} raw paths; selective {} raw paths; {} directory registrations; elapsed {:?}",recursive_count.load(Ordering::SeqCst),selective_count.load(Ordering::SeqCst),plan.directories.len(),start.elapsed());
        assert!(recursive_count.load(Ordering::SeqCst)>=500);assert_eq!(selective_count.load(Ordering::SeqCst),0);
    }
}
