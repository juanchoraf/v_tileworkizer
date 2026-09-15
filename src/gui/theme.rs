use eframe::egui::{self, Color32, RichText};

pub const BACKGROUND: Color32 = Color32::from_rgb(13, 18, 27);
pub const SIDEBAR: Color32 = Color32::from_rgb(17, 24, 35);
pub const SURFACE: Color32 = Color32::from_rgb(23, 32, 45);
pub const SELECTED: Color32 = Color32::from_rgb(28, 56, 59);
pub const ACCENT: Color32 = Color32::from_rgb(83, 224, 186);
pub const AMBER: Color32 = Color32::from_rgb(247, 186, 107);
pub const TEXT: Color32 = Color32::from_rgb(230, 237, 246);
pub const MUTED: Color32 = Color32::from_rgb(140, 158, 181);
pub const BORDER: Color32 = Color32::from_rgb(45, 59, 78);
pub const SLIDER_HANDLE: Color32 = Color32::from_rgb(40, 118, 112); // #287670

pub fn install(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = SIDEBAR;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = SELECTED;
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_fill = SELECTED;
    visuals.widgets.active.bg_fill = SELECTED;
    let corners = egui::CornerRadius::same(6);
    visuals.menu_corner_radius = corners;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = corners;
    }
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 9.0);
        style.spacing.interact_size.y = 34.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
    });
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(16)
}

pub fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(32.0).strong().color(TEXT));
    ui.label(RichText::new(subtitle).color(MUTED));
    ui.add_space(16.0);
}

pub fn primary(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).strong().color(BACKGROUND)).fill(ACCENT))
}

pub fn slider(ui: &mut egui::Ui, label: &str, slider: egui::Slider<'_>) -> egui::Response {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        let style = ui.style_mut();
        style.spacing.slider_rail_height = 6.0;
        // egui uses inactive.bg_fill for both the track and the idle handle.
        // Scope these colors to sliders so cards and other buttons retain theirs.
        let widgets = &mut style.visuals.widgets;
        widgets.inactive.bg_fill = SLIDER_HANDLE;
        widgets.hovered.bg_fill = SLIDER_HANDLE;
        widgets.active.bg_fill = SLIDER_HANDLE;
        widgets.noninteractive.bg_fill = SLIDER_HANDLE;
        ui.add(
            slider
                .trailing_fill(false)
                .fixed_decimals(1)
                .custom_parser(parse_slider_value),
        )
        .labelled_by(label.id)
    })
    .inner
}

fn parse_slider_value(input: &str) -> Option<f64> {
    let input = input.trim();
    let mut dot_seen = false;
    let mut fractional_digits = 0;
    for byte in input.bytes() {
        match byte {
            b'.' if !dot_seen => dot_seen = true,
            b'0'..=b'9' => {
                if dot_seen {
                    fractional_digits += 1;
                    if fractional_digits > 1 {
                        return None;
                    }
                }
            }
            _ => return None,
        }
    }
    input.parse::<f64>().ok().filter(|value| value.is_finite())
}

pub fn tile_color(index: usize) -> Color32 {
    [
        Color32::from_rgb(40, 118, 112),
        Color32::from_rgb(50, 85, 139),
        Color32::from_rgb(105, 76, 140),
        Color32::from_rgb(130, 91, 53),
        Color32::from_rgb(58, 102, 133),
    ][index % 5]
}
