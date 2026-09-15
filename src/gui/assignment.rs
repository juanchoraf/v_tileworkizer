use super::{App, inventory};
use crate::{
    display_layout::Choice,
    presets::{Preset, WindowBinding},
};
use eframe::egui;

impl App {
    pub(super) fn tile_assignment(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(area) = self.selected_monitor_area() else {
            return;
        };
        let Some(mut tile) = self.draft.tiles.get(index).cloned() else {
            return;
        };
        let local = self
            .inventory
            .as_ref()
            .map(|s| crate::display_scope::for_display(s, area));
        let windows: Vec<_> = local
            .as_ref()
            .map(|s| s.windows.iter().collect())
            .unwrap_or_default();
        let resolution = crate::window_binding::tile_resolutions(&self.draft, &windows)[index];
        let current = resolution.index().map(|index| windows[index]);
        ui.add_space(16.0);
        let mut changed = ui
            .horizontal(|ui| inventory::assignment(ui, &mut tile, local.as_ref(), current))
            .inner;
        if let Some(binding) = &mut tile.window {
            if !binding.app_id.is_empty() {
                ui.collapsing("Reconnect options", |ui| {
                    ui.small(v_concat::v_concat!("Application: {}", binding.app_id));
                    ui.label("Optional stable title fragment to distinguish this app’s windows:");
                    changed |= ui.add(egui::TextEdit::singleline(&mut binding.title_filter)
                        .hint_text("For example, a project name; leave empty for a single window")
                        .desired_width(f32::INFINITY)).changed();
                    ui.small("Used only when reconnecting. Live window title changes keep their assignment.");
                });
            }
        }
        if changed {
            // Use the latest draft so assigning another tile retains earlier edits.
            let template = self.draft.clone();
            self.assign_workspace_window(&template, index, tile.window.clone());
        }
        if let Some(tile) = self.draft.tiles.get(index) {
            let resolution = crate::window_binding::tile_resolutions(&self.draft, &windows)[index];
            inventory::assignment_status(
                ui,
                tile,
                resolution.index().map(|index| windows[index]),
                resolution,
            );
        }
    }

    fn assign_workspace_window(
        &mut self,
        template: &Preset,
        index: usize,
        window: Option<WindowBinding>,
    ) {
        let Some(area) = self.selected_monitor_area() else {
            return;
        };
        let choice = self.selected_layout();
        let selected_live = window
            .as_ref()
            .and_then(|binding| {
                self.inventory.as_ref().and_then(|snapshot| {
                    snapshot
                        .windows
                        .iter()
                        .find(|window| binding.matches(window))
                })
            })
            .cloned();
        if let Choice::Builtin(layout) = choice {
            let before = self.config.standard_assignments.clone();
            if let Some(live) = &selected_live {
                for entry in &mut self.config.standard_assignments {
                    if entry.display == area && entry.layout == layout {
                        for other in &mut entry.windows {
                            if other.as_ref().is_some_and(|binding| binding.matches(live)) {
                                *other = None;
                            }
                        }
                    }
                }
            }
            self.config
                .assign_standard_window(area, layout, index, window);
            if let Err(error) = self.config.validate_standard_assignments() {
                self.config.standard_assignments = before;
                self.notifications.error(error.to_string());
            }
            self.sync_workspace_editor();
            return;
        }
        // A shared preset remains reusable. Assignments on one display must not
        // change the windows assigned on another display using that same preset.
        let shared = matches!(&choice, Choice::Preset(name) if
            self.config.active_preset.as_ref() == Some(name)
                || self.config.display_layouts.iter().any(|entry| entry.display != area && entry.choice == choice));
        let mut updated = template.clone();
        let Some(tile) = updated.tiles.get_mut(index) else {
            return;
        };
        tile.window = window.clone();
        tile.title_match.clear();
        if let Some(binding) = &window {
            for (other_index, tile) in updated.tiles.iter_mut().enumerate() {
                if other_index != index
                    && tile.window.as_ref().is_some_and(|other| {
                        (other.id == binding.id && other.session == binding.session)
                            || selected_live
                                .as_ref()
                                .is_some_and(|live| other.matches(live))
                    })
                {
                    tile.window = None;
                }
            }
        }
        if shared {
            updated.name =
                self.unique_name(&v_concat::v_concat!("Display {} layout", self.monitor + 1));
        }
        if let Err(error) = updated.validate() {
            self.notifications.error(error.to_string());
            return;
        }
        if shared {
            self.editing_name = None;
            self.editor_baseline = None;
        }
        self.draft = updated;
    }
}
