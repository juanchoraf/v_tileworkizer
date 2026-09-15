use super::{App, theme};
use crate::{config::Config, service};
use eframe::egui;

impl App {
    pub(super) fn settings(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        theme::heading(
            ui,
            "All Displays Settings",
            "Fine-tune spacing, desktop behavior, and the windows you leave alone.",
        );
        theme::card().show(ui, |ui| {
            ui.heading("Spaces");
            changed |= theme::slider(
                ui,
                "Standard layout gap / inset",
                egui::Slider::new(&mut self.config.gap, 0..=100),
            )
            .changed();
            ui.small("Custom layouts use the exact tile borders you draw.");
            changed |= theme::slider(
                ui,
                "Master share",
                egui::Slider::new(&mut self.config.master_ratio, 0.2..=0.8).max_decimals(1),
            )
            .changed();
            changed |= theme::slider(
                ui,
                "Refresh interval (ms)",
                egui::Slider::new(&mut self.config.poll_ms, 250..=30_000).logarithmic(true),
            )
            .changed();
        });
        ui.add_space(12.0);
        theme::card().show(ui, |ui| {
            ui.heading("Leave these windows alone");
            ui.label("One title fragment per line. Matching ignores letter case.");
            changed |= ui.add(egui::TextEdit::multiline(&mut self.exclusions).desired_rows(4).desired_width(f32::INFINITY)).changed();
            ui.label("The organizer itself, minimized windows, and fullscreen windows are excluded automatically where supported.");
        });
        ui.add_space(ui.spacing().item_spacing.y * 2.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Reset to Defaults").clicked() {
                let defaults = Config::default();
                self.config.gap = defaults.gap;
                self.config.master_ratio = defaults.master_ratio;
                self.config.poll_ms = defaults.poll_ms;
                self.exclusions = defaults.excluded_titles.join("\n");
                self.config.excluded_titles = defaults.excluded_titles;
                changed = true;
            }
            if ui.button("Restart service if stopped").clicked() {
                let result = service::start();
                self.report(result, "Service start requested");
            }
        });
        if changed {
            self.settings_save.changed(ui.ctx());
        }
        self.settings_save.show(ui);
    }
}
