//! Temporary native viewport labels; use precisely the snapshot order shown in the dropdown.
use crate::platform::Snapshot;
use eframe::egui::{self, Color32, RichText};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Identification {
    until: Instant,
    snapshot: Snapshot,
}
fn key() -> egui::Id {
    egui::Id::new("display_identification")
}

pub(super) fn start(ctx: &egui::Context, snapshot: &Snapshot) {
    ctx.data_mut(|data| {
        data.insert_temp(
            key(),
            Identification {
                until: Instant::now() + Duration::from_secs(5),
                snapshot: snapshot.clone(),
            },
        )
    });
    ctx.request_repaint();
}

pub(super) fn show(ctx: &egui::Context) {
    let Some(state) = ctx.data(|data| data.get_temp::<Identification>(key())) else {
        return;
    };
    if Instant::now() >= state.until {
        ctx.data_mut(|data| data.remove::<Identification>(key()));
        return; // Dropping immediate viewports closes their native windows.
    }
    ctx.request_repaint_after(Duration::from_millis(100));
    for (index, area) in state.snapshot.monitors.iter().enumerate() {
        let label = state.snapshot.display_name(index);
        let position =
            |scale: f32| egui::pos2(area.x as f32 / scale + 16.0, area.y as f32 / scale + 16.0);
        let units = if cfg!(target_os = "macos") {
            1.0
        } else {
            ctx.pixels_per_point()
        };
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(("identify_display", index)),
            egui::ViewportBuilder::default()
                .with_title(v_concat::v_concat!("v_tileworkizer Identify {}", index + 1))
                .with_inner_size([290.0, 150.0])
                .with_position(position(units))
                .with_decorations(false)
                .with_resizable(false)
                .with_active(false)
                .with_mouse_passthrough(true)
                .with_window_level(egui::WindowLevel::AlwaysOnTop),
            |ui, _| {
                // Reposition using this viewport's actual destination DPI, not the main window's.
                let units = if cfg!(target_os = "macos") {
                    1.0
                } else {
                    ui.ctx().pixels_per_point()
                };
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::OuterPosition(position(units)));
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(Color32::from_rgb(16, 30, 43))
                            .inner_margin(16),
                    )
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(v_concat::v_concat!("{}", index + 1))
                                .size(68.0)
                                .strong()
                                .color(super::theme::ACCENT),
                        );
                        ui.label(RichText::new(&label).size(19.0).color(Color32::WHITE));
                    });
            },
        );
    }
}
