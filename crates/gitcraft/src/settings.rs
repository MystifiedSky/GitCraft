//! Preferences, open tabs and drafts, kept in the `settings.json` GitCraft has always
//! used, so earlier installations keep their preferences.
//!
//! The document is held as JSON and only the keys this interface knows are read or changed:
//! everything else (drafts, profiles, preferences from other versions) is written back
//! exactly as it was read.
//!
//! A repository window runs as a process of its own, with its own copy. Each window saves only
//! what it changed, merged into the file as it is on disk, and reads the file again when
//! another window wrote it, so changes made in both windows are all kept.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use gitcraft_core::settings_file::Changes;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// Wait this long after the last change before writing, so typing doesn't write on every key.
const SAVE_AFTER: Duration = Duration::from_millis(400);
/// After a failed write, wait this long before trying again, so a full or read-only disk
/// doesn't bring a write and a notice every frame.
const RETRY_AFTER: Duration = Duration::from_secs(10);
/// How often to look whether another window changed the file.
const CHECK_EVERY: Duration = Duration::from_secs(1);

/// The zoom steps offered by View > Increase and Decrease Zoom (metrics `zoom_factors`).
pub const ZOOM_FACTORS: [f32; 12] = [3.0, 2.5, 2.0, 1.75, 1.5, 1.4, 1.3, 1.2, 1.1, 1.0, 0.9, 0.8];

pub struct Store {
    /// `None` for picture-taking runs, which must neither read nor change the user's file.
    path: Option<PathBuf>,
    doc: Value,
    /// When the unwritten changes are due to be written; `None` when everything is written.
    due: Option<Instant>,
    /// Bumped by every change, so others can tell the preferences moved.
    pub revision: u64,
    /// Shown once at start-up: what was recovered or could not be read.
    pub warning: Option<String>,
    /// False in a repository window of its own, whose tabs aren't the ones to reopen.
    pub shares_tabs: bool,
    /// What this window changed and has not written yet.
    changes: Changes,
    /// When the file was last written, as this window last read or wrote it.
    seen: Option<SystemTime>,
    /// When the file was last looked at for changes from another window.
    checked: Option<Instant>,
    /// Set when another window's changes were read in; see [`Store::take_reloaded`].
    reloaded: bool,
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub fn file() -> PathBuf {
    gitcraft_core::sample::data_dir().join("settings.json")
}

impl Store {
    pub fn open() -> Store {
        Self::open_at(file())
    }

    pub(crate) fn open_at(path: PathBuf) -> Store {
        let (doc, warning) = match gitcraft_core::settings_file::load(&path) {
            Ok(Value::Null) => (Value::Object(Map::new()), None),
            Ok(mut doc) => {
                let warning = doc.as_object_mut().and_then(|o| o.remove("recoveryWarning")).and_then(|w| w.as_str().map(str::to_owned));
                (doc, warning)
            }
            Err(e) => (Value::Object(Map::new()), Some(e)),
        };
        // A file that could not be read is left alone rather than overwritten with defaults.
        let path = if warning.as_deref().is_some_and(|w| w.starts_with("Could not read")) { None } else { Some(path) };
        let seen = path.as_deref().and_then(modified);
        Store { path, doc, due: None, revision: 1, warning, shares_tabs: true, changes: Changes::default(), seen, checked: None, reloaded: false }
    }

    /// Defaults only, saved nowhere.
    pub fn in_memory() -> Store {
        Store { path: None, doc: Value::Object(Map::new()), due: None, revision: 1, warning: None, shares_tabs: true, changes: Changes::default(), seen: None, checked: None, reloaded: false }
    }

    fn settings(&self) -> Option<&Map<String, Value>> {
        self.doc.get("settings")?.as_object()
    }

    /// A preference, or `None` when it is missing or of another type.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        serde_json::from_value(self.settings()?.get(key)?.clone()).ok()
    }

    pub fn set(&mut self, key: &str, value: impl Serialize) {
        let Ok(value) = serde_json::to_value(value) else { return };
        let root = object(&mut self.doc);
        let settings = object(root.entry("settings").or_insert_with(|| Value::Object(Map::new())));
        if settings.get(key) != Some(&value) {
            settings.insert(key.to_owned(), value.clone());
            self.changes.settings.insert(key.to_owned(), Some(value));
            self.touch();
        }
    }

    /// A number preference.
    pub fn num(&self, key: &str, default: f64) -> f64 {
        self.get::<f64>(key).filter(|n| n.is_finite()).unwrap_or(default)
    }

    /// A text preference.
    pub fn string(&self, key: &str, default: &str) -> String {
        self.get::<String>(key).unwrap_or_else(|| default.to_owned())
    }

    /// A key at the top of the document (beside `settings`), such as `drafts` or `profiles`.
    pub fn get_root<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        serde_json::from_value(self.doc.get(key)?.clone()).ok()
    }

    pub fn set_root(&mut self, key: &str, value: impl Serialize) {
        let Ok(value) = serde_json::to_value(value) else { return };
        let root = object(&mut self.doc);
        if root.get(key) != Some(&value) {
            root.insert(key.to_owned(), value.clone());
            self.changes.sections.insert(key.to_owned(), Some(value));
            self.touch();
        }
    }

    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.get(key).unwrap_or(default)
    }

    pub fn toggle(&mut self, key: &str, default: bool) {
        let now = !self.flag(key, default);
        self.set(key, now);
    }

    pub fn theme(&self) -> String {
        self.get("theme").unwrap_or_else(|| "system".into())
    }

    pub fn zoom(&self) -> f32 {
        self.get::<f32>("zoom").filter(|z| z.is_finite() && *z > 0.0).unwrap_or(1.0)
    }

    pub fn recents(&self) -> Vec<String> {
        self.get("recents").unwrap_or_default()
    }

    /// The alias the user gave a repository tab, if any.
    pub fn alias(&self, path: &str) -> Option<String> {
        self.get::<Map<String, Value>>("aliases")?.get(path)?.as_str().filter(|a| !a.is_empty()).map(str::to_owned)
    }

    pub fn set_alias(&mut self, path: &str, name: &str) {
        let mut aliases: Map<String, Value> = self.get("aliases").unwrap_or_default();
        let name = name.trim();
        if name.is_empty() {
            aliases.remove(path);
        } else {
            aliases.insert(path.to_owned(), Value::from(name));
        }
        self.set("aliases", aliases);
    }

    pub fn tab_width(&self, path: &str) -> Option<f32> {
        self.get::<Map<String, Value>>("tabWidths")?.get(path)?.as_f64().map(|w| w as f32)
    }

    pub fn set_tab_width(&mut self, path: &str, width: f32) {
        self.set_entry("tabWidths", path, Some(Value::from(width.round())));
    }

    /// Forget a dragged width, so the tab fits its title again.
    pub fn clear_tab_width(&mut self, path: &str) {
        self.set_entry("tabWidths", path, None);
    }

    /// Change one entry of a map preference, merged with other windows' entries when saved.
    fn set_entry(&mut self, key: &str, entry: &str, value: Option<Value>) {
        let root = object(&mut self.doc);
        let settings = object(root.entry("settings").or_insert_with(|| Value::Object(Map::new())));
        let map = object(settings.entry(key).or_insert_with(|| Value::Object(Map::new())));
        if map.get(entry) == value.as_ref() {
            return;
        }
        match &value {
            Some(value) => {
                map.insert(entry.to_owned(), value.clone());
            }
            None => {
                map.remove(entry);
            }
        }
        self.changes.entries.entry(key.to_owned()).or_default().insert(entry.to_owned(), value);
        self.touch();
    }

    fn profiles(&self) -> &[Value] {
        self.doc.get("profiles").and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }

    pub fn profile_names(&self) -> Vec<String> {
        self.profiles().iter().filter_map(|p| p.get("name")?.as_str().map(str::to_owned)).collect()
    }

    pub fn active_profile_index(&self) -> usize {
        let active = self.doc.get("activeProfile");
        self.profiles().iter().position(|p| p.get("id") == active).unwrap_or(0)
    }

    /// The name and icon number of the profile in use.
    pub fn active_profile(&self) -> (String, usize) {
        let profile = self.profiles().get(self.active_profile_index());
        let name = profile.and_then(|p| p.get("name")?.as_str()).unwrap_or("Default Profile").to_owned();
        let icon = profile.and_then(|p| p.get("icon")?.as_u64()).unwrap_or(0) as usize;
        (name, icon)
    }

    /// The repository tabs that were open last time, and which of them was in front.
    pub fn tabs(&self) -> (Vec<String>, usize) {
        let paths = self.doc.get("tabs").and_then(Value::as_array).map(|tabs| tabs.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        let active = self.doc.get("active").and_then(Value::as_u64).unwrap_or(0) as usize;
        (paths, active)
    }

    pub fn set_tabs(&mut self, paths: Vec<String>, active: usize) {
        if !self.shares_tabs {
            return;
        }
        let root = object(&mut self.doc);
        let tabs = Value::from(paths);
        let active = Value::from(active);
        if root.get("tabs") != Some(&tabs) || root.get("active") != Some(&active) {
            root.insert("tabs".into(), tabs.clone());
            root.insert("active".into(), active.clone());
            self.changes.sections.insert("tabs".into(), Some(tabs));
            self.changes.sections.insert("active".into(), Some(active));
            self.touch();
        }
    }

    fn touch(&mut self) {
        self.due = Some(Instant::now() + SAVE_AFTER);
        self.revision += 1;
    }

    /// The whole preferences object (for profiles and backups).
    pub fn settings_value(&self) -> Value {
        self.doc.get("settings").cloned().unwrap_or_else(|| Value::Object(Map::new()))
    }

    /// Replace all preferences (a profile switch): the whole object is written as it is.
    pub fn replace_settings(&mut self, value: Value) {
        object(&mut self.doc).insert("settings".into(), value.clone());
        self.changes.settings.clear();
        self.changes.entries.clear();
        self.changes.sections.insert("settings".into(), Some(value));
        self.touch();
    }

    /// Whether another window's changes were read in since the last call, so what is applied
    /// once (the theme, the zoom) can follow.
    pub fn take_reloaded(&mut self) -> bool {
        std::mem::take(&mut self.reloaded)
    }

    /// With nothing waiting to be written: read the file again if another window wrote it.
    fn reload_if_changed(&mut self) {
        let Some(path) = self.path.clone() else { return };
        if self.checked.is_some_and(|t| t.elapsed() < CHECK_EVERY) {
            return;
        }
        self.checked = Some(Instant::now());
        let now = modified(&path);
        if now == self.seen {
            return;
        }
        if let Ok(mut doc) = gitcraft_core::settings_file::load(&path)
            && let Some(root) = doc.as_object_mut()
        {
            root.remove("recoveryWarning");
            self.seen = now;
            if doc != self.doc {
                self.doc = doc;
                self.revision += 1;
                self.reloaded = true;
            }
        }
    }

    /// Once per frame: write a moment after the last change.
    pub fn save_when_due(&mut self, ctx: &egui::Context) -> Result<(), String> {
        let Some(due) = self.due else {
            self.reload_if_changed();
            return Ok(());
        };
        let now = Instant::now();
        if now < due {
            ctx.request_repaint_after(due - now);
            return Ok(());
        }
        // Waiting briefly: a lock another window holds is tried again later, quietly.
        let result = match self.flush_within(Some(std::time::Duration::from_millis(50))) {
            Err(e) if e == gitcraft_core::settings_file::LOCK_BUSY => Ok(()),
            other => other,
        };
        // Still pending (a busy lock or a failed write): come back for it even if nothing else repaints.
        if let Some(due) = self.due {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()).max(std::time::Duration::from_millis(100)));
        }
        result
    }

    /// Write what has not been written yet (before quitting or restarting), waiting a few
    /// seconds at most for another window's lock.
    pub fn flush(&mut self) -> Result<(), String> {
        self.flush_within(Some(std::time::Duration::from_secs(3)))
    }

    /// Write what has not been written yet. A failed write stays pending and is tried again
    /// a little later.
    fn flush_within(&mut self, wait: Option<std::time::Duration>) -> Result<(), String> {
        if self.due.is_none() {
            return Ok(());
        }
        let Some(path) = self.path.clone() else {
            self.changes = Changes::default();
            self.due = None;
            return Ok(());
        };
        match gitcraft_core::settings_file::save_changes_within(&path, &self.changes, wait) {
            Ok(merged) => {
                self.changes = Changes::default();
                self.due = None;
                self.seen = modified(&path);
                // What other windows saved meanwhile is now part of this copy too.
                if merged != self.doc {
                    self.doc = merged;
                    self.revision += 1;
                    self.reloaded = true;
                }
                Ok(())
            }
            Err(e) => {
                // Another window's save is brief: try again soon.
                let after = if e == gitcraft_core::settings_file::LOCK_BUSY { Duration::from_millis(250) } else { RETRY_AFTER };
                self.due = Some(Instant::now() + after);
                Err(e)
            }
        }
    }
}

/// The object inside `value`, replacing anything that is not an object.
fn object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("just made an object")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_keys_survive_a_change() {
        let mut store = Store::in_memory();
        store.doc = serde_json::json!({ "settings": { "zoom": 1.2, "externalEditor": "code" }, "drafts": { "/r": { "summary": "wip" } }, "profiles": [] });
        store.set("zoom", 1.5);
        store.set_tabs(vec!["/r".into()], 0);
        assert_eq!(store.doc["settings"]["externalEditor"], "code");
        assert_eq!(store.doc["drafts"]["/r"]["summary"], "wip");
        assert_eq!(store.zoom(), 1.5);
        assert_eq!(store.tabs(), (vec!["/r".to_owned()], 0));
    }

    #[test]
    fn missing_or_wrong_values_fall_back_to_the_defaults() {
        let mut store = Store::in_memory();
        assert_eq!(store.theme(), "system");
        assert_eq!(store.zoom(), 1.0);
        assert!(store.flag("showLeft", true));
        store.doc = serde_json::json!({ "settings": { "zoom": "large", "showLeft": 3 } });
        assert_eq!(store.zoom(), 1.0);
        assert!(store.flag("showLeft", true));
    }

    #[test]
    fn setting_the_same_value_is_not_a_change() {
        let mut store = Store::in_memory();
        store.set("theme", "dark");
        store.due = None;
        store.set("theme", "dark");
        assert!(store.due.is_none());
    }

    #[test]
    fn a_failed_write_stays_pending_and_waits_before_retrying() {
        let dir = std::env::temp_dir().join(format!("gitcraft-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A folder where the file should be: the write fails.
        let path = dir.join("settings.json");
        std::fs::create_dir(&path).unwrap();
        let mut store = Store::in_memory();
        store.path = Some(path.clone());
        store.set("theme", "dark");
        assert!(store.flush().is_err());
        let due = store.due.expect("still pending");
        assert!(due >= Instant::now() + RETRY_AFTER - Duration::from_secs(1));
        std::fs::remove_dir(&path).unwrap();
        assert!(store.flush().is_ok());
        assert!(store.due.is_none());
        assert!(std::fs::read_to_string(&path).unwrap().contains("dark"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_windows_keep_and_see_each_others_changes() {
        let dir = std::env::temp_dir().join(format!("gitcraft-two-windows-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let window = || {
            let mut store = Store::in_memory();
            store.path = Some(path.clone());
            store
        };
        let (mut main, mut repo) = (window(), window());
        repo.shares_tabs = false;
        main.set("zoom", 1.5);
        main.set_tabs(vec!["/a".into()], 0);
        main.set_tab_width("/a", 120.0);
        repo.set("theme", "light");
        repo.set_tabs(vec!["/b".into()], 0);
        repo.set_tab_width("/b", 140.0);
        main.flush().unwrap();
        repo.flush().unwrap();
        // The repository window wrote last and kept the main window's zoom, tabs and tab width.
        let disk: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(disk["settings"]["zoom"], 1.5);
        assert_eq!(disk["settings"]["theme"], "light");
        assert_eq!(disk["tabs"], serde_json::json!(["/a"]));
        assert_eq!(disk["settings"]["tabWidths"], serde_json::json!({"/a": 120.0, "/b": 140.0}));
        main.clear_tab_width("/a");
        repo.set_tab_width("/b", 150.0);
        main.flush().unwrap();
        repo.flush().unwrap();
        let disk: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(disk["settings"]["tabWidths"], serde_json::json!({"/b": 150.0}));
        assert_eq!(repo.zoom(), 1.5);
        // The main window reads the theme in when it next looks.
        main.checked = None;
        main.seen = None;
        main.reload_if_changed();
        assert_eq!(main.theme(), "light");
        assert!(main.take_reloaded());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
