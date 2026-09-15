use crate::update::{self, Progress};
use eframe::egui;
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Debug)]
enum Event {
    Progress(Progress),
    Finished(Result<String, String>),
}

pub(super) struct UpdateModal {
    receiver: Receiver<Event>,
    progress: Progress,
    outcome: Option<Result<String, String>>,
}

impl UpdateModal {
    pub(super) fn start(ctx: &egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let task = std::thread::Builder::new()
            .name("app-update".into())
            .spawn(move || {
                let result = update::execute_with_progress(true, |progress| {
                    let _ = sender.send(Event::Progress(progress));
                    ctx.request_repaint();
                })
                .map_err(|error| v_concat::v_concat!("{error:#}"));
                let _ = sender.send(Event::Finished(result));
                ctx.request_repaint();
            });
        Self {
            receiver,
            progress: Progress::Checking,
            outcome: task
                .err()
                .map(|error| Err(v_concat::v_concat!("Cannot start updater: {error}"))),
        }
    }

    fn poll(&mut self) {
        while self.outcome.is_none() {
            match self.receiver.try_recv() {
                Ok(Event::Progress(progress)) => self.progress = progress,
                Ok(Event::Finished(result)) => self.outcome = Some(result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.outcome =
                        Some(Err("The update task ended unexpectedly. Try again.".into()));
                }
            }
        }
    }

    /// Returns true only when the user acknowledges a completed result.
    pub(super) fn show(&mut self, ctx: &egui::Context) -> bool {
        self.poll();
        egui::Modal::new(egui::Id::new("application_update_modal"))
            .show(ctx, |ui| {
                ui.set_width(430.0);
                ui.heading("Update v_tileworkizer");
                ui.small(v_concat::v_concat!(
                    "Current version: {}",
                    env!("CARGO_PKG_VERSION")
                ));
                ui.small("Source: github.com/juanchoraf/v_tileworkizer");
                ui.add_space(16.0);
                if let Some(outcome) = &self.outcome {
                    match outcome {
                        Ok(message) => {
                            ui.label(message);
                        }
                        Err(message) => {
                            ui.colored_label(super::theme::AMBER, "Update could not be completed");
                            egui::ScrollArea::vertical()
                                .max_height(220.0)
                                .show(ui, |ui| {
                                    ui.label(message);
                                });
                        }
                    }
                    ui.add_space(16.0);
                    return ui.button("Ok").clicked();
                }
                let bar = match self.progress {
                    Progress::Checking => egui::ProgressBar::new(0.0)
                        .animate(true)
                        .text("Checking for updates…"),
                    Progress::Downloading { received, total } => {
                        ui.label(v_concat::v_concat!(
                            "Downloading: {:.1} / {:.1} MiB",
                            received as f64 / 1_048_576.0,
                            total as f64 / 1_048_576.0
                        ));
                        egui::ProgressBar::new(received as f32 / total.max(1) as f32)
                            .show_percentage()
                    }
                    Progress::Verifying => egui::ProgressBar::new(0.0)
                        .animate(true)
                        .text("Verifying download…"),
                    Progress::Installing => {
                        ui.label("Approve the system administrator prompt if requested.");
                        egui::ProgressBar::new(0.0)
                            .animate(true)
                            .text("Installing update…")
                    }
                };
                ui.add(bar.desired_width(ui.available_width()));
                false
            })
            .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_update_result_remains_available_after_worker_exits() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Event::Finished(Ok("No updates available.".into())))
            .unwrap();
        drop(sender);
        let mut modal = UpdateModal {
            receiver,
            progress: Progress::Checking,
            outcome: None,
        };
        modal.poll();
        modal.poll();
        assert_eq!(modal.outcome, Some(Ok("No updates available.".into())));
    }

    #[test]
    fn unexpected_worker_exit_becomes_a_visible_error() {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        let mut modal = UpdateModal {
            receiver,
            progress: Progress::Checking,
            outcome: None,
        };
        modal.poll();
        assert!(modal.outcome.unwrap().is_err());
    }
}
