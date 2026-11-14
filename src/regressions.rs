//! Repository fixtures for history, file safety and conflict regressions.
use crate::{actions::{Action, Engine}, git};
use std::path::Path;

fn fixture(label: &str) -> tempfile::TempDir {
    let folder = tempfile::Builder::new().prefix(label).tempdir().unwrap();
    for args in [vec!["init", "-q", "-b", "main"], vec!["config", "user.name", "Test"], vec!["config", "user.email", "test@example.com"], vec!["config", "commit.gpgsign", "false"]] {
        assert!(git::run(folder.path(), &args).ok);
    }
    std::fs::write(folder.path().join("file.txt"), "base\n").unwrap();
    assert!(git::run(folder.path(), &["add", "file.txt"]).ok);
    assert!(git::run(folder.path(), &["commit", "-qm", "base"]).ok);
    folder
}
fn head(path: &Path) -> String { git::run(path, &["rev-parse", "HEAD"]).stdout.trim().to_owned() }

#[test]
fn undo_and_redo_refuse_unrelated_branches_and_external_commits() {
    let folder = fixture("gitcraft-undo-"); let path = folder.path(); let repo = path.to_string_lossy(); let engine = Engine::default();
    let original = head(path);
    std::fs::write(path.join("file.txt"), "app commit\n").unwrap();
    assert!(engine.perform(&repo, Action::Commit { summary: "app".into(), description: "".into(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
    let app_commit = head(path);
    assert!(git::run(path, &["checkout", "-qb", "other"]).ok);
    assert!(git::run(path, &["commit", "--allow-empty", "-qm", "external"]).ok);
    let other = head(path);
    assert!(!engine.perform(&repo, Action::Undo).ok);
    assert_eq!(head(path), other);
    assert!(git::run(path, &["checkout", "-q", "main"]).ok);
    assert!(engine.perform(&repo, Action::Undo).ok);
    assert_eq!(head(path), original);
    assert!(engine.perform(&repo, Action::Redo).ok);
    assert_eq!(head(path), app_commit);
    assert!(git::run(path, &["commit", "--allow-empty", "-qm", "external on main"]).ok);
    let external = head(path);
    assert!(!engine.perform(&repo, Action::Undo).ok);
    assert_eq!(head(path), external);
}

#[test]
fn stashed_file_refuses_dirty_worktree_and_index_but_applies_to_clean_file() {
    let folder = fixture("gitcraft-stash-safety-"); let path = folder.path(); let repo = path.to_string_lossy(); let engine = Engine::default();
    std::fs::write(path.join("file.txt"), "stashed\n").unwrap();
    assert!(git::run(path, &["stash", "push", "-qm", "saved"]).ok);
    std::fs::write(path.join("file.txt"), "new work\n").unwrap();
    let apply = || engine.perform(&repo, Action::StashTakeFile { index: 0, path: "file.txt".into() });
    assert!(!apply().ok);
    assert_eq!(std::fs::read_to_string(path.join("file.txt")).unwrap(), "new work\n");
    assert!(git::run(path, &["add", "file.txt"]).ok);
    assert!(!apply().ok);
    assert!(git::run(path, &["commit", "-qm", "save newer work"]).ok);
    assert!(apply().ok);
    assert_eq!(std::fs::read_to_string(path.join("file.txt")).unwrap().replace("\r\n", "\n"), "stashed\n");
}

#[test]
fn clearing_or_matching_push_url_removes_an_old_override() {
    let folder = fixture("gitcraft-remote-edit-"); let path = folder.path(); let repo = path.to_string_lossy(); let engine = Engine::default();
    let fetch = "https://example.test/new.git";
    assert!(git::run(path, &["remote", "add", "origin", fetch]).ok);
    for replacement in ["", fetch] {
        assert!(git::run(path, &["remote", "set-url", "--push", "origin", "https://example.test/old.git"]).ok);
        let result = engine.perform(&repo, Action::RemoteEdit { name: "origin".into(), new_name: "origin".into(), url: fetch.into(), push_url: replacement.into() });
        assert!(result.ok, "{}", result.message);
        assert_eq!(git::run(path, &["remote", "get-url", "--push", "origin"]).stdout.trim(), fetch);
    }
}

#[test]
fn interactive_reword_supports_apostrophes_in_repository_paths() {
    let folder = fixture("gitcraft-O'Connor-"); let path = folder.path();
    let result = crate::rewrite::interactive_rebase(path, "--root", &[crate::rewrite::TodoItem { action: crate::rewrite::Step::Reword, sha: head(path), message: Some("updated".into()) }]);
    assert!(result.ok, "{}", result.message());
    assert_eq!(git::run(path, &["log", "-1", "--format=%s"]).stdout.trim(), "updated");
}

#[test]
fn conflict_reader_and_resolution_round_trip_the_repository_encoding() {
    let folder = fixture("gitcraft-conflict-encoding-"); let path = folder.path();
    let original = crate::prefs::runtime();
    let mut settings = original.clone();
    settings.encodings.insert(path.to_string_lossy().into_owned(), "windows-1252".into());
    crate::prefs::set_runtime(settings);
    std::fs::write(path.join("file.txt"), b"<<<<<<< HEAD\ncaf\xe9\n=======\nother\n>>>>>>> incoming\n").unwrap();
    let info = crate::rewrite::conflict_info(path, "file.txt");
    let readable = info.working.as_deref().is_some_and(|text| text.contains("café"));
    let save = crate::files::save(path, "file.txt", "café\n");
    let bytes = std::fs::read(path.join("file.txt")).unwrap();
    crate::prefs::set_runtime(original);
    assert!(readable);
    assert!(save.is_ok());
    assert_eq!(bytes, b"caf\xe9\n");
}

#[cfg(target_os = "linux")]
#[test]
fn glib_variant_string_iterator_uses_a_mutable_ffi_out_pointer() {
    use glib::variant::ToVariant;
    let values = ["alpha", "café", "", "omega"];
    let variant = glib::Variant::array_from_iter::<String>(values.map(|value| value.to_variant()));
    for _ in 0..100 {
        assert_eq!(variant.array_iter_str().unwrap().collect::<Vec<_>>(), values);
        assert_eq!(variant.array_iter_str().unwrap().next(), Some("alpha"));
        assert_eq!(variant.array_iter_str().unwrap().next_back(), Some("omega"));
        assert_eq!(variant.array_iter_str().unwrap().nth(1), Some("café"));
        assert_eq!(variant.array_iter_str().unwrap().nth_back(1), Some(""));
        assert_eq!(variant.array_iter_str().unwrap().last(), Some("omega"));
        let mut mixed = variant.array_iter_str().unwrap();
        assert_eq!(mixed.next(), Some("alpha"));
        assert_eq!(mixed.next_back(), Some("omega"));
        assert_eq!(mixed.next_back(), Some(""));
        assert_eq!(mixed.next(), Some("café"));
        assert_eq!(mixed.next(), None);
    }
}
