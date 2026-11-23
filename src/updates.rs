use crate::i18n::Text;
use eframe::egui;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, sources::GithubSource};

const REPO: &str = "https://github.com/DwennK/Easy-Glucose";
const DAY: i64 = 24 * 60 * 60;

enum Status {
    Idle,
    Checking,
    Current,
    Available(Box<UpdateInfo>),
    Installing,
    InstallFailed,
    Restart,
    Failed,
}

pub(crate) struct Updates {
    manager: Option<UpdateManager>,
    status: Status,
    pending: Option<Receiver<Status>>,
}

impl Updates {
    pub fn new() -> Self {
        let source = GithubSource::new(REPO, None, false);
        Self {
            manager: UpdateManager::new(source, None, None).ok(),
            status: Status::Idle,
            pending: None,
        }
    }

    pub fn managed(&self) -> bool {
        self.manager.is_some()
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some() || matches!(self.status, Status::Restart)
    }

    pub fn has_update(&self) -> bool {
        matches!(
            self.status,
            Status::Available(_) | Status::Installing | Status::InstallFailed
        )
    }

    pub fn auto_check_due(&self, now: i64, previous: i64) -> bool {
        self.managed()
            && !self.busy()
            && !self.has_update()
            && (previous > now || now.saturating_sub(previous) >= DAY)
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        status: Status,
        work: impl FnOnce(UpdateManager) -> Result<Status, velopack::Error> + Send + 'static,
    ) -> bool {
        if self.busy() {
            return false;
        }
        let Some(manager) = self.manager.clone() else {
            return false;
        };
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        self.status = status;
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(work(manager).unwrap_or(Status::Failed));
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
        true
    }

    pub fn check(&mut self, ctx: &egui::Context) -> bool {
        self.start(ctx, Status::Checking, |manager| {
            Ok(match manager.check_for_updates()? {
                UpdateCheck::UpdateAvailable(info) => Status::Available(info),
                UpdateCheck::NoUpdateAvailable => Status::Current,
                UpdateCheck::RemoteIsEmpty => Status::Failed,
            })
        })
    }

    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        self.status = match rx.try_recv() {
            Ok(Status::Failed) if matches!(self.status, Status::Installing) => {
                Status::InstallFailed
            }
            Ok(status) => status,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Status::Failed,
        };
        self.pending = None;
        matches!(self.status, Status::Restart)
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        match &self.status {
            Status::Available(info) => {
                let install = ui
                    .horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} {}",
                            Text::UpdateAvailable.get(),
                            info.TargetFullRelease.Version
                        ));
                        ui.button(Text::InstallUpdate.get()).clicked()
                    })
                    .inner;
                if install {
                    let info = info.clone();
                    self.start(ui.ctx(), Status::Installing, move |manager| {
                        manager.download_updates(&info, None)?;
                        manager.wait_exit_then_apply_updates(
                            &*info,
                            false,
                            true,
                            std::iter::empty::<&str>(),
                        )?;
                        Ok(Status::Restart)
                    });
                }
            }
            Status::InstallFailed => {
                ui.label(Text::UpdateFailed.get());
                if ui.button(Text::CheckUpdate.get()).clicked() {
                    self.check(ui.ctx());
                }
            }
            Status::Idle | Status::Restart => {}
            other => {
                let label = match other {
                    Status::Checking => Text::CheckingUpdate,
                    Status::Current => Text::AppCurrent,
                    Status::Installing => Text::InstallingUpdate,
                    _ => Text::UpdateFailed,
                };
                ui.label(label.get());
            }
        }
    }
}
