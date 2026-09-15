use crate::{
    platform::{Snapshot, Window},
    presets::{Tile, WindowBinding},
    window_binding::Resolution,
    workspace,
};
use eframe::egui;

pub(super) fn assignment(
    ui: &mut egui::Ui,
    tile: &mut Tile,
    snapshot: Option<&Snapshot>,
    current: Option<&Window>,
) -> bool {
    let before = tile.clone();
    if let Some(binding) = &mut tile.window
        && let Some(window) = current
    {
        // Commit an unambiguous reconnection to its current live identity before
        // editing reconnect rules, so rule edits do not detach the open window.
        if binding.id != window.id || binding.session != window.session {
            binding.id = window.id;
            binding.session = window.session.clone();
            binding.title = window.title.clone();
        }
        if binding.app_id.is_empty() {
            binding.app_id = window.app_id.clone();
        }
    }
    ui.label("Assign Window:");
    let selected = tile
        .window
        .as_ref()
        .map(|w| {
            current
                .map(|window| v_concat::v_concat!("{} · #{}", window.title, window.id))
                .unwrap_or_else(|| v_concat::v_concat!("{} · unavailable", w.title))
        })
        .unwrap_or_else(|| {
            if tile.title_match.is_empty() {
                "Automatic"
            } else {
                "Saved title rule"
            }
            .into()
        });
    egui::ComboBox::from_id_salt("tile_window")
        .selected_text(selected)
        .width(ui.available_width().max(80.0))
        .truncate()
        .show_ui(ui, |ui| {
            if ui
                .selectable_value(&mut tile.window, None, "Automatic")
                .clicked()
            {
                tile.title_match.clear();
            }
            if let Some(snapshot) = snapshot {
                for window in &snapshot.windows {
                    let label = v_concat::v_concat!(
                        "{} · #{} · {}",
                        window.title,
                        window.id,
                        window.status()
                    );
                    ui.add_enabled_ui(!window.state.protected, |ui| {
                        if ui
                            .selectable_label(
                                current.is_some_and(|selected| selected.id == window.id),
                                label,
                            )
                            .on_hover_text(&window.app_id)
                            .clicked()
                        {
                            tile.window = Some(WindowBinding::from_window(window));
                        }
                    });
                }
            }
        });
    *tile != before
}

pub(super) fn assignment_status(
    ui: &mut egui::Ui,
    tile: &Tile,
    current: Option<&Window>,
    resolution: Resolution,
) {
    if let Some(binding) = &tile.window {
        match current {
            Some(w) if !w.eligible() => {
                ui.label("Assigned; placement waits until you restore the window and make its workspace visible.");
            }
            None => {
                if resolution == Resolution::Ambiguous {
                    ui.label("Several windows or tiles match this application. Choose the window again or set a unique reconnect title fragment.");
                } else if binding.app_id.is_empty() {
                    ui.label("Window unavailable. Select it again to enable application-based reconnection.");
                } else {
                    ui.label("Waiting for a matching application window on this display.");
                }
            }
            _ => {}
        }
    }
}

pub(super) fn show(ui: &mut egui::Ui, snapshot: Option<&Snapshot>) {
    ui.collapsing("Detected displays & windows · refreshes automatically", |ui| {
        let Some(snapshot) = snapshot else { ui.label("Waiting for desktop discovery…"); return; };
        for (i, area) in snapshot.monitors.iter().enumerate() {
            ui.label(workspace::display_label(snapshot, i, *area));
        }
        ui.separator();
        for window in &snapshot.windows {
            ui.label(v_concat::v_concat!("{} · #{} · {} · Display {}", window.title, window.id,
                window.status(), workspace::display_index(snapshot, window) + 1));
        }
        if snapshot.windows.is_empty() { ui.label("No accessible application windows detected."); }
        ui.small("Assign windows below the preview in Workspace. Display positions use desktop coordinates.");
    });
}
