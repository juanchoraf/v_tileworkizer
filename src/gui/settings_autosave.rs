use super::{App, theme};
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct SettingsSave {
    due: Option<Instant>,
    result: Option<(bool, String)>,
}

impl SettingsSave {
    pub(super) fn changed(&mut self, ctx: &egui::Context) {
        let delay = Duration::from_millis(300);
        self.due = Some(Instant::now() + delay);
        self.result = None;
        ctx.request_repaint_after(delay);
    }

    pub(super) fn show(&self, ui: &mut egui::Ui) {
        if let Some((saved, message)) = &self.result {
            ui.colored_label(if *saved { theme::ACCENT } else { theme::AMBER }, message);
        } else if self.due.is_some() {
            ui.colored_label(theme::MUTED, "Saving settings…");
        }
    }
}

impl App {
    pub(super) fn autosave_settings(&mut self, ctx: &egui::Context) {
        let Some(due) = self.settings_save.due else {
            return;
        };
        let remaining = due.saturating_duration_since(Instant::now());
        if !remaining.is_zero() {
            ctx.request_repaint_after(remaining);
            return;
        }
        // Commit sliders on release, and text after a brief typing pause.
        if ctx.input(|input| input.pointer.any_down()) {
            return;
        }
        let pending = self.config.clone();
        // Persist only the Settings fields, preserving pending workspace choices.
        self.config = self.saved.clone();
        self.config.gap = pending.gap;
        self.config.master_ratio = pending.master_ratio;
        self.config.poll_ms = pending.poll_ms;
        let result = self.save();
        let excluded_titles = self.config.excluded_titles.clone();
        self.config = pending;
        match result {
            Ok(()) => {
                self.config.excluded_titles = excluded_titles;
                self.settings_save.due = None;
                self.settings_save.result = Some((true, "Settings saved automatically".into()));
            }
            Err(error) => {
                let delay = Duration::from_secs(2);
                self.settings_save.due = Some(Instant::now() + delay);
                self.settings_save.result =
                    Some((false, v_concat::v_concat!("Not saved: {error:#}")));
                ctx.request_repaint_after(delay);
            }
        }
        ctx.request_repaint();
    }
}
