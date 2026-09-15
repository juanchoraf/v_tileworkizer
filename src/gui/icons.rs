use eframe::egui::{self, Color32};

pub(super) enum Icon {
    Duplicate,
    New,
    Delete,
    AddTile,
    RemoveTile,
}

pub(super) fn toolbar(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    // Bound the right-aligned layout to one row instead of the remaining panel height.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), contents);
    });
}

pub(super) fn update_button(ui: &mut egui::Ui) -> egui::Response {
    let text = ui.painter().layout_no_wrap(
        "Update".into(),
        egui::TextStyle::Button.resolve(ui.style()),
        Color32::PLACEHOLDER,
    );
    let content_width = 18.0 + 6.0 + text.size().x;
    let padding = ui.spacing().button_padding;
    let size = egui::vec2(
        content_width + 2.0 * padding.x,
        (text.size().y.max(18.0) + 2.0 * padding.y).max(ui.spacing().interact_size.y),
    );
    let response = ui.add_sized(
        size,
        egui::Button::new("").fill(Color32::from_rgb(0x1A, 0x20, 0x2A)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Update")
    });
    if ui.is_rect_visible(response.rect) {
        let color = if response.hovered() {
            Color32::from_rgb(0x6A, 0xE0, 0xB5)
        } else {
            ui.style().interact(&response).text_color()
        };
        let origin = response.rect.center() - egui::vec2(content_width / 2.0, 9.0);
        let painter = ui.painter();
        painter.galley(
            egui::pos2(
                origin.x + 24.0,
                response.rect.center().y - text.size().y / 2.0,
            ),
            text,
            color,
        );
        let line = |points: &[[f32; 2]]| {
            painter.add(egui::Shape::line(
                points
                    .iter()
                    .map(|p| origin + egui::vec2(p[0], p[1]))
                    .collect(),
                egui::Stroke::new(1.7, color),
            ));
        };
        line(&[
            [5.0, 12.0],
            [3.5, 12.0],
            [1.8, 11.3],
            [1.0, 9.5],
            [1.3, 7.8],
            [2.5, 6.5],
            [4.0, 6.0],
            [4.5, 3.8],
            [6.2, 2.2],
            [8.5, 1.7],
            [10.8, 2.3],
            [12.3, 4.0],
            [12.8, 6.0],
            [14.5, 6.0],
            [16.2, 7.0],
            [17.0, 9.0],
            [16.5, 10.8],
            [15.0, 12.0],
            [13.0, 12.0],
        ]);
        line(&[[9.0, 8.0], [9.0, 17.0]]);
        line(&[[5.5, 13.5], [9.0, 17.0], [12.5, 13.5]]);
    }
    response.on_hover_text("Check for updates")
}

pub(super) fn button(ui: &mut egui::Ui, icon: Icon) -> egui::Response {
    let label = match icon {
        Icon::Duplicate => "Duplicate Layout",
        Icon::New => "Add Layout",
        Icon::Delete => "Delete Layout",
        Icon::AddTile => "Add tile",
        Icon::RemoveTile => "Delete Tile",
    };
    let fill = match icon {
        Icon::Duplicate => Color32::from_rgb(43, 99, 170),
        Icon::New | Icon::AddTile => Color32::from_rgb(27, 116, 83),
        Icon::Delete | Icon::RemoveTile => Color32::from_rgb(190, 59, 74),
    };
    let tile_text = match icon {
        Icon::AddTile => Some("+ Tile"),
        _ => None,
    };
    if let Some(text) = tile_text {
        return ui
            .add(egui::Button::new(egui::RichText::new(text).color(Color32::WHITE)).fill(fill))
            .on_hover_text(label)
            .on_disabled_hover_text(label);
    }
    let button = egui::Button::new("").fill(fill);
    let text = ui.painter().layout_no_wrap(
        if matches!(icon, Icon::RemoveTile) {
            "Tile"
        } else {
            "Layout"
        }
        .into(),
        egui::TextStyle::Button.resolve(ui.style()),
        Color32::WHITE,
    );
    let content_width = 18.0 + 8.0 + text.size().x;
    let width = content_width + 2.0 * ui.spacing().button_padding.x;
    let size = ui.spacing().interact_size.y.max(34.0);
    let response = ui.add_sized(egui::vec2(width, size), button);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if ui.is_rect_visible(response.rect) {
        let painter = ui.painter();
        let origin = response.rect.center() - egui::vec2(content_width / 2.0, 9.0);
        painter.galley(
            egui::pos2(
                origin.x + 26.0,
                response.rect.center().y - text.size().y / 2.0,
            ),
            text,
            Color32::WHITE,
        );
        let stroke = egui::Stroke::new(1.7, Color32::WHITE);
        let line = |points: &[[f32; 2]]| {
            painter.add(egui::Shape::line(
                points
                    .iter()
                    .map(|p| origin + egui::vec2(p[0], p[1]))
                    .collect(),
                stroke,
            ));
        };
        match icon {
            Icon::Duplicate => {
                line(&[
                    [5.0, 5.0],
                    [5.0, 1.0],
                    [17.0, 1.0],
                    [17.0, 13.0],
                    [13.0, 13.0],
                ]);
                line(&[
                    [1.0, 5.0],
                    [13.0, 5.0],
                    [13.0, 17.0],
                    [1.0, 17.0],
                    [1.0, 5.0],
                ]);
            }
            Icon::New | Icon::AddTile => {
                line(&[[9.0, 2.0], [9.0, 16.0]]);
                line(&[[2.0, 9.0], [16.0, 9.0]]);
            }
            Icon::Delete | Icon::RemoveTile => {
                line(&[[2.0, 4.0], [16.0, 4.0]]);
                line(&[[6.0, 4.0], [6.0, 1.0], [12.0, 1.0], [12.0, 4.0]]);
                line(&[[4.0, 6.0], [5.0, 17.0], [13.0, 17.0], [14.0, 6.0]]);
                line(&[[7.0, 7.0], [7.5, 14.0]]);
                line(&[[11.0, 7.0], [10.5, 14.0]]);
            }
        }
    }
    response.on_hover_text(label).on_disabled_hover_text(label)
}
