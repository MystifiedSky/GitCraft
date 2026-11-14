//! Repository fixtures for history, file safety and conflict regressions.
use crate::{actions::{Action, Engine}, git};
use std::path::Path;

fn fixture(label: &str) -> tempfile::TempDir {
    let folder = tempfile::Builder::new().prefix(label).tempdir().unwrap();
    for args in [vec!["init", "-q", "-b", "main"], vec!["config", "core.autocrlf", "false"], vec!["config", "user.name", "Test"], vec!["config", "user.email", "test@example.com"], vec!["config", "commit.gpgsign", "false"]] {
        assert!(git::run(folder.path(), &args).ok);
    }
    std::fs::write(folder.path().join("file.txt"), "base\n").unwrap();
    assert!(git::run(folder.path(), &["add", "file.txt"]).ok);
    assert!(git::run(folder.path(), &["commit", "-qm", "base"]).ok);
    folder
}
fn head(path: &Path) -> String { git::run(path, &["rev-parse", "HEAD"]).stdout.trim().to_owned() }

#[test]
fn discard_without_new_changes_preserves_an_existing_stash() {
    let folder = fixture("gitcraft-discard-stash-"); let path = folder.path();
    std::fs::write(path.join("file.txt"), "saved work\n").unwrap();
    assert!(git::run(path, &["stash", "push", "-qm", "keep this stash"]).ok);
    let saved = git::run(path, &["rev-parse", "stash@{0}"]).stdout;
    let engine = Engine::default();
    assert!(!engine.perform(&path.to_string_lossy(), Action::DiscardAll).ok);
    assert_eq!(git::run(path, &["rev-parse", "stash@{0}"]).stdout, saved);
    assert!(engine.undo_info(&path.to_string_lossy()).undo.is_none());
}

#[test]
fn editor_saves_refuse_newer_disk_contents_and_deletions() {
    let folder = fixture("gitcraft-editor-version-"); let path = folder.path();
    let before = std::fs::read(path.join("file.txt")).unwrap();
    let token = crate::files::content_token(Some(&before));
    std::fs::write(path.join("file.txt"), "external change\n").unwrap();
    assert!(crate::files::save_checked(path, "file.txt", "stale draft", Some(&token)).is_err());
    assert_eq!(std::fs::read_to_string(path.join("file.txt")).unwrap(), "external change\n");
    std::fs::remove_file(path.join("file.txt")).unwrap();
    assert!(crate::files::save_checked(path, "file.txt", "stale draft", Some(&token)).is_err());
    assert!(!path.join("file.txt").exists());
}

#[test]
fn rename_previews_keep_the_original_side_and_rename_patch() {
    let folder = fixture("gitcraft-rename-preview-"); let path = folder.path();
    let parent = head(path);
    assert!(git::run(path, &["mv", "file.txt", "renamed.txt"]).ok);
    assert!(git::run(path, &["config", "diff.renames", "false"]).ok);
    assert!(git::diff_text(path, &git::DiffSource::Staged, "renamed.txt", None, false).contains("rename from file.txt"));
    let staged = crate::files::versions(path, &git::DiffSource::Staged, "renamed.txt", None, false);
    assert_eq!(staged.old.unwrap().replace("\r\n", "\n"), "base\n");
    assert!(git::run(path, &["commit", "-qm", "rename"]).ok);
    let source = git::DiffSource::Commit(head(path));
    let versions = crate::files::versions(path, &source, "renamed.txt", Some(&parent), false);
    assert_eq!(versions.old.unwrap().replace("\r\n", "\n"), "base\n");
    assert!(git::diff_text(path, &source, "renamed.txt", Some(&parent), false).contains("rename from file.txt"));
}

#[test]
fn corrupt_status_is_an_error_instead_of_a_clean_snapshot() {
    let folder = fixture("gitcraft-status-error-"); let path = folder.path();
    std::fs::write(path.join(".git/index"), b"bad-index").unwrap();
    assert!(git::try_status(path).is_err());
    assert!(git::load_view_checked(path, 10, &Default::default()).is_err());
}

#[test]
fn partially_created_composed_commits_remain_undoable() {
    let folder = fixture("gitcraft-compose-failure-"); let path = folder.path();
    let before = head(path);
    std::fs::write(path.join("one.txt"), "first\n").unwrap();
    std::fs::write(path.join("two.txt"), "second\n").unwrap();
    let hook = path.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nif git diff --cached --name-only | grep -q two.txt; then exit 1; fi\n").unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let engine = Engine::default(); let repo = path.to_string_lossy();
    let commits = ["one.txt", "two.txt"].map(|file| crate::ai::ComposedCommit { summary: file.into(), description: String::new(), files: vec![file.into()] }).to_vec();
    assert!(!engine.perform(&repo, Action::ComposeCommits { commits }).ok);
    assert_ne!(head(path), before);
    assert!(engine.undo_info(&repo).undo.is_some());
    assert!(engine.perform(&repo, Action::Undo).ok);
    assert_eq!(head(path), before);
    assert!(path.join("one.txt").exists() && path.join("two.txt").exists());
}

#[test]
fn initial_composition_separates_staged_files_and_supports_undo_redo() {
    let folder = fixture("gitcraft-initial-compose-"); let path = folder.path();
    assert!(git::run(path, &["rm", "-q", "file.txt"]).ok);
    assert!(git::run(path, &["update-ref", "-d", "refs/heads/main"]).ok);
    for name in ["one.txt", "two.txt"] { std::fs::write(path.join(name), name).unwrap(); }
    assert!(git::run(path, &["add", "."]).ok);
    let commits = ["one.txt", "two.txt"].map(|file| crate::ai::ComposedCommit { summary: file.into(), description: String::new(), files: vec![file.into()] }).to_vec();
    let engine = Engine::default(); let repo = path.to_string_lossy();
    let result = engine.perform(&repo, Action::ComposeCommits { commits });
    assert!(result.ok, "{}", result.message);
    assert_eq!(git::run(path, &["ls-tree", "--name-only", "HEAD~1"]).stdout.trim(), "one.txt");
    let after = head(path);
    assert!(engine.perform(&repo, Action::Undo).ok);
    assert!(!git::run(path, &["rev-parse", "--verify", "HEAD"]).ok);
    assert!(path.join("one.txt").exists() && path.join("two.txt").exists());
    assert!(engine.perform(&repo, Action::Redo).ok);
    assert_eq!(head(path), after);
}

#[test]
fn backend_mutations_wait_for_the_checkout_operation_lock() {
    use std::sync::mpsc;
    let folder = fixture("gitcraft-operation-queue-"); let path = folder.path();
    std::fs::write(path.join("file.txt"), "changed\n").unwrap();
    let operation = crate::operation::lock_for(path);
    let guard = operation.lock().unwrap();
    let repo = path.to_string_lossy().to_string();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || { tx.send(Engine::default().perform(&repo, Action::StageAll)).unwrap(); });
    assert!(rx.recv_timeout(std::time::Duration::from_millis(100)).is_err());
    drop(guard);
    assert!(rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap().ok);
    worker.join().unwrap();
}

#[test]
fn large_ref_lists_are_drained_without_a_pipe_deadlock() {
    let folder = fixture("gitcraft-many-refs-"); let path = folder.path();
    let sha = head(path);
    let refs = path.join(".git/refs/tags"); std::fs::create_dir_all(&refs).unwrap();
    for i in 0..3000 { std::fs::write(refs.join(format!("tag-{i:05}")), format!("{sha}\n")).unwrap(); }
    assert_eq!(git::walkable_refs(path).len(), 3001);
}

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
    let apply = || engine.perform(&repo, Action::StashTakeFile { index: 0, path: "file.txt".into(), expected: None });
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


#[test]
fn discard_redo_preserves_scope_and_refuses_intervening_edits() {
    let f = fixture("gitcraft-discard-redo-"); let d=f.path(); let path=d.to_string_lossy(); let e=Engine::default();
    std::fs::write(d.join("file.txt"), "selected\n").unwrap();
    std::fs::write(d.join("unrelated.txt"), "untracked\n").unwrap();
    assert!(e.perform(&path,Action::Discard { paths:vec!["file.txt".into()] }).ok);
    for _ in 0..2 {
        assert!(e.perform(&path,Action::Undo).ok);
        assert_eq!(std::fs::read_to_string(d.join("file.txt")).unwrap(),"selected\n");
        assert!(e.perform(&path,Action::Redo).ok);
        assert_eq!(std::fs::read_to_string(d.join("unrelated.txt")).unwrap(),"untracked\n");
    }
    assert!(e.perform(&path,Action::Undo).ok);
    std::fs::write(d.join("file.txt"), "new edit\n").unwrap();
    assert!(!e.perform(&path,Action::Redo).ok);
    assert_eq!(std::fs::read_to_string(d.join("file.txt")).unwrap(),"new edit\n");
}

#[test]
fn stale_hunks_and_user_prefixes_cannot_target_another_change() {
    let f=fixture("gitcraft-hunk-revision-"); let d=f.path();
    std::fs::create_dir(d.join("dir")).unwrap();
    std::fs::write(d.join("dir/file.txt"),"base\n").unwrap();
    assert!(git::run(d,&["add","."]).ok); assert!(git::run(d,&["commit","-qm","nested"]).ok);
    assert!(git::run(d,&["config","diff.noprefix","true"]).ok);
    std::fs::write(d.join("dir/file.txt"),"change\n").unwrap();
    let shown=git::file_diff_opts(d,&git::DiffSource::Unstaged,"dir/file.txt",false,None,false);
    std::fs::write(d.join("dir/file.txt"),"different\n").unwrap();
    assert!(crate::files::apply_part_checked(d,"dir/file.txt",false,crate::files::PatchOp::Discard,0,None,&shown).is_err());
    assert_eq!(std::fs::read_to_string(d.join("dir/file.txt")).unwrap(),"different\n");
    let current=git::file_diff_opts(d,&git::DiffSource::Unstaged,"dir/file.txt",false,None,false);
    crate::files::apply_part_checked(d,"dir/file.txt",false,crate::files::PatchOp::Stage,0,None,&current).unwrap();
    assert_eq!(git::run(d,&["diff","--cached","--name-only"]).stdout.trim(),"dir/file.txt");
    crate::files::apply_part(d,"dir/file.txt",false,crate::files::PatchOp::Unstage,0,None).unwrap();
    assert!(git::run(d,&["diff","--cached","--name-only"]).stdout.trim().is_empty());
    crate::files::apply_part(d,"dir/file.txt",false,crate::files::PatchOp::Discard,0,None).unwrap();
    assert_eq!(std::fs::read_to_string(d.join("file.txt")).unwrap(),"base\n");
}

#[test]
fn unborn_unstage_keeps_newer_working_bytes_and_deleted_files() {
    let f=tempfile::tempdir().unwrap();let d=f.path();assert!(git::run(d,&["init","-q"]).ok);
    std::fs::write(d.join("new.txt"),"v1\n").unwrap();assert!(git::run(d,&["add","."]).ok);
    std::fs::write(d.join("new.txt"),"v2\n").unwrap();let e=Engine::default();
    assert!(e.perform(&d.to_string_lossy(),Action::Unstage {paths:vec!["new.txt".into()]}).ok);
    assert_eq!(std::fs::read_to_string(d.join("new.txt")).unwrap(),"v2\n");
    assert!(git::run(d,&["add","."]).ok);std::fs::remove_file(d.join("new.txt")).unwrap();
    assert!(e.perform(&d.to_string_lossy(),Action::UnstageAll).ok);
    assert!(git::run(d,&["ls-files"]).stdout.is_empty());assert!(!d.join("new.txt").exists());
}

#[test]
fn stash_actions_follow_the_selected_object_after_an_external_push() {
    let f=fixture("gitcraft-stash-object-");let d=f.path();
    std::fs::write(d.join("file.txt"),"older\n").unwrap();assert!(git::run(d,&["stash","push","-qm","older"]).ok);
    let expected=git::run(d,&["rev-parse","stash@{0}"]).stdout.trim().to_owned();
    std::fs::write(d.join("file.txt"),"newer\n").unwrap();assert!(git::run(d,&["stash","push","-qm","newer"]).ok);
    let newer=git::run(d,&["rev-parse","stash@{0}"]).stdout;
    let e=Engine::default();assert!(e.perform(&d.to_string_lossy(),Action::StashDrop {index:0,expected:Some(expected.clone())}).ok);
    assert_eq!(git::run(d,&["rev-parse","stash@{0}"]).stdout,newer);
    assert!(!e.perform(&d.to_string_lossy(),Action::StashDrop {index:0,expected:Some(expected)}).ok);
}

#[test]
fn initialization_refuses_existing_files_and_commits() {
    let f=fixture("gitcraft-init-existing-");let d=f.path();let old=head(d);
    let opts=crate::manage::InitOptions {readme:true,..Default::default()};
    assert!(!crate::manage::init_files(d,"existing",&opts,&|a|git::run(d,a)).ok);
    assert_eq!(head(d),old);assert!(!d.join("README.md").exists());
    let empty=tempfile::tempdir().unwrap();let d=empty.path();assert!(git::run(d,&["init","-q"]).ok);
    std::fs::write(d.join("README.md"),"keep me").unwrap();
    assert!(!crate::manage::init_files(d,"existing",&opts,&|a|git::run(d,a)).ok);
    assert_eq!(std::fs::read_to_string(d.join("README.md")).unwrap(),"keep me");
}

#[test]
fn legacy_encoders_refuse_lossy_substitution() {
    assert!(crate::prefs::encode("emoji 😀",encoding_rs::WINDOWS_1252,false).is_err());
    assert_eq!(crate::prefs::encode("café",encoding_rs::WINDOWS_1252,false).unwrap(),vec![99,97,102,233]);
}

#[test]
fn pr_creation_uses_the_push_repository_and_renamed_upstream() {
    let f=fixture("gitcraft-pr-destination-");let d=f.path();
    for args in [vec!["remote","add","origin","https://github.com/base/project.git"],vec!["remote","add","work","https://github.com/me/fork.git"],vec!["config","branch.main.remote","work"],vec!["config","branch.main.merge","refs/heads/published-feature"]] {assert!(git::run(d,&args).ok);}
    let (repo,branch)=crate::hosting::pull_request_destination(d,"main","").unwrap();
    assert_eq!(repo.path,"me/fork");assert_eq!(branch,"published-feature");
}

#[test]
fn patch_import_preserves_preexisting_resolution_and_merge_export_is_explicit() {
    let f=fixture("gitcraft-patch-session-");let d=f.path();let root=head(d);
    std::fs::create_dir(d.join(".git/rebase-apply")).unwrap();
    std::fs::write(d.join("file.txt"),"resolved\n").unwrap();assert!(git::run(d,&["add","file.txt"]).ok);
    let index=git::run(d,&["show",":file.txt"]).stdout;
    let patch=d.join("import.patch");std::fs::write(&patch,"From deadbeef Mon Sep 17 00:00:00 2001\n").unwrap();
    assert!(!crate::extras::apply_patch_file(d,&patch,&|_|panic!("must not start am")).ok);
    assert_eq!(git::run(d,&["show",":file.txt"]).stdout,index);assert!(d.join(".git/rebase-apply").exists());
    std::fs::remove_dir(d.join(".git/rebase-apply")).unwrap();assert!(git::run(d,&["reset","--hard","HEAD"]).ok);
    assert!(git::run(d,&["checkout","-qb","side"]).ok);std::fs::write(d.join("side.txt"),"side").unwrap();assert!(git::run(d,&["add","side.txt"]).ok);assert!(git::run(d,&["commit","-qm","side"]).ok);
    assert!(git::run(d,&["checkout","main"]).ok);assert!(git::run(d,&["merge","--no-ff","-m","merge","side"]).ok);
    assert!(crate::extras::make_patch(d,&[head(d)]).unwrap_err().contains("Merge commits"));
    assert!(crate::extras::make_patch(d,&[root]).unwrap().contains("Subject: [PATCH] base"));
}

#[test]
fn orig_cleanup_keeps_preexisting_and_ignored_files_and_waits_for_completion() {
    let f=fixture("gitcraft-orig-cleanup-");let d=f.path();
    std::fs::write(d.join("preexisting.orig"),"keep").unwrap();
    std::fs::write(d.join(".gitignore"),"ignored.orig\n").unwrap();
    crate::prefs::begin_orig_cleanup(d);
    std::fs::write(d.join("ignored.orig"),"keep").unwrap();std::fs::write(d.join("new.orig"),"backup").unwrap();
    std::fs::write(d.join(".git/MERGE_HEAD"),head(d)).unwrap();
    assert!(crate::prefs::delete_orig_files(d).is_empty());assert!(d.join("new.orig").exists());
    std::fs::remove_file(d.join(".git/MERGE_HEAD")).unwrap();
    assert_eq!(crate::prefs::delete_orig_files(d),vec!["new.orig"]);
    assert!(d.join("preexisting.orig").exists());assert!(d.join("ignored.orig").exists());
}


#[test]
fn submodules_keep_custom_names_and_spaced_paths_and_refuse_dirty_removal() {
    let source=fixture("gitcraft-submodule-source-");let parent=fixture("gitcraft-submodule-parent-");let d=parent.path();
    assert!(git::run(d,&["-c","protocol.file.allow=always","submodule","add","--name","custom-name",&source.path().to_string_lossy(),"module with spaces"]).ok);
    assert!(git::run(d,&["commit","-qam","module"]).ok);
    let list=crate::manage::submodules(d);assert_eq!(list.len(),1);assert_eq!(list[0].path,"module with spaces");assert_eq!(list[0].url,source.path().to_string_lossy());
    let e=Engine::default();std::fs::write(d.join("module with spaces/untracked.txt"),"precious").unwrap();
    assert!(!e.perform(&d.to_string_lossy(),Action::SubmoduleRemove {path:"module with spaces".into()}).ok);
    assert!(d.join("module with spaces/untracked.txt").exists());
    std::fs::remove_file(d.join("module with spaces/untracked.txt")).unwrap();
    std::fs::write(d.join("module with spaces/file.txt"),"dirty").unwrap();
    assert!(!e.perform(&d.to_string_lossy(),Action::SubmoduleRemove {path:"module with spaces".into()}).ok);
}

#[test]
fn replay_plan_continues_remaining_commits_after_a_conflict() {
    use crate::rewrite::{TodoItem,Step};
    let f=fixture("gitcraft-replay-plan-");let d=f.path();
    assert!(git::run(d,&["checkout","-qb","source"]).ok);
    std::fs::write(d.join("file.txt"),"source\n").unwrap();assert!(git::run(d,&["commit","-qam","first"]).ok);let first=head(d);
    std::fs::write(d.join("second.txt"),"second\n").unwrap();assert!(git::run(d,&["add","second.txt"]).ok);assert!(git::run(d,&["commit","-qm","second"]).ok);let second=head(d);
    assert!(git::run(d,&["checkout","main"]).ok);std::fs::write(d.join("file.txt"),"target\n").unwrap();assert!(git::run(d,&["commit","-qam","target"]).ok);let original=head(d);
    let plan=vec![TodoItem {action:Step::Pick,sha:first,message:None},TodoItem {action:Step::Reword,sha:second,message:Some("renamed second".into())}];
    let out=crate::rewrite::interactive_cherry_pick(d,&plan,&|a|git::run(d,a));assert!(!out.ok);assert!(d.join(".git/rebase-merge").exists());
    std::fs::write(d.join("file.txt"),"resolved\n").unwrap();assert!(git::run(d,&["add","file.txt"]).ok);
    let out=git::run(d,&["-c","core.editor=true","rebase","--continue"]);assert!(out.ok,"{}",out.message());
    assert!(d.join("second.txt").exists());assert_eq!(git::run(d,&["log","-1","--format=%s"]).stdout.trim(),"renamed second");
    assert!(git::run(d,&["merge-base","--is-ancestor",&original,"HEAD"]).ok);
}

#[test]
fn rebase_refuses_a_plan_after_external_commits_or_checkout() {
    let f=fixture("gitcraft-stale-plan-");let d=f.path();let base=head(d);
    std::fs::write(d.join("file.txt"),"B\n").unwrap();assert!(git::run(d,&["commit","-qam","B"]).ok);
    let expected=crate::rewrite::plan_state(d,&base).unwrap();
    let item=crate::rewrite::TodoItem {action:crate::rewrite::Step::Pick,sha:head(d),message:None};
    std::fs::write(d.join("C.txt"),"C").unwrap();assert!(git::run(d,&["add","C.txt"]).ok);assert!(git::run(d,&["commit","-qm","C"]).ok);let current=head(d);
    let e=Engine::default();assert!(!e.perform(&d.to_string_lossy(),Action::InteractiveRebase {base:base.clone(),items:vec![item.clone()],expected:expected.clone()}).ok);assert_eq!(head(d),current);assert!(d.join("C.txt").exists());
    assert!(git::run(d,&["checkout","-qb","other"]).ok);
    assert!(!e.perform(&d.to_string_lossy(),Action::InteractiveRebase {base,items:vec![item],expected}).ok);assert_eq!(head(d),current);
}

#[test]
fn spaced_remotes_hooks_and_config_include_round_trip() {
    let f=fixture("gitcraft-spaced-remote-");let d=f.path();let url=d.join("remote with spaces.git").to_string_lossy().into_owned();
    assert!(git::run(d,&["remote","add","origin",&url]).ok);let r=crate::manage::remotes(d);assert_eq!(r[0].fetch_url,url);assert_eq!(r[0].push_url,url);
    std::fs::write(d.join("included.cfg"),"[custom]\nvalue = included\n").unwrap();
    assert!(git::run(d,&["config","include.path",&d.join("included.cfg").to_string_lossy()]).ok);
    let m=crate::prefs::config_get(Some(d),"local",&["custom.value".into(),"custom.missing".into()]);assert_eq!(m["custom.value"],"included");assert_eq!(m["custom.missing"],"");
    if git::run(d,&["lfs","version"]).ok {
        assert!(git::run(d,&["config","core.hooksPath","custom-hooks"]).ok);assert!(git::run(d,&["lfs","install","--local"]).ok);
        assert!(crate::manage::lfs_info(d).enabled);
    }
}

#[test]
fn finder_preserves_unicode_names_and_blame_supports_sha256() {
    let f=fixture("gitcraft-finder-name-");let d=f.path();let name="café file.txt";
    std::fs::write(d.join(name),"unicode").unwrap();assert!(git::run(d,&["add","--",name]).ok);
    assert!(crate::files::tracked_files(d).contains(&name.to_owned()));
    let f=tempfile::tempdir().unwrap();let d=f.path();assert!(git::run(d,&["init","-q","--object-format=sha256"]).ok);
    for args in [vec!["config","user.name","SHA256 Test"],vec!["config","user.email","sha@example.test"],vec!["config","commit.gpgsign","false"]] {assert!(git::run(d,&args).ok);}
    std::fs::write(d.join("file.txt"),"one\ntwo\n").unwrap();assert!(git::run(d,&["add","."]).ok);assert!(git::run(d,&["commit","-qm","sha256"]).ok);
    let rows=crate::files::blame(d,"file.txt",None).unwrap();assert_eq!(rows.len(),2);assert_eq!(rows[0].sha.len(),64);assert_eq!(rows[0].author,"SHA256 Test");
}

#[test]
fn first_push_qualifies_a_branch_that_shares_its_name_with_a_tag() {
    let f=fixture("gitcraft-push-tag-");let remote=tempfile::tempdir().unwrap();let d=f.path();assert!(git::run(remote.path(),&["init","--bare","-q"]).ok);
    assert!(git::run(d,&["tag","main"]).ok);assert!(git::run(d,&["remote","add","origin",&remote.path().to_string_lossy()]).ok);
    let result=Engine::default().perform(&d.to_string_lossy(),Action::Push {force:false,lease:None});assert!(result.ok,"{}",result.message);
    assert_eq!(git::run(remote.path(),&["rev-parse","refs/heads/main"]).stdout.trim(),head(d));
    assert!(!git::run(remote.path(),&["show-ref","--verify","refs/tags/main"]).ok);
}

#[test]
fn cancelling_a_job_kills_its_helper_before_it_can_write() {
    use std::sync::{Arc,mpsc};use std::time::Duration;
    let f=fixture("gitcraft-cancel-helper-");let d=f.path().to_owned();let jobs=Arc::new(crate::jobs::Jobs::default());let (tx,rx)=mpsc::channel();
    jobs.set_listener(Arc::new(move |_,text,_| {if text.contains("helper-ready") {let _=tx.send(());}}));
    let runner=jobs.clone();let thread=std::thread::spawn(move || runner.run("helper",&d,&["-c","alias.audit-helper=!echo helper-ready >&2; sleep 3; echo leaked > cancelled-marker","audit-helper"]));
    rx.recv_timeout(Duration::from_secs(10)).unwrap();assert!(jobs.cancel("helper"));let result=thread.join().unwrap();assert!(!result.ok);
    std::thread::sleep(Duration::from_secs(4));assert!(!f.path().join("cancelled-marker").exists());
}


#[test]
fn gitflow_retry_finishes_after_the_develop_conflict_without_retagging() {
    let f=fixture("gitcraft-gitflow-retry-");let d=f.path();
    let cfg=crate::manage::GitflowConfig {master:"main".into(),develop:"develop".into(),feature:"feature/".into(),release:"release/".into(),hotfix:"hotfix/".into(),versiontag:"v".into(),initialized:true};
    assert!(crate::manage::gitflow_init(d,&cfg,&|a|git::run(d,a)).ok);
    assert!(git::run(d,&["checkout","develop"]).ok);std::fs::write(d.join("file.txt"),"develop\n").unwrap();assert!(git::run(d,&["commit","-qam","develop"]).ok);
    assert!(git::run(d,&["checkout","-qb","release/1","main"]).ok);std::fs::write(d.join("file.txt"),"release\n").unwrap();assert!(git::run(d,&["commit","-qam","release"]).ok);
    let first=crate::manage::gitflow_finish(d,"release","1",false,"release",&|a|git::run(d,a));assert!(!first.ok);assert!(d.join(".git/MERGE_HEAD").exists());
    let tag=git::run(d,&["rev-parse","refs/tags/v1"]).stdout;
    std::fs::write(d.join("file.txt"),"resolved\n").unwrap();assert!(git::run(d,&["add","file.txt"]).ok);assert!(git::run(d,&["commit","-qm","resolved"]).ok);
    let retry=crate::manage::gitflow_finish(d,"release","1",false,"release",&|a|git::run(d,a));assert!(retry.ok,"{}",retry.message());
    assert_eq!(git::run(d,&["rev-parse","refs/tags/v1"]).stdout,tag);assert!(!git::run(d,&["show-ref","--verify","refs/heads/release/1"]).ok);
}

#[test]
fn untracked_stash_entries_have_their_own_preview_revision() {
    let f=fixture("gitcraft-stash-preview-");let d=f.path();std::fs::write(d.join("new.txt"),"saved untracked\n").unwrap();
    assert!(git::run(d,&["stash","push","--include-untracked","-qm","external"]).ok);
    let sha=git::run(d,&["rev-parse","stash@{0}"]).stdout.trim().to_owned();let parents=git::run(d,&["show","-s","--format=%P",&sha]).stdout.split_whitespace().map(str::to_owned).collect();
    let c=git::Commit {sha,parents,author:String::new(),email:String::new(),time:0,summary:String::new(),body:String::new()};
    let files=git::commit_file_entries(d,&c);let file=files.iter().find(|f|f.change.path=="new.txt").unwrap();assert_eq!(file.change.status,'A');
    let revision=file.storage.as_ref().unwrap();let preview=crate::files::versions(d,&git::DiffSource::Commit(revision.clone()),"new.txt",None,false);
    assert_eq!(preview.new.unwrap().replace("\r\n","\n"),"saved untracked\n");assert!(preview.old.unwrap_or_default().is_empty());
}

#[test]
fn unborn_patch_exports_working_edits_relative_to_the_empty_tree() {
    let f=tempfile::tempdir().unwrap();let d=f.path();assert!(git::run(d,&["init","-q"]).ok);
    std::fs::write(d.join("new.txt"),"v1\n").unwrap();assert!(git::run(d,&["add","new.txt"]).ok);std::fs::write(d.join("new.txt"),"v2\n").unwrap();
    let patch=crate::extras::make_patch(d,&[]).unwrap();assert!(patch.contains("+v2"));assert!(!patch.contains("+v1"));assert!(!git::run(d,&["rev-parse","--verify","HEAD"]).ok);
}

#[test]
fn selected_ssh_keys_are_passed_as_literal_shell_arguments() {
    let f=fixture("gitcraft-ssh-literal-");let d=f.path();
    let key="C:/keys/key$(printf EXPANDED)`echo BAD`' spaced";
    let settings=crate::prefs::RuntimeSettings {ssh_use_agent:false,ssh_private_key:key.into(),..Default::default()};
    let command=crate::prefs::ssh_command(&settings).unwrap();
    let alias=format!("alias.audit-key=!f() {{ printf '%s\\n' \"$@\"; }}; {}",command.replacen("ssh ","f ",1));
    let output=git::run(d,&["-c",&alias,"audit-key"]);assert!(output.ok,"{}",output.message());
    assert_eq!(output.stdout.lines().nth(1),Some(key));
}


#[test]
fn legacy_partial_patches_are_rejected_without_altering_file_or_index_bytes() {
    let f=fixture("gitcraft-legacy-patch-");let d=f.path();let index=git::git_cmd(d).args(["show",":file.txt"]).output().unwrap().stdout;
    let bytes=b"base\ncaf\xe9\n".to_vec();std::fs::write(d.join("file.txt"),&bytes).unwrap();
    assert!(crate::files::apply_part(d,"file.txt",false,crate::files::PatchOp::Stage,0,None).is_err());
    assert!(crate::files::apply_part(d,"file.txt",false,crate::files::PatchOp::Discard,0,None).is_err());
    assert!(crate::extras::make_patch(d,&[]).is_err());
    assert_eq!(git::git_cmd(d).args(["show",":file.txt"]).output().unwrap().stdout,index);
    assert_eq!(std::fs::read(d.join("file.txt")).unwrap(),bytes);
    assert!(git::run(d,&["add","file.txt"]).ok);
    assert_eq!(git::git_cmd(d).args(["show",":file.txt"]).output().unwrap().stdout,bytes);
    assert!(crate::files::apply_part(d,"file.txt",false,crate::files::PatchOp::Unstage,0,None).is_err());
}

#[test]
fn fork_pull_request_checkout_never_uses_a_same_named_local_branch() {
    let base=fixture("gitcraft-pr-base-");let source=fixture("gitcraft-pr-fork-");let d=base.path();let fork=source.path();
    std::fs::write(fork.join("fork.txt"),"fork only").unwrap();assert!(git::run(fork,&["add","fork.txt"]).ok);assert!(git::run(fork,&["commit","-qm","fork"]).ok);
    let expected=head(fork);let local=head(d);let e=Engine::default();
    assert!(e.perform(&d.to_string_lossy(),Action::CheckoutPullRequest {url:fork.to_string_lossy().into_owned(),reference:"refs/heads/main".into(),expected:expected.clone()}).ok);
    assert_eq!(head(d),expected);assert_eq!(git::run(d,&["rev-parse","refs/heads/main"]).stdout.trim(),local);assert!(d.join("fork.txt").exists());
    assert!(e.perform(&d.to_string_lossy(),Action::Undo).ok);assert_eq!(head(d),local);
    assert!(!e.perform(&d.to_string_lossy(),Action::CheckoutPullRequest {url:fork.to_string_lossy().into_owned(),reference:"refs/heads/main".into(),expected:local.clone()}).ok);assert_eq!(head(d),local);
}

#[test]
fn sparse_patterns_accept_root_and_nested_files() {
    let source=fixture("gitcraft-sparse-source-");let d=source.path();std::fs::create_dir(d.join("nested")).unwrap();std::fs::write(d.join("nested/only.txt"),"nested").unwrap();std::fs::write(d.join("README.md"),"readme").unwrap();
    assert!(git::run(d,&["add","."]).ok);assert!(git::run(d,&["commit","-qm","files"]).ok);
    assert!(git::run(d,&["sparse-checkout","set","--no-cone","--","/README.md","/nested/only.txt"]).ok);
    assert!(d.join("README.md").exists());assert!(d.join("nested/only.txt").exists());assert!(!d.join("file.txt").exists());
}

#[test]
fn selected_gpg_program_and_fingerprints_are_discovered_together() {
    let f=fixture("gitcraft-configured-gpg-");let d=f.path();
    let fingerprint="123456789012345678901234ABCDEF0123456789";
    let output=format!("sec:u:4096:1:ABCDEF0123456789:::::::::\nfpr:::::::::{fingerprint}:\nuid:::::::::Configured User:\n");
    #[cfg(windows)] let program={let p=d.join("configured-gpg.cmd");let script=format!("@echo off\r\n{}\r\nexit /b 0\r\n",output.lines().map(|l|format!("echo {l}")).collect::<Vec<_>>().join("\r\n"));std::fs::write(&p,script).unwrap();p};
    #[cfg(unix)] let program={use std::os::unix::fs::PermissionsExt;let p=d.join("configured-gpg");std::fs::write(&p,format!("#!/bin/sh\ncat <<'END'\n{output}END\n")).unwrap();std::fs::set_permissions(&p,std::fs::Permissions::from_mode(0o700)).unwrap();p};
    assert!(git::run(d,&["config","gpg.program",&program.to_string_lossy()]).ok);
    let account=crate::accounts::info(d);assert!(account.gpg);assert_eq!(account.gpg_keys.len(),1);assert_eq!(account.gpg_keys[0].fingerprint,fingerprint);assert_eq!(account.gpg_keys[0].id,"ABCDEF0123456789");
}

#[test]
fn activity_log_transfers_nothing_when_the_version_is_unchanged() {
    let e=Engine::default();let f=fixture("gitcraft-log-version-");let initial=e.log_snapshot(None).unwrap();assert!(initial.1.is_empty());assert!(e.log_snapshot(Some(initial.0)).is_none());
    assert!(e.git(f.path(),&["--version"]).ok);let updated=e.log_snapshot(Some(initial.0)).unwrap();assert_eq!(updated.1.len(),1);assert_ne!(updated.0,initial.0);assert!(e.log_snapshot(Some(updated.0)).is_none());
}


#[test]
fn commit_control_characters_remain_data_in_all_history_readers() {
    let f=fixture("gitcraft-history-framing-");let d=f.path();let base=head(d);
    let body=format!("Real body\u{1e}{}\u{1f}\u{1f}Invented Author\u{1f}invented@example.test\u{1f}1\u{1f}Invented Subject\u{1f}Invented Body","f".repeat(40));
    std::fs::write(d.join("file.txt"),"changed\n").unwrap();assert!(git::run(d,&["commit","-qam",&format!("Real\u{1f} subject\n\n{body}")]).ok);
    let history=git::load(d,100);assert_eq!(history.commits.len(),2);assert!(history.commits.iter().all(|commit| commit.sha!="f".repeat(40)));assert!(history.commits[0].body.contains(&body));
    let file_history=crate::files::history(d,"file.txt",100);assert_eq!(file_history.len(),2);assert_eq!(file_history[0].commit.sha,head(d));assert!(file_history[0].commit.body.contains(&body));
    let plan=crate::rewrite::rebase_commits(d,&base).unwrap();assert_eq!(plan.len(),1);assert!(plan[0].body.contains(&body));
}

#[test]
fn lfs_initialization_failures_preserve_the_created_repository_and_report_recovery() {
    let f=fixture("gitcraft-lfs-init-warning-");let d=f.path();let before=head(d);
    let result=crate::manage::initialize_lfs(d,true,&|args| {assert_eq!(args,["lfs","install","--local"]);git::GitOut::failed("Existing custom pre-push hook")});
    assert_eq!(result.path,d.to_string_lossy());assert!(result.warning.unwrap().contains("Repository created, but Git LFS setup did not finish"));assert_eq!(head(d),before);
    assert!(crate::manage::initialize_lfs(d,false,&|_|panic!("not requested")).warning.is_none());
    assert!(crate::manage::initialize_lfs(d,true,&|_|git::GitOut {ok:true,stdout:String::new(),stderr:String::new()}).warning.is_none());
}

#[test]
fn explicit_utf8_conversion_preserves_unsupported_characters_in_future_saves() {
    let f=fixture("gitcraft-explicit-utf8-");let d=f.path();let bytes=b"caf\xe9\n";
    crate::prefs::set_runtime(crate::prefs::RuntimeSettings {encodings:[(d.to_string_lossy().into_owned(),"windows-1252".into())].into(),ssh_use_agent:true,..Default::default()});
    std::fs::write(d.join("file.txt"),bytes).unwrap();let token=crate::files::content_token(Some(bytes));
    assert!(crate::files::save_checked_mode(d,"file.txt","café 😀\n",Some(&token),false).is_err());assert_eq!(std::fs::read(d.join("file.txt")).unwrap(),bytes);
    crate::files::save_checked_mode(d,"file.txt","café 😀\n",Some(&token),true).unwrap();let converted=std::fs::read(d.join("file.txt")).unwrap();assert!(converted.starts_with(&[0xef,0xbb,0xbf]));
    crate::files::save_checked_mode(d,"file.txt","café 😀 next\n",Some(&crate::files::content_token(Some(&converted))),false).unwrap();
    assert!(std::fs::read_to_string(d.join("file.txt")).unwrap().contains("😀 next"));crate::prefs::set_runtime(crate::prefs::RuntimeSettings {ssh_use_agent:true,..Default::default()});
}

#[test]
fn self_hosted_https_ports_are_distinct_from_ssh_transport_ports() {
    let https=crate::hosting::parse_remote("https://gitlab.example.test:8443/team/repo.git","https://gitlab.example.test:8443").unwrap();assert_eq!(https.host,"gitlab.example.test:8443");
    let ssh=crate::hosting::parse_remote("ssh://git@gitlab.example.test:2222/team/repo.git","https://gitlab.example.test:8443").unwrap();assert_eq!(ssh.web,"https://gitlab.example.test:8443/team/repo");
}


#[test]
fn generated_key_passphrases_use_stdin_for_empty_and_nonempty_values() {
    let folder=tempfile::tempdir().unwrap();let d=folder.path();
    #[cfg(windows)] let program={let path=d.join("fake-gpg.cmd");std::fs::write(&path,"@echo off\r\nif \"%~1\"==\"--list-secret-keys\" goto list\r\necho %* > \"%~dp0arguments.txt\"\r\nset /p synthetic=\r\n> \"%~dp0stdin.txt\" echo %synthetic%\r\nexit /b 0\r\n:list\r\necho fpr:::::::::SYNTHETIC-FINGERPRINT:\r\nexit /b 0\r\n").unwrap();path};
    #[cfg(unix)] let program={use std::os::unix::fs::PermissionsExt;let path=d.join("fake-gpg");std::fs::write(&path,"#!/bin/sh\ndir=$(dirname \"$0\")\nif [ \"$1\" = --list-secret-keys ]; then echo fpr:::::::::SYNTHETIC-FINGERPRINT:; exit 0; fi\nprintf '%s\\n' \"$@\" > \"$dir/arguments.txt\"\nIFS= read -r synthetic\nprintf '%s\\n' \"$synthetic\" > \"$dir/stdin.txt\"\n").unwrap();std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700)).unwrap();path};
    for secret in ["","synthetic-passphrase"] {
        assert_eq!(crate::prefs::gpg_generate(&program.to_string_lossy(),"Test","fake@example.test",secret).unwrap(),"SYNTHETIC-FINGERPRINT");
        let args=std::fs::read_to_string(d.join("arguments.txt")).unwrap();assert!(args.contains("--passphrase-fd"));assert!(!args.contains("synthetic-passphrase"));
        let input=std::fs::read_to_string(d.join("stdin.txt")).unwrap();if !secret.is_empty() {assert_eq!(input.trim(),secret);}
    }
}


#[test]
fn discard_redo_detects_changes_between_distinct_non_utf8_bytes() {
    let f=fixture("gitcraft-redo-legacy-");let d=f.path();let e=Engine::default();
    std::fs::write(d.join("file.txt"),b"caf\xe9\n").unwrap();assert!(e.perform(&d.to_string_lossy(),Action::Discard {paths:vec!["file.txt".into()]}).ok);assert!(e.perform(&d.to_string_lossy(),Action::Undo).ok);
    std::fs::write(d.join("file.txt"),b"caf\xe8\n").unwrap();assert!(!e.perform(&d.to_string_lossy(),Action::Redo).ok);assert_eq!(std::fs::read(d.join("file.txt")).unwrap(),b"caf\xe8\n");
}

#[test]
fn reviewed_merge_backups_refuse_changes_while_confirmation_is_open() {
    let f=fixture("gitcraft-orig-review-");let d=f.path();crate::prefs::begin_orig_cleanup(d);
    std::fs::write(d.join("new.orig"),"old backup").unwrap();let candidates=crate::prefs::orig_cleanup_candidates(d);assert_eq!(candidates.len(),1);
    std::fs::write(d.join("new.orig"),"new backup").unwrap();assert!(crate::prefs::delete_orig_selected(d,&candidates).is_err());assert!(d.join("new.orig").exists());
    let current=crate::prefs::orig_cleanup_candidates(d);assert_eq!(crate::prefs::delete_orig_selected(d,&current).unwrap(),vec!["new.orig"]);
}


#[test]
fn escaped_exact_ignore_patterns_match_only_the_named_file_and_directory() {
    let f=fixture("gitcraft-exact-ignore-");let d=f.path();
    for name in ["[ab].txt","a.txt","b.txt"] {std::fs::write(d.join(name),"untracked").unwrap();}
    crate::manage::ignore(d,r"/\[ab\].txt").unwrap();
    assert!(git::run(d,&["check-ignore","--","[ab].txt"]).ok);assert!(!git::run(d,&["check-ignore","--","a.txt","b.txt"]).ok);
    std::fs::create_dir(d.join("[dir]")).unwrap();std::fs::write(d.join("[dir]/inside.txt"),"ignored").unwrap();
    crate::manage::ignore(d,r"/\[dir\]/").unwrap();assert!(git::run(d,&["check-ignore","--","[dir]/inside.txt"]).ok);
}
