use super::{
    App,
    icons::{self, Icon},
    theme,
};
use crate::presets::{Preset, Tile};
use eframe::egui::{self, RichText};

impl App {
    pub(super) fn new_preset(&mut self) {
        let preset = Preset::from_layout(
            self.unique_name("My workspace"),
            crate::layout::Layout::Grid,
            1,
            self.config.master_ratio,
        );
        self.start_custom_layout(preset);
    }

    fn duplicate_preset(&mut self) {
        let mut preset = self.draft.clone();
        preset.name = self.unique_name("Workspace copy");
        self.start_custom_layout(preset);
    }

    fn start_custom_layout(&mut self, preset: Preset) {
        let Some(area) = self.selected_monitor_area() else {
            return;
        };
        if self.config.presets.len() >= 64 {
            self.notifications
                .error("At most 64 custom presets are supported.");
            return;
        }
        // Select an independent custom layout; the workspace autosaves it below.
        self.config.presets.push(preset.clone());
        self.config.set_display_layout(
            area,
            crate::display_layout::Choice::Preset(preset.name.clone()),
        );
        self.editing_name = Some(preset.name.clone());
        self.draft = preset;
        self.selected_tile = 0;
        self.editor_baseline = None;
        self.editor_context = Some(self.current_editor_context());
    }

    pub(super) fn unique_name(&self, base: &str) -> String {
        let mut name = base.to_owned();
        let mut index = 2;
        while self
            .config
            .presets
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&name))
        {
            name = v_concat::v_concat!("{base} {index}");
            index += 1;
        }
        name
    }

    pub(super) fn save_preset(&mut self) -> bool {
        if self.standard_layout_selected() {
            return false;
        }
        let result = self.draft.validate();
        if result.is_err() {
            self.autosave.record(&result, &self.draft);
            return false;
        }
        let before = self.config.clone();
        if let Some(index) = self
            .editing_name
            .as_ref()
            .and_then(|name| self.config.presets.iter().position(|p| &p.name == name))
        {
            let old_name = self.config.presets[index].name.clone();
            self.config
                .rename_layout_preset(&old_name, &self.draft.name);
            self.config.presets[index] = self.draft.clone();
        } else {
            self.config.presets.push(self.draft.clone());
        }
        if let Some(area) = self.selected_monitor_area() {
            self.config.set_display_layout(
                area,
                crate::display_layout::Choice::Preset(self.draft.name.clone()),
            );
        }
        // Saving this custom layout must not apply pending standard window
        // assignments from other displays that may already be engaged.
        self.config.standard_assignments = self.saved.standard_assignments.clone();
        let result = self.save();
        let saved = result.is_ok();
        if saved {
            self.config.standard_assignments = before.standard_assignments;
            self.editing_name = Some(self.draft.name.clone());
            self.editor_baseline = Some(self.draft.clone());
            self.editor_context = Some(self.current_editor_context());
        } else {
            self.config = before;
        }
        self.autosave.record(&result, &self.draft);
        saved
    }

    pub(super) fn preset_editor(&mut self, ui: &mut egui::Ui) {
        if self.standard_layout_selected() {
            icons::toolbar(ui, |ui| {
                ui.add_enabled_ui(self.selected_monitor_area().is_some(), |ui| {
                    if icons::button(ui, Icon::New).clicked() {
                        self.new_preset();
                    }
                    if icons::button(ui, Icon::Duplicate).clicked() {
                        self.duplicate_preset();
                    }
                });
            });
            return;
        }
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(
                    "Drag tiles to move them. Pull a corner to resize. Higher layers appear in front.",
                )
                .color(theme::MUTED),
            ).on_hover_text("Click the preview, then use arrow keys to move the selected tile. Shift+Arrow moves in smaller steps. Nearby edges snap lightly; keep dragging to overlap tiles.");
        });
        icons::toolbar(ui, |ui| {
            self.tile_actions(ui);
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            if let Some(name) = self.editing_name.clone()
                && icons::button(ui, Icon::Delete).clicked()
            {
                let before = self.config.clone();
                self.config.presets.retain(|p| p.name != name);
                self.config.remove_layout_preset(&name);
                let result = self.save();
                if result.is_ok() {
                    self.editing_name = None;
                    self.editor_baseline = Some(self.draft.clone());
                    self.editor_context = None;
                    self.sync_workspace_editor();
                } else {
                    self.config = before;
                }
                self.report(result, "Layout deleted.");
            }
            if icons::button(ui, Icon::New).clicked() {
                self.new_preset();
            }
            if icons::button(ui, Icon::Duplicate).clicked() {
                self.duplicate_preset();
            }
            let height = ui.spacing().interact_size.y.max(
                ui.text_style_height(&egui::TextStyle::Button)
                    + 2.0 * ui.spacing().button_padding.y,
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.draft.name)
                    .desired_width(220.0)
                    .min_size(egui::vec2(220.0, height))
                    .vertical_align(egui::Align::Center),
            );
        });
    }

    fn tile_actions(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled_ui(self.draft.tiles.len() > 1, |ui| {
                icons::button(ui, Icon::RemoveTile)
            })
            .inner
            .clicked()
        {
            self.draft
                .tiles
                .remove(self.selected_tile.min(self.draft.tiles.len() - 1));
            self.selected_tile = 0;
        }
        if ui
            .add_enabled_ui(self.draft.tiles.len() < 64, |ui| {
                icons::button(ui, Icon::AddTile)
            })
            .inner
            .clicked()
        {
            let i = self.draft.tiles.len();
            self.draft.tiles.push(Tile {
                name: v_concat::v_concat!("Tile {}", i + 1),
                z_index: self
                    .draft
                    .tiles
                    .iter()
                    .map(|t| t.z_index)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1)
                    .min(1000),
                ..Tile::default()
            });
            self.selected_tile = i;
        }
    }
}
