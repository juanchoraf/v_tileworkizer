use super::{canvas_input, theme};
use crate::presets::Preset;
use eframe::egui::{self, Color32};

const CANVAS_INSET: f32 = 18.0;

pub fn monitor_height(width: f32, monitor: Option<crate::layout::Rect>) -> f32 {
    let ratio = monitor
        .filter(|area| area.width > 0 && area.height > 0)
        .map_or(9.0 / 16.0, |area| area.height as f32 / area.width as f32);
    // Match the dotted drawing area's proportions, accounting for its border inset.
    (width - 2.0 * CANVAS_INSET).max(1.0) * ratio + 2.0 * CANVAS_INSET
}

pub fn show(
    ui: &mut egui::Ui,
    preset: &mut Preset,
    selected: &mut usize,
    editable: bool,
    height: f32,
    captions: &[Option<String>],
) {
    let (frame, focus) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    if focus.clicked() {
        focus.request_focus();
    }
    if editable
        && focus.has_focus()
        && !ui.input(|input| input.pointer.any_down())
        && let Some(tile) = preset.tiles.get_mut(*selected)
    {
        canvas_input::keyboard(ui, tile);
    }
    let painter = ui.painter().with_clip_rect(frame);
    painter.rect_filled(frame, 10.0, theme::BACKGROUND);
    let canvas = frame.shrink(CANVAS_INSET);
    for x in 0..=20 {
        for y in 0..=10 {
            painter.circle_filled(
                canvas.min
                    + egui::vec2(
                        canvas.width() * x as f32 / 20.0,
                        canvas.height() * y as f32 / 10.0,
                    ),
                1.0,
                theme::BORDER,
            );
        }
    }
    let mut order: Vec<_> = (0..preset.tiles.len()).collect();
    order.sort_by_key(|i| preset.tiles[*i].z_index);
    let borders: Vec<_> = preset
        .tiles
        .iter()
        .map(|tile| {
            egui::Rect::from_min_size(
                egui::pos2(tile.x, tile.y),
                egui::vec2(tile.width, tile.height),
            )
        })
        .collect();
    for i in order {
        let tile = &mut preset.tiles[i];
        let rect = egui::Rect::from_min_size(
            canvas.min
                + egui::vec2(
                    canvas.width() * tile.x / 100.0,
                    canvas.height() * tile.y / 100.0,
                ),
            egui::vec2(
                canvas.width() * tile.width / 100.0,
                canvas.height() * tile.height / 100.0,
            ),
        );
        painter.rect_filled(
            rect.translate(egui::vec2(3.0, 5.0)),
            7.0,
            Color32::from_black_alpha(80),
        );
        painter.rect_filled(rect, 7.0, theme::tile_color(i));
        painter.rect_stroke(
            rect,
            7.0,
            egui::Stroke::new(
                if *selected == i { 2.0 } else { 1.0 },
                if *selected == i {
                    theme::ACCENT
                } else {
                    theme::BORDER
                },
            ),
            egui::StrokeKind::Inside,
        );
        let text = painter.with_clip_rect(rect.shrink(5.0));
        text.text(
            rect.min + egui::vec2(12.0, 10.0),
            egui::Align2::LEFT_TOP,
            &tile.name,
            egui::FontId::proportional(14.0),
            theme::TEXT,
        );
        text.text(
            rect.min + egui::vec2(12.0, 29.0),
            egui::Align2::LEFT_TOP,
            v_concat::v_concat!("LAYER {}", tile.z_index),
            egui::FontId::monospace(10.0),
            theme::ACCENT,
        );
        let caption = captions
            .get(i)
            .and_then(|caption| caption.as_deref())
            .unwrap_or_else(|| {
                tile.window
                    .as_ref()
                    .map(|w| w.title.as_str())
                    .unwrap_or(&tile.title_match)
            });
        if !caption.is_empty() && rect.height() > 70.0 {
            text.text(
                rect.min + egui::vec2(12.0, 49.0),
                egui::Align2::LEFT_TOP,
                caption,
                egui::FontId::proportional(11.0),
                theme::TEXT,
            );
        }
        if !editable
            && ui
                .interact(
                    rect,
                    ui.id().with(("preview_tile", i)),
                    egui::Sense::click(),
                )
                .clicked()
        {
            *selected = i;
        }
        if editable {
            let movement = ui.interact(
                rect,
                ui.id().with(("tile", i)),
                egui::Sense::click_and_drag(),
            );
            let corner = egui::Rect::from_min_max(rect.max - egui::vec2(18.0, 18.0), rect.max);
            let resize = ui.interact(corner, ui.id().with(("resize", i)), egui::Sense::drag());
            painter.line_segment(
                [
                    rect.max - egui::vec2(12.0, 4.0),
                    rect.max - egui::vec2(4.0, 12.0),
                ],
                (2.0, theme::TEXT),
            );
            if movement.clicked() || movement.drag_started() || resize.drag_started() {
                *selected = i;
                focus.request_focus();
            }
            let raw_id = movement.id.with("unsnapped_position");
            if movement.drag_started() {
                ui.data_mut(|data| data.insert_temp(raw_id, egui::vec2(tile.x, tile.y)));
            }
            let delta = ui.input(|input| input.pointer.delta());
            if resize.dragged() {
                tile.width = (tile.width + delta.x / canvas.width() * 100.0)
                    .clamp(1.0, (100.0 - tile.x).max(1.0));
                tile.height = (tile.height + delta.y / canvas.height() * 100.0)
                    .clamp(1.0, (100.0 - tile.y).max(1.0));
            } else if movement.dragged() {
                // Accumulate pointer movement independently of the snapped position.
                // Continuing past the six-point attraction zone releases the edge.
                let mut raw = ui
                    .data_mut(|data| data.get_temp::<egui::Vec2>(raw_id))
                    .unwrap_or(egui::vec2(tile.x, tile.y));
                raw.x = (raw.x + delta.x / canvas.width() * 100.0)
                    .clamp(0.0, (100.0 - tile.width).max(0.0));
                raw.y = (raw.y + delta.y / canvas.height() * 100.0)
                    .clamp(0.0, (100.0 - tile.height).max(0.0));
                ui.data_mut(|data| data.insert_temp(raw_id, raw));
                let position = canvas_input::snap(
                    raw,
                    egui::vec2(tile.width, tile.height),
                    borders
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != i)
                        .map(|(_, rect)| *rect),
                    egui::vec2(600.0 / canvas.width(), 600.0 / canvas.height()),
                );
                tile.x = position.x;
                tile.y = position.y;
            }
            if !ui.input(|input| input.pointer.any_down()) {
                ui.data_mut(|data| data.remove::<egui::Vec2>(raw_id));
            }
        }
    }
    if editable && focus.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                focus.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
    }
}
