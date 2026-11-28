//! The AI buttons: commit and stash messages, explaining a commit or branch, composing commits
//! and merging a conflict. Each request runs with a snapshot of Preferences > AI; untracked files
//! are listed for review before they are sent.

use super::App;
use super::dialogs::Dialog;
use super::widgets::Kind;
use gitcraft_core::actions::{Action, RecomposeSource};
use gitcraft_core::ai::{self, AiConfig, ComposedCommit};
use gitcraft_core::ai_context::{self, PreviewKind};
use std::path::Path;

/// What an AI request produced, and where it goes.
pub enum AiOutput {
    CommitMessage { path: String, before: super::repo::Draft, message: ai::GeneratedCommitMessage },
    StashTitle { tab: u64, before: String, text: String },
    /// `key` names what was explained (a commit, or a branch at its tip) for the recent explanations kept.
    Explain { key: Option<String>, title: String, text: String },
    Compose { tab: u64, commits: Vec<ComposedCommit>, source: Option<RecomposeSource> },
    Merge { tab: u64, file: String, editor: u64, revision: u64, text: String, notes: Vec<ai::ConflictNote> },
    PullText { tab: u64, request: u64, title: String, body: String },
}

/// A request that may need the untracked files reviewed first.
#[derive(Clone, Copy)]
enum Reviewed {
    Commit,
    Stash,
    Compose,
}

impl App {
    /// Preferences > AI as the backend reads it.
    pub(crate) fn ai_config(&self) -> AiConfig {
        let mut base = serde_json::to_value(AiConfig::default()).unwrap_or_default();
        if let (Some(obj), Some(saved)) = (base.as_object_mut(), self.settings.get::<serde_json::Value>("ai").and_then(|v| v.as_object().cloned())) {
            for (k, v) in saved {
                obj.insert(k, v);
            }
        }
        serde_json::from_value(base).unwrap_or_default()
    }

    /// Run one AI request away from the window, with a busy note and Stop AI.
    fn ai_run(&mut self, label: &str, job: impl FnOnce() -> Result<AiOutput, String> + Send + 'static) {
        self.ai_run_for(label, None, job);
    }

    fn ai_run_for(&mut self, label: &str, commit_tab: Option<u64>, job: impl FnOnce() -> Result<AiOutput, String> + Send + 'static) {
        self.ai_requests += 1;
        let id = format!("ai-{}", self.ai_requests);
        self.ai_reads.push(id.clone());
        self.ai_label = Some(label.to_owned());
        let config = self.ai_config();
        let read = id.clone();
        self.spawn(move || {
            let result = gitcraft_core::read_jobs::scope_for(Some(read), std::time::Duration::from_secs(300), || ai::with_config(config, job));
            super::Msg::Ai { id, commit_tab, result: Box::new(result) }
        });
    }

    pub(crate) fn stop_ai(&mut self) {
        for id in std::mem::take(&mut self.ai_reads) {
            gitcraft_core::read_jobs::cancel(&id);
        }
        self.ai_label = None;
        // A stopped AI merge no longer keeps the conflict editor busy.
        for r in self.repos.values_mut() {
            r.conflict_ui.ai_busy = false;
        }
    }

    pub(crate) fn ai_done(&mut self, id: String, commit_tab: Option<u64>, result: Result<AiOutput, String>) {
        // A request stopped with Stop AI changes nothing, whatever it brings back.
        if !self.ai_reads.contains(&id) {
            return;
        }
        self.ai_reads.retain(|r| *r != id);
        if self.ai_reads.is_empty() {
            self.ai_label = None;
        }
        let out = match result {
            Ok(out) => out,
            Err(e) => {
                if let Some(tab) = commit_tab {
                    return self.commit_ai_failed(tab, e);
                }
                if e.contains("Preferences > AI") {
                    self.open_prefs(Some("ai"));
                }
                return self.toast(false, e);
            }
        };
        match out {
            AiOutput::CommitMessage { path, before, message } => {
                let ai::GeneratedCommitMessage { summary, description, local_fallback } = message;
                if local_fallback {
                    self.toast(true, "The AI size limit was reached. GitCraft prepared a summary from the changed files.");
                }
                let now = self.draft(&path);
                if now != before {
                    let text = format!("{summary}\n\n{description}\n\nYour draft changed while this was generated.");
                    self.dialog = Some(Dialog::new("Generated commit message", super::dialogs::Body::Text(text)).choice("Use generated message", Kind::Default, move |app, _| {
                        let mut d = app.draft(&path);
                        d.summary = summary;
                        d.description = description;
                        app.set_draft(&path, d);
                    }));
                } else {
                    let mut d = now;
                    d.summary = summary;
                    d.description = description;
                    self.set_draft(&path, d);
                }
            }
            AiOutput::StashTitle { tab, before, text } => {
                // A title typed while this was generated is kept unless the generated one is chosen.
                let now = self.repo(tab).map(|r| r.work.stash_message.clone()).unwrap_or_default();
                if now == before {
                    if let Some(r) = self.repo_mut(tab) {
                        r.work.stash_message = text;
                    }
                } else {
                    let shown = format!("{text}\n\nYour stash title changed while this was generated.");
                    self.dialog = Some(Dialog::new("Generated stash title", super::dialogs::Body::Text(shown)).choice("Use generated title", Kind::Default, move |app, _| {
                        if let Some(r) = app.repo_mut(tab) {
                            r.work.stash_message = text;
                        }
                    }));
                }
            }
            AiOutput::Explain { key, title, text } => {
                let text = format!("{text}\n\nGenerated using your selected AI connection. Review the result before using it.");
                // The last few explanations are kept, so asking again shows them without another request.
                if let Some(key) = key {
                    self.explanations.retain(|(k, ..)| *k != key);
                    self.explanations.push_front((key, title.clone(), text.clone()));
                    self.explanations.truncate(5);
                }
                self.dialog = Some(Dialog::info(&title, &text));
            }
            AiOutput::Compose { tab, commits, source } => {
                let n = commits.len();
                let list = commits.iter().enumerate().map(|(i, c)| format!("{}. {}\n{}", i + 1, c.summary, c.files.iter().map(|f| format!("     {f}")).collect::<Vec<_>>().join("\n"))).collect::<Vec<_>>().join("\n\n");
                let plural = if n == 1 { "" } else { "s" };
                self.dialog = Some(Dialog::confirm_with(&format!("Create {n} commit{plural}?"), &format!("{list}\n\nYou can undo this with Undo."), &format!("Create {n} Commit{plural}"), false, move |app, _| app.act(tab, Action::ComposeCommits { commits, source })));
            }
            AiOutput::Merge { tab, file, editor, revision, text, notes } => self.ai_merged(tab, &file, editor, revision, Ok((text, notes))),
            AiOutput::PullText { tab, request, title, body } => self.pr_text_ready(tab, request, title, body),
        }
    }

    fn commit_ai_failed(&mut self, tab: u64, error: String) {
        let error = if error.trim().is_empty() {
            "The request returned no error details. Try again or test the connection in Preferences > AI.".into()
        } else {
            gitcraft_core::git::redact_diagnostic(&error)
        };
        if let Some(repo) = self.repo_mut(tab) {
            repo.work.commit_ai_error = Some(error.clone());
        }
        self.toast(false, format!("Could not generate a commit message. {error}"));
    }

    /// List the untracked files a request would send, ask, then send.
    fn with_reviewed(&mut self, tab: u64, kind: Reviewed) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let preview_kind = match kind {
            Reviewed::Commit => PreviewKind::Commit,
            Reviewed::Stash => PreviewKind::Stash,
            Reviewed::Compose => PreviewKind::Compose,
        };
        self.ai_label = Some("Checking which files to share…".into());
        self.spawn(move || {
            let result = ai_context::preview(Path::new(&path), preview_kind).map(|p| (p.id, p.files));
            super::Msg::AiPreview { tab, kind: kind as u8, result }
        });
    }

    pub(crate) fn ai_preview(&mut self, tab: u64, kind: u8, result: Result<(Option<u32>, Vec<String>), String>) {
        if self.ai_reads.is_empty() {
            self.ai_label = None;
        }
        let kind = match kind {
            0 => Reviewed::Commit,
            1 => Reviewed::Stash,
            _ => Reviewed::Compose,
        };
        let (id, files) = match result {
            Ok(r) => r,
            Err(e) if matches!(kind, Reviewed::Commit) => return self.commit_ai_failed(tab, e),
            Err(e) => return self.toast(false, e),
        };
        if id.is_none() {
            return self.ai_send(tab, kind, None);
        }
        let text = files.iter().map(|f| format!("\"{}\"", f.replace('"', "\\\""))).collect::<Vec<_>>().join("\n");
        self.dialog = Some(Dialog::new("Send these untracked files to AI?", super::dialogs::Body::Text(text)).choice("Send and continue", Kind::Primary, move |app, _| app.ai_send(tab, kind, id)));
    }

    fn ai_send(&mut self, tab: u64, kind: Reviewed, preview: Option<u32>) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        match kind {
            Reviewed::Commit => {
                let before = self.draft(&path);
                self.ai_run_for("Writing a commit message…", Some(tab), move || {
                    let dir = Path::new(&path);
                    let files = ai_context::take_preview(dir, PreviewKind::Commit, preview)?;
                    let message = ai::commit_message(dir, None, &files)?;
                    Ok(AiOutput::CommitMessage { path, before, message })
                });
            }
            Reviewed::Stash => {
                let before = self.repo(tab).map(|r| r.work.stash_message.clone()).unwrap_or_default();
                self.ai_run("Naming the stash…", move || {
                    let dir = Path::new(&path);
                    let files = ai_context::take_preview(dir, PreviewKind::Stash, preview)?;
                    Ok(AiOutput::StashTitle { tab, before, text: ai::stash_message(dir, &files.text)? })
                })
            }
            Reviewed::Compose => self.ai_run("Grouping your changes into commits…", move || {
                let dir = Path::new(&path);
                let files = ai_context::take_preview(dir, PreviewKind::Compose, preview)?;
                Ok(AiOutput::Compose { tab, commits: ai::compose(dir, files)?, source: None })
            }),
        }
    }

    pub(crate) fn ai_commit_message(&mut self, tab: u64) {
        if let Some(repo) = self.repo_mut(tab) {
            repo.work.commit_ai_error = None;
        }
        self.with_reviewed(tab, Reviewed::Commit);
    }

    pub(crate) fn ai_stash_title(&mut self, tab: u64) {
        self.with_reviewed(tab, Reviewed::Stash);
    }

    pub(crate) fn compose_commits(&mut self, tab: u64) {
        self.with_reviewed(tab, Reviewed::Compose);
    }

    /// A kept explanation for `key`, shown with Explain Again (which asks the AI anew). Returns whether one was kept.
    fn cached_explanation(&mut self, key: &str, again: impl FnOnce(&mut App) + 'static) -> bool {
        let Some((_, title, text)) = self.explanations.iter().find(|(k, ..)| k == key).cloned() else { return false };
        self.dialog = Some(Dialog::confirm_with(&title, &text, "Explain Again", false, move |app, _| again(app)).cancel_label("Close"));
        true
    }

    pub(crate) fn explain_commit(&mut self, tab: u64, sha: &str, summary: &str) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let key = format!("commit:{path}:{sha}");
        let (s2, summary2, k2) = (sha.to_owned(), summary.to_owned(), key.clone());
        if self.cached_explanation(&key, move |app| app.explain_now(tab, k2, ExplainWhat::Commit(s2, summary2))) {
            return;
        }
        self.explain_now(tab, key, ExplainWhat::Commit(sha.to_owned(), summary.to_owned()));
    }

    pub(crate) fn explain_branch(&mut self, tab: u64, branch: &str) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        // Kept by the branch's tip, so a branch that moved is explained again.
        let tip = self.repo(tab).and_then(|r| r.view.as_ref()).and_then(|v| v.data.refs.iter().find(|r| r.name == branch).map(|r| r.sha.clone())).unwrap_or_default();
        let key = format!("branch:{path}:{branch}:{tip}");
        let (b2, k2) = (branch.to_owned(), key.clone());
        if self.cached_explanation(&key, move |app| app.explain_now(tab, k2, ExplainWhat::Branch(b2))) {
            return;
        }
        self.explain_now(tab, key, ExplainWhat::Branch(branch.to_owned()));
    }

    fn explain_now(&mut self, tab: u64, key: String, what: ExplainWhat) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        match what {
            ExplainWhat::Commit(sha, summary) => {
                let title = format!("Explain: {summary}");
                self.ai_run("Explaining the commit…", move || Ok(AiOutput::Explain { key: Some(key), title, text: ai::explain_commit(Path::new(&path), &sha)? }));
            }
            ExplainWhat::Branch(branch) => {
                let title = format!("What {branch} changes");
                self.ai_run(&format!("Explaining {branch}…"), move || {
                    let dir = Path::new(&path);
                    let base = gitcraft_core::extras::default_target(dir).ok_or("No branch to compare with")?;
                    Ok(AiOutput::Explain { key: Some(key), title, text: ai::explain_branch(dir, &branch, &base)? })
                });
            }
        }
    }

    /// Recompose with AI: the commits after `base` on the checked-out branch are undone (a soft reset, which Undo
    /// reverses) and their changes are composed into new commits with AI. The working folder
    /// must be clean so nothing else is folded in.
    pub(crate) fn recompose_commits(&mut self, tab: u64, base: String, count: usize) {
        let Some(v) = self.repo(tab).and_then(|r| r.view.as_ref()) else { return };
        if !v.data.status.staged.is_empty() || !v.data.status.unstaged.is_empty() {
            return self.toast(false, "Commit or stash your working changes first, so only these commits are recomposed.");
        }
        let plural = if count == 1 { "" } else { "s" };
        let original = v.data.head_sha.clone();
        let text = format!("The {count} newest commit{plural} on this branch are undone (their changes stay in the working folder) and composed again into new commits with AI. You can review the result before it is committed, and Undo brings the original commits back.");
        self.dialog = Some(Dialog::confirm_with(&format!("Recompose {count} commit{plural} with AI?"), &text, "Recompose", false, move |app, _| {
            app.recompose_pending = Some((tab, RecomposeSource { base: base.clone(), original }));
            app.act(tab, Action::Reset { sha: base, mode: "soft".into() });
        }));
    }

    /// The soft reset of Recompose with AI finished: compose its changes, or stop if it failed.
    pub(crate) fn recompose_reset_done(&mut self, tab: u64, ok: bool) {
        if !self.recompose_pending.as_ref().is_some_and(|(pending, _)| *pending == tab) {
            return;
        }
        if let Some((_, source)) = self.recompose_pending.take()
            && ok
            && let Some(path) = self.repo(tab).map(|repo| repo.path.clone())
        {
            self.ai_run("Grouping your changes into commits…", move || {
                let commits = ai::compose_tree(Path::new(&path), &source.base, &source.original)?;
                Ok(AiOutput::Compose { tab, commits, source: Some(source) })
            });
        }
    }

    /// Explain the uncommitted changes. Not kept: they change as you work.
    pub(crate) fn explain_working(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        self.ai_run("Explaining the working changes…", move || Ok(AiOutput::Explain { key: None, title: "Explain working changes".into(), text: ai::explain_working(Path::new(&path))? }));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ai_resolve_conflict(&mut self, tab: u64, base: String, ours: String, theirs: String, file: String, editor: u64, revision: u64) {
        self.ai_run("Merging both sides…", move || {
            let (text, notes) = ai::resolve_conflict(&base, &ours, &theirs, &file)?;
            Ok(AiOutput::Merge { tab, editor, revision, text, notes, file })
        });
        if let Some(r) = self.repo_mut(tab) {
            r.conflict_ui.ai_busy = true;
        }
    }

    pub(crate) fn ai_pull_request(&mut self, tab: u64, request: u64, source: String, target: String) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        self.ai_run("Writing the pull request…", move || {
            let (title, body) = ai::pull_request_text(Path::new(&path), &source, &target)?;
            Ok(AiOutput::PullText { tab, request, title, body })
        });
    }
}

/// What an explanation is of.
enum ExplainWhat {
    Commit(String, String),
    Branch(String),
}
