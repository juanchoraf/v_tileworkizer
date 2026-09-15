use super::{App, theme};
use crate::layout::Layout;
use eframe::egui::{self, RichText};

impl App {
    pub(super) fn dashboard(&mut self, ui: &mut egui::Ui) {
        theme::heading(
            ui,
            "Your space. Your rules.",
            "Choose a standard layout or build a workspace that fits the way you think.",
        );
        ui.horizontal(|ui| {
            let monitors = self.inventory.as_ref().map_or(0, |s| s.monitors.len());
            let windows = self.inventory.as_ref().map_or(0, |s| s.windows.len());
            for (value, caption) in [
                (monitors, "DISPLAYS"),
                (windows, "WINDOWS"),
                (self.config.presets.len(), "SAVED PRESETS"),
            ] {
                theme::card().show(ui, |ui| {
                    ui.set_min_width(110.0);
                    ui.label(RichText::new(value.to_string()).size(26.0).strong());
                    ui.label(RichText::new(caption).size(10.0).color(theme::MUTED));
                });
            }
        });
        super::inventory::show(ui, self.inventory.as_ref());
        ui.add_space(9.0);
        theme::card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let count = self.inventory.as_ref().map_or(0, |s| s.monitors.len());
                self.monitor = self.monitor.min(count.saturating_sub(1));
                egui::ComboBox::from_id_salt("preview_monitor")
                    .selected_text(
                        self.inventory
                            .as_ref()
                            .filter(|s| !s.monitors.is_empty())
                            .map(|s| s.display_name(self.monitor))
                            .unwrap_or_else(|| "Waiting for displays…".into()),
                    )
                    .show_ui(ui, |ui| {
                        if let Some(snapshot) = &self.inventory {
                            for index in 0..count {
                                ui.selectable_value(
                                    &mut self.monitor,
                                    index,
                                    snapshot.display_name(index),
                                );
                            }
                        }
                    });
                if ui
                    .add_enabled(count > 0, egui::Button::new("Identify"))
                    .clicked()
                    && let Some(snapshot) = &self.inventory
                {
                    super::identify::start(ui.ctx(), snapshot);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(count > 0, |ui| self.layout_picker(ui));
                    ui.label("Layout:");
                });
            });
            self.sync_workspace_editor();
            self.workspace_canvas(ui);
            self.preset_editor(ui);
            self.tile_sections(ui);
            self.autosave_workspace(ui.ctx());
            let monitor = self.selected_monitor_area();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(monitor.is_some(), |ui| {
                        if theme::primary(ui, "Apply & Enforce").clicked() {
                            self.apply_workspace(true);
                        }
                        if ui.button("Arrange Once").clicked() {
                            self.apply_workspace(false);
                        }
                        if ui.add(egui::Button::new(
                            RichText::new("Not Enforce").color(theme::BACKGROUND).strong()
                        ).fill(theme::AMBER))
                            .on_hover_text("Stop automatic arrangement and restore this display’s previous arrangement")
                            .clicked() {
                            self.disengage();
                        }
                    });
                    self.autosave.show(ui);
                });
            });
        });
    }

    pub(super) fn selected_monitor_area(&self) -> Option<crate::layout::Rect> {
        self.inventory
            .as_ref()
            .and_then(|s| s.monitors.get(self.monitor))
            .copied()
    }

    pub(super) fn selected_layout(&self) -> crate::display_layout::Choice {
        self.config.layout_for(self.selected_monitor_area())
    }

    fn layout_picker(&mut self, ui: &mut egui::Ui) {
        use crate::display_layout::Choice;
        let mut choice = self.selected_layout();
        let before = choice.clone();
        egui::ComboBox::from_id_salt("display_layout")
            .width(150.0)
            .selected_text(choice.label())
            .show_ui(ui, |ui| {
                ui.label(
                    RichText::new("Standard Layouts")
                        .strong()
                        .color(theme::MUTED),
                );
                ui.separator();
                for layout in [Layout::Master, Layout::Columns, Layout::Rows, Layout::Grid] {
                    let value = Choice::Builtin(layout);
                    let label = value.label().to_owned();
                    ui.selectable_value(&mut choice, value, label);
                }
                ui.separator();
                ui.label(RichText::new("Custom Layouts").strong().color(theme::MUTED));
                for preset in &self.config.presets {
                    ui.selectable_value(
                        &mut choice,
                        Choice::Preset(preset.name.clone()),
                        &preset.name,
                    );
                }
            });
        if choice != before
            && let Some(area) = self.selected_monitor_area()
        {
            self.config.set_display_layout(area, choice);
        }
    }

    pub(super) fn monitor_windows(&self) -> Vec<crate::platform::Window> {
        self.inventory
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .windows
                    .iter()
                    .filter(|window| {
                        if !window.eligible() {
                            return false;
                        }
                        snapshot
                            .monitors
                            .iter()
                            .position(|m| {
                                m.contains(
                                    window.rect.x + window.rect.width / 2,
                                    window.rect.y + window.rect.height / 2,
                                )
                            })
                            .unwrap_or(0)
                            == self.monitor
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}
