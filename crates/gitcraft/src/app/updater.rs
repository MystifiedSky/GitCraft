//! Automatic updates: check GitHub Releases at start-up and every hour while GitCraft is open,
//! download a newer version in the background and offer "Update Ready (Restart GitCraft)".
//! File > Check for Update works with the automatic checks turned off.

use super::{App, Msg, VERSION};
use gitcraft_core::updates::{self, Update};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    None,
    Downloading,
    Ready,
    Failed,
}

#[derive(Default)]
pub struct UpdateState {
    pub phase: Phase,
    pub update: Option<Update>,
    pub file: Option<PathBuf>,
    /// Download progress in percent (`u32::MAX`: unknown).
    pub percent: Arc<AtomicU32>,
    pub error: Option<String>,
    pub checked_at: Option<Instant>,
    busy: bool,
}

impl UpdateState {
    pub fn percent(&self) -> Option<u32> {
        let p = self.percent.load(Ordering::Relaxed);
        (p <= 100).then_some(p)
    }
}

impl App {
    /// Run the hourly check when it is due. Development builds only check when asked, so they
    /// don't offer to replace themselves with the published release.
    pub(super) fn schedule_updates(&mut self, ctx: &egui::Context) {
        if cfg!(debug_assertions) || self.screenshot.is_some() || !self.settings.flag("autoUpdate", true) || self.update.phase == Phase::Ready || self.update.busy {
            return;
        }
        let due = self.update.checked_at.is_none_or(|t| t.elapsed() >= CHECK_EVERY);
        if due {
            self.check_for_updates(false);
        } else if let Some(t) = self.update.checked_at {
            ctx.request_repaint_after(CHECK_EVERY.saturating_sub(t.elapsed()));
        }
    }

    /// Look for a newer release; when one is found, download it (in the background when
    /// automatic downloads are on, or because this was asked for).
    pub(crate) fn check_for_updates(&mut self, manual: bool) {
        if self.update.busy || self.screenshot.is_some() {
            return;
        }
        if self.update.phase == Phase::Ready {
            if manual {
                self.toast(true, "An update is ready. Restart GitCraft to install it.");
            }
            return;
        }
        if let Err(e) = updates::can_install() {
            self.update.checked_at = Some(Instant::now());
            if manual {
                self.toast(false, e);
            }
            return;
        }
        self.update.busy = true;
        self.update.checked_at = Some(Instant::now());
        self.update.phase = Phase::Checking;
        self.update.error = None;
        self.spawn(move || {
            let r = updates::check(VERSION);
            Msg::Apply(Box::new(move |app: &mut App| {
                app.update.busy = false;
                match r {
                    Ok(None) => {
                        app.update.phase = Phase::None;
                        if manual {
                            app.toast(true, format!("GitCraft {VERSION} is the newest version."));
                        }
                    }
                    Ok(Some(u)) => {
                        app.update.update = Some(u);
                        if manual || app.settings.flag("autoUpdate", true) {
                            app.download_update(manual);
                        } else {
                            app.update.phase = Phase::Idle;
                        }
                    }
                    Err(e) => {
                        app.update.phase = Phase::Failed;
                        if manual {
                            app.toast(false, format!("Could not check for updates: {e}"));
                        }
                        app.update.error = Some(e);
                    }
                }
            }))
        });
    }

    fn download_update(&mut self, manual: bool) {
        let Some(update) = self.update.update.clone() else { return };
        self.update.busy = true;
        self.update.phase = Phase::Downloading;
        self.update.percent.store(u32::MAX, Ordering::Relaxed);
        let (percent, wake) = (self.update.percent.clone(), self.ctx.clone());
        self.spawn(move || {
            let r = updates::download(&update, move |p| {
                percent.store(p.unwrap_or(u32::MAX), Ordering::Relaxed);
                wake.request_repaint();
            });
            Msg::Apply(Box::new(move |app: &mut App| {
                app.update.busy = false;
                match r {
                    Ok(file) => {
                        app.update.file = Some(file);
                        app.update.phase = Phase::Ready;
                    }
                    Err(e) => {
                        app.update.phase = Phase::Failed;
                        if manual {
                            app.toast(false, format!("The update could not be downloaded: {e}"));
                        }
                        app.update.error = Some(e);
                    }
                }
            }))
        });
    }

    /// "Update Ready (Restart GitCraft)": install, then start the new version.
    pub(crate) fn install_update(&mut self, ctx: &egui::Context) {
        if self.update.phase != Phase::Ready || self.update.busy {
            return;
        }
        let Some(file) = self.update.file.clone() else { return };
        // Save or Discard first; Discard leaves the text in place until GitCraft has closed, so
        // a failure below keeps it.
        if self.ask_unsaved(Vec::new(), false, Box::new(|app: &mut App| {
            let ctx = app.ctx.clone();
            app.install_update(&ctx);
        })) {
            return;
        }
        if let Err(e) = self.settings.flush() {
            self.discarding = false;
            self.toast(false, format!("Could not save preferences before restarting: {e}"));
            return;
        }
        // Let the new version take the window over instead of handing its launch to this one.
        if let Some(p) = self.primary.as_mut() {
            p.shutdown();
        }
        match updates::install(&file) {
            Ok(true) => {
                if let Err(e) = updates::relaunch() {
                    self.discarding = false;
                    self.resume_launches();
                    self.toast(false, e);
                    return;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            // The Windows installer is scheduled to run once this process has closed.
            Ok(false) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => {
                self.discarding = false;
                self.resume_launches();
                self.toast(false, format!("The update could not be installed: {e}"));
            }
        }
    }

    /// The window stays open after all: it takes launches from other copies again.
    fn resume_launches(&mut self) {
        if let Some(p) = self.primary.as_mut() {
            p.resume();
        }
    }
}
