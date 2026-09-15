use super::{App, theme};
use crate::presets::Tile;
use eframe::egui;

impl App {
    pub(super) fn tile_sections(&mut self, ui: &mut egui::Ui) {
        let visible = self.visible_editor_tiles();
        if visible.is_empty() {
            ui.label("This layout has no tiles on the selected display.");
            return;
        }
        let editable = !self.standard_layout_selected();
        if !visible.contains(&self.selected_tile) {
            self.selected_tile = visible[0];
        }
        ui.horizontal_wrapped(|ui| {
            for &index in &visible {
                let tile = &self.draft.tiles[index];
                ui.selectable_value(
                    &mut self.selected_tile,
                    index,
                    v_concat::v_concat!("{} · z{}", tile.name, tile.z_index),
                );
            }
        });
        let index = self.selected_tile;
        ui.push_id(("tile_section", self.monitor, index), |ui| {
            theme::card().show(ui, |ui| {
                let tile = &mut self.draft.tiles[index];
                if editable {
                    geometry(ui, tile);
                } else {
                    ui.strong(&tile.name);
                }
                self.tile_assignment(ui, index);
            });
        });
    }
}

fn geometry(ui: &mut egui::Ui, tile: &mut Tile) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        ui.label("Tile name");
        let height = ui.spacing().interact_size.y.max(
            ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y,
        );
        ui.add(
            egui::TextEdit::singleline(&mut tile.name)
                .min_size(egui::vec2(0.0, height))
                .vertical_align(egui::Align::Center),
        );
        ui.add_space(24.0);
        ui.label("Layer");
        ui.scope(|ui| {
            let widgets = &mut ui.visuals_mut().widgets;
            widgets.inactive.bg_fill = theme::SELECTED;
            widgets.inactive.weak_bg_fill = theme::SELECTED;
            widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT);
            widgets.hovered.bg_fill = theme::SLIDER_HANDLE;
            widgets.hovered.weak_bg_fill = theme::SLIDER_HANDLE;
            widgets.active.bg_fill = theme::SLIDER_HANDLE;
            widgets.active.weak_bg_fill = theme::SLIDER_HANDLE;
            ui.add(egui::DragValue::new(&mut tile.z_index).range(-1000..=1000));
        });
    });
    ui.add_space(12.0);
    let rail_width = ((ui.available_width() - 3.0 * ui.spacing().item_spacing.x) / 4.0 - 150.0)
        .clamp(48.0, 120.0);
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = rail_width;
        // Reserve enough room for 100.0, including the field padding,
        // so the numeric controls keep the same width as values change.
        ui.spacing_mut().interact_size.x = 76.0;
        ui.horizontal(|ui| {
            theme::slider(
                ui,
                "Left %",
                egui::Slider::new(&mut tile.x, 0.0..=(100.0 - tile.width).max(0.0)).max_decimals(1),
            );
            theme::slider(
                ui,
                "Top %",
                egui::Slider::new(&mut tile.y, 0.0..=(100.0 - tile.height).max(0.0))
                    .max_decimals(1),
            );
            theme::slider(
                ui,
                "Width %",
                egui::Slider::new(&mut tile.width, 1.0..=(100.0 - tile.x).max(1.0)).max_decimals(1),
            );
            theme::slider(
                ui,
                "Height %",
                egui::Slider::new(&mut tile.height, 1.0..=(100.0 - tile.y).max(1.0))
                    .max_decimals(1),
            );
        });
    });
}
