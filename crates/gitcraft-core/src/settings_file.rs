use serde_json::Value;
use std::{fs, io::{self, Write}, path::Path, sync::Mutex};

static ACCESS: Mutex<()> = Mutex::new(());

fn parse(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(bytes).ok().filter(Value::is_object)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| io::Error::other("Settings path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut pending = tempfile::NamedTempFile::new_in(parent)?;
    pending.write_all(bytes)?;
    pending.as_file().sync_all()?;
    pending.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Value, String> {
    let _guard = ACCESS.lock().unwrap_or_else(|error| error.into_inner());
    let warning = match fs::read(path) {
        Ok(bytes) => {
            if let Some(value) = parse(&bytes) { return Ok(value); }
            let parent = path.parent().ok_or("Settings path has no parent")?;
            let mut quarantined = tempfile::Builder::new().prefix("settings.corrupt-").suffix(".json").tempfile_in(parent).map_err(|error| error.to_string())?;
            quarantined.write_all(&bytes).and_then(|_| quarantined.as_file().sync_all()).map_err(|error| error.to_string())?;
            let (_, saved) = quarantined.keep().map_err(|error| error.to_string())?;
            fs::remove_file(path).map_err(|error| error.to_string())?;
            Some(format!("Invalid preferences were preserved in {}.", saved.display()))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Could not read {}: {error}. The existing preferences were kept.", path.display())),
    };
    let backup = path.with_extension("json.bak");
    if let Some(mut recovered) = fs::read(backup).ok().and_then(|data| parse(&data)) {
        recovered["recoveryWarning"] = format!("{}Restored preferences from the last readable backup.", warning.map(|value| format!("{value} ")).unwrap_or_default()).into();
        // Restore the primary immediately so the next launch sees clean preferences.
        let mut clean = recovered.clone(); clean.as_object_mut().expect("validated JSON object").remove("recoveryWarning");
        write_atomic(path, &serde_json::to_vec_pretty(&clean).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        return Ok(recovered);
    }
    Ok(warning.map(|warning| serde_json::json!({"recoveryWarning": format!("{warning} No readable backup was available.")})).unwrap_or(Value::Null))
}

pub fn save(path: &Path, settings: &Value) -> Result<(), String> {
    let _guard = ACCESS.lock().unwrap_or_else(|error| error.into_inner());
    if !settings.is_object() { return Err("Preferences must be a JSON object.".into()); }
    match fs::read(path) {
        Ok(bytes) if parse(&bytes).is_some() => write_atomic(&path.with_extension("json.bak"), &bytes).map_err(|error| error.to_string())?,
        Ok(_) => return Err("Preferences on disk are invalid. Restart to recover them before saving.".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.to_string()),
    }
    write_atomic(path, &serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

/// What one window changed since it last saved: preferences by key, entries of map preferences
/// changed one at a time, and whole top-level sections (`tabs`, `drafts`, `profiles`, or all of
/// `settings` when a profile is switched). `None` removes the key or entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub settings: std::collections::BTreeMap<String, Option<Value>>,
    pub entries: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Option<Value>>>,
    pub sections: std::collections::BTreeMap<String, Option<Value>>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty() && self.entries.is_empty() && self.sections.is_empty()
    }

    /// `doc` with these changes made: sections first, then the preferences on top.
    pub fn apply(&self, doc: &mut Value) {
        if !doc.is_object() {
            *doc = Value::Object(serde_json::Map::new());
        }
        let root = doc.as_object_mut().expect("just made an object");
        for (key, value) in &self.sections {
            match value {
                Some(value) => root.insert(key.clone(), value.clone()),
                None => root.remove(key),
            };
        }
        if self.settings.is_empty() && self.entries.is_empty() {
            return;
        }
        let settings = root.entry("settings").or_insert_with(|| Value::Object(serde_json::Map::new()));
        if !settings.is_object() {
            *settings = Value::Object(serde_json::Map::new());
        }
        let settings = settings.as_object_mut().expect("just made an object");
        for (key, value) in &self.settings {
            match value {
                Some(value) => settings.insert(key.clone(), value.clone()),
                None => settings.remove(key),
            };
        }
        for (key, entries) in &self.entries {
            let value = settings.entry(key.clone()).or_insert_with(|| Value::Object(serde_json::Map::new()));
            if !value.is_object() {
                *value = Value::Object(serde_json::Map::new());
            }
            let map = value.as_object_mut().expect("just made an object");
            for (entry, value) in entries {
                match value {
                    Some(value) => map.insert(entry.clone(), value.clone()),
                    None => map.remove(entry),
                };
            }
            if map.is_empty() {
                settings.remove(key);
            }
        }
    }
}

/// What [`save_changes_within`] returns when another window held the lock all the while.
pub const LOCK_BUSY: &str = "The preferences are being saved by another GitCraft window.";

/// Write `changes` into the preferences file as it is now on disk, so what another GitCraft
/// window saved in the meantime is kept, and return the merged document. Other processes
/// wait on the lock file beside it while this one reads and writes.
pub fn save_changes(path: &Path, changes: &Changes) -> Result<Value, String> {
    save_changes_within(path, changes, None)
}

/// [`save_changes`], waiting at most `wait` for the lock (`None`: as long as it takes), so a
/// window that saves while another one holds the lock stays responsive and tries again later.
/// Gives [`LOCK_BUSY`] when the wait ran out.
pub fn save_changes_within(path: &Path, changes: &Changes, wait: Option<std::time::Duration>) -> Result<Value, String> {
    let _guard = ACCESS.lock().unwrap_or_else(|error| error.into_inner());
    let parent = path.parent().ok_or("Settings path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("json.lock")).map_err(|error| error.to_string())?;
    match wait {
        None => lock.lock().map_err(|error| format!("Could not lock the preferences: {error}"))?,
        Some(wait) => {
            let until = std::time::Instant::now() + wait;
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(fs::TryLockError::WouldBlock) if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(5)),
                    Err(fs::TryLockError::WouldBlock) => return Err(LOCK_BUSY.into()),
                    Err(fs::TryLockError::Error(error)) => return Err(format!("Could not lock the preferences: {error}")),
                }
            }
        }
    }
    let mut doc = match fs::read(path) {
        Ok(bytes) => match parse(&bytes) {
            Some(doc) => {
                write_atomic(&path.with_extension("json.bak"), &bytes).map_err(|error| error.to_string())?;
                doc
            }
            None => return Err("Preferences on disk are invalid. Restart to recover them before saving.".into()),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Value::Object(serde_json::Map::new()),
        Err(error) => return Err(error.to_string()),
    };
    changes.apply(&mut doc);
    write_atomic(path, &serde_json::to_vec_pretty(&doc).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_primary_is_preserved_and_the_last_backup_is_restored() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let first = serde_json::json!({"settings": {"zoom": 1.25}});
        save(&path, &first).unwrap();
        save(&path, &serde_json::json!({"settings": {"zoom": 2}})).unwrap();
        fs::write(&path, b"damaged preferences").unwrap();
        assert!(save(&path, &first).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"damaged preferences");
        let recovered = load(&path).unwrap();
        assert_eq!(recovered["settings"], first["settings"]);
        assert!(recovered["recoveryWarning"].as_str().unwrap().contains("Restored"));
        let quarantined = fs::read_dir(temp.path()).unwrap().flatten().find(|entry| entry.file_name().to_string_lossy().starts_with("settings.corrupt-")).unwrap();
        assert_eq!(fs::read(quarantined.path()).unwrap(), b"damaged preferences");
        assert_eq!(load(&path).unwrap(), first);
    }

    #[test]
    fn a_held_lock_makes_a_bounded_save_give_up_quickly() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let mut changes = Changes::default();
        changes.settings.insert("zoom".into(), Some(serde_json::json!(1.5)));
        // Another window holds the lock (a separate open file, as another process would).
        let held = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("json.lock")).unwrap();
        held.lock().unwrap();
        let started = std::time::Instant::now();
        assert_eq!(save_changes_within(&path, &changes, Some(std::time::Duration::from_millis(50))).unwrap_err(), LOCK_BUSY);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(!path.exists(), "nothing written without the lock");
        held.unlock().unwrap();
        assert_eq!(save_changes_within(&path, &changes, Some(std::time::Duration::from_millis(50))).unwrap()["settings"]["zoom"], 1.5);
    }

    #[test]
    fn two_windows_keep_each_others_changes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        save(&path, &serde_json::json!({"settings": {"zoom": 1, "theme": "dark"}, "tabs": ["/a"], "drafts": {"x": 1}})).unwrap();
        // One window changes the zoom and its tabs, the other the theme, a new key and removes one.
        let mut first = Changes::default();
        first.settings.insert("zoom".into(), Some(serde_json::json!(1.5)));
        first.sections.insert("tabs".into(), Some(serde_json::json!(["/a", "/b"])));
        save_changes(&path, &first).unwrap();
        let mut second = Changes::default();
        second.settings.insert("theme".into(), Some(serde_json::json!("light")));
        second.settings.insert("showLeft".into(), Some(serde_json::json!(false)));
        second.sections.insert("drafts".into(), None);
        let merged = save_changes(&path, &second).unwrap();
        let want = serde_json::json!({"settings": {"zoom": 1.5, "theme": "light", "showLeft": false}, "tabs": ["/a", "/b"]});
        assert_eq!(merged, want);
        assert_eq!(load(&path).unwrap(), want);
        // A whole-settings replacement (a profile switch) wins over the file, and keys set after it on top.
        let mut third = Changes::default();
        third.sections.insert("settings".into(), Some(serde_json::json!({"theme": "dark"})));
        third.settings.insert("zoom".into(), Some(serde_json::json!(2)));
        assert_eq!(save_changes(&path, &third).unwrap()["settings"], serde_json::json!({"theme": "dark", "zoom": 2}));
    }

    #[test]
    fn entries_of_a_map_setting_merge_across_windows() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        save(&path, &serde_json::json!({"settings": {"tabWidths": {"/a": 100}}})).unwrap();
        let mut first = Changes::default();
        first.entries.entry("tabWidths".into()).or_default().insert("/b".into(), Some(serde_json::json!(200)));
        assert_eq!(save_changes(&path, &first).unwrap()["settings"]["tabWidths"], serde_json::json!({"/a": 100, "/b": 200}));
        let mut second = Changes::default();
        second.entries.entry("tabWidths".into()).or_default().insert("/a".into(), None);
        let merged = save_changes(&path, &second).unwrap();
        assert_eq!(merged["settings"]["tabWidths"], serde_json::json!({"/b": 200}));
        let mut third = Changes::default();
        third.entries.entry("tabWidths".into()).or_default().insert("/b".into(), None);
        assert_eq!(save_changes(&path, &third).unwrap()["settings"].get("tabWidths"), None);
    }
}
