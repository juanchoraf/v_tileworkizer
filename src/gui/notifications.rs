use eframe::egui::{self, Color32};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const LIFETIME: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Success,
    Error,
}

struct Notification {
    kind: Kind,
    message: String,
    expires_at: Instant,
}

#[derive(Default)]
pub(super) struct Notifications {
    entries: VecDeque<Notification>,
}

impl Notifications {
    pub(super) fn success(&mut self, message: impl Into<String>) {
        self.push(Kind::Success, message.into(), Instant::now());
    }

    pub(super) fn error(&mut self, message: impl Into<String>) {
        self.push(Kind::Error, message.into(), Instant::now());
    }

    fn push(&mut self, kind: Kind, message: String, now: Instant) {
        if !message.trim().is_empty() {
            self.entries.push_back(Notification {
                kind,
                message,
                expires_at: now + LIFETIME,
            });
        }
    }

    fn expire(&mut self, now: Instant) {
        self.entries.retain(|entry| entry.expires_at > now);
    }

    pub(super) fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.expire(now);
        let Some(first) = self.entries.front() else {
            return;
        };
        ctx.request_repaint_after(first.expires_at.saturating_duration_since(now));
        let bounds = ctx.content_rect();
        let width = (bounds.width() - 32.0).clamp(1.0, 400.0);
        egui::Area::new(egui::Id::new("app_notifications"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .order(egui::Order::Foreground)
            .movable(false)
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::ScrollArea::vertical()
                    .id_salt("notification_stack")
                    .max_height((bounds.height() - 32.0).max(1.0))
                    .auto_shrink([false, true])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // Align each bubble to the right, including short messages
                        // that do not fill the notification stack's maximum width.
                        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                            // Append in arrival order: older messages above newer ones.
                            for entry in &self.entries {
                                let (color, background) = match entry.kind {
                                    Kind::Success => (
                                        Color32::from_rgb(112, 225, 151),
                                        Color32::from_rgb(20, 45, 32),
                                    ),
                                    Kind::Error => (
                                        Color32::from_rgb(255, 125, 135),
                                        Color32::from_rgb(53, 24, 30),
                                    ),
                                };
                                egui::Frame::new()
                                    .fill(background)
                                    .stroke(egui::Stroke::new(1.0, color))
                                    .corner_radius(6)
                                    .inner_margin(12)
                                    .show(ui, |ui| {
                                        let icon_width = 20.0 + ui.spacing().item_spacing.x;
                                        let text = ui.painter().layout(
                                            entry.message.clone(),
                                            egui::TextStyle::Body.resolve(ui.style()),
                                            color,
                                            (ui.available_width() - icon_width).max(1.0),
                                        );
                                        let size = egui::vec2(
                                            icon_width + text.size().x,
                                            text.size().y.max(20.0),
                                        );
                                        ui.allocate_ui_with_layout(
                                            size,
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                draw_icon(ui, entry.kind, color);
                                                ui.add(egui::Label::new(text));
                                            },
                                        );
                                    });
                            }
                        });
                    });
            });
    }
}

fn draw_icon(ui: &mut egui::Ui, kind: Kind, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    let point = |x, y| rect.min + egui::vec2(x, y);
    let stroke = egui::Stroke::new(2.5, color);
    let segments = match kind {
        Kind::Success => [
            [point(3.0, 10.0), point(8.0, 15.0)],
            [point(8.0, 15.0), point(17.0, 5.0)],
        ],
        Kind::Error => [
            [point(5.0, 5.0), point(15.0, 15.0)],
            [point(5.0, 15.0), point(15.0, 5.0)],
        ],
    };
    for segment in segments {
        ui.painter().line_segment(segment, stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_keep_their_order_kind_and_independent_fifteen_second_lifetime() {
        let start = Instant::now();
        let mut notifications = Notifications::default();
        notifications.push(Kind::Success, "Saved".into(), start);
        notifications.push(Kind::Error, "Failed".into(), start + Duration::from_secs(5));
        notifications.expire(start + Duration::from_secs(14));
        assert_eq!(notifications.entries.len(), 2);
        assert_eq!(notifications.entries[0].message, "Saved");
        assert_eq!(notifications.entries[0].kind, Kind::Success);
        assert_eq!(notifications.entries[1].message, "Failed");
        assert_eq!(notifications.entries[1].kind, Kind::Error);
        notifications.expire(start + LIFETIME);
        assert_eq!(notifications.entries.len(), 1);
        assert_eq!(notifications.entries[0].message, "Failed");
        notifications.expire(start + Duration::from_secs(20));
        assert!(notifications.entries.is_empty());
    }

    #[test]
    fn repeated_messages_are_separate_and_empty_messages_are_ignored() {
        let now = Instant::now();
        let mut notifications = Notifications::default();
        notifications.push(Kind::Success, "Saved".into(), now);
        notifications.push(Kind::Success, "Saved".into(), now + Duration::from_secs(1));
        notifications.push(Kind::Success, " ".into(), now);
        assert_eq!(notifications.entries.len(), 2);
    }
}
