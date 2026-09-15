use crate::presets::Tile;
use eframe::egui::{self, Key};

pub(super) fn keyboard(ui: &mut egui::Ui, tile: &mut Tile) {
    let delta = ui.input_mut(|input| {
        let mut delta = egui::Vec2::ZERO;
        input.events.retain(|event| {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
                && !modifiers.alt
                && !modifiers.ctrl
                && !modifiers.command
                && !modifiers.mac_cmd
            {
                let direction = match key {
                    Key::ArrowLeft => egui::vec2(-1.0, 0.0),
                    Key::ArrowRight => egui::vec2(1.0, 0.0),
                    Key::ArrowUp => egui::vec2(0.0, -1.0),
                    Key::ArrowDown => egui::vec2(0.0, 1.0),
                    _ => return true,
                };
                delta += direction * if modifiers.shift { 0.1 } else { 1.0 };
                return false;
            }
            true
        });
        delta
    });
    tile.x = (tile.x + delta.x).clamp(0.0, (100.0 - tile.width).max(0.0));
    tile.y = (tile.y + delta.y).clamp(0.0, (100.0 - tile.height).max(0.0));
}

pub(super) fn snap(
    raw: egui::Vec2,
    size: egui::Vec2,
    others: impl Iterator<Item = egui::Rect>,
    tolerance: egui::Vec2,
) -> egui::Vec2 {
    let mut result = raw;
    let mut nearest = tolerance;
    for other in others {
        if raw.y + size.y >= other.top() - tolerance.y && raw.y <= other.bottom() + tolerance.y {
            for edge in [other.left(), other.right()] {
                for offset in [0.0, size.x] {
                    let candidate = edge - offset;
                    let distance = (candidate - raw.x).abs();
                    if candidate >= 0.0 && candidate + size.x <= 100.0 && distance <= nearest.x {
                        result.x = candidate;
                        nearest.x = distance;
                    }
                }
            }
        }
        if raw.x + size.x >= other.left() - tolerance.x && raw.x <= other.right() + tolerance.x {
            for edge in [other.top(), other.bottom()] {
                for offset in [0.0, size.y] {
                    let candidate = edge - offset;
                    let distance = (candidate - raw.y).abs();
                    if candidate >= 0.0 && candidate + size.y <= 100.0 && distance <= nearest.y {
                        result.y = candidate;
                        nearest.y = distance;
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neighbor() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(50.0, 20.0), egui::vec2(30.0, 30.0))
    }

    #[test]
    fn nearby_edges_snap_but_moving_further_can_cross_into_the_neighbor() {
        let size = egui::vec2(20.0, 20.0);
        let tolerance = egui::vec2(1.0, 1.0);
        for (raw_x, expected) in [(29.5, 30.0), (30.5, 30.0), (32.0, 32.0), (40.0, 40.0)] {
            let position = snap(
                egui::vec2(raw_x, 25.0),
                size,
                [neighbor()].into_iter(),
                tolerance,
            );
            assert_eq!(position.x, expected);
            assert_eq!(position.y, 25.0);
        }
    }

    #[test]
    fn distant_tiles_do_not_attract_and_snapping_does_not_go_offscreen() {
        let raw = egui::vec2(29.5, 80.0);
        let size = egui::vec2(20.0, 20.0);
        assert_eq!(
            snap(raw, size, [neighbor()].into_iter(), egui::vec2(1.0, 1.0)),
            raw
        );
        let edge = egui::Rect::from_min_size(egui::pos2(19.5, 20.0), size);
        assert_eq!(
            snap(
                egui::vec2(0.0, 25.0),
                size,
                [edge].into_iter(),
                egui::vec2(1.0, 1.0)
            )
            .x,
            0.0
        );
    }
}
