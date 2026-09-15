mod assignment;
mod autosave;
mod canvas;
mod canvas_input;
mod dashboard;
mod editor_state;
mod icons;
mod identify;
mod inventory;
mod notifications;
mod preset_editor;
mod settings;
mod settings_autosave;
mod theme;
mod tile_editor;
mod update_modal;

use crate::{
    config::Config, layout::Layout, platform::Snapshot, presets::Preset, service, workspace,
};
use anyhow::{Context, Result};
use eframe::egui::{self, RichText};
use std::time::{Duration, Instant};

pub fn run() -> Result<()> {
    let config = Config::load()?;
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo.png"))?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("v_tileworkizer")
            .with_inner_size([1140.0, 1196.0])
            .with_min_inner_size([880.0, 760.0])
            .with_icon(icon.clone()),
        ..Default::default()
    };
    eframe::run_native(
        "v_tileworkizer",
        options,
        Box::new(move |cc| {
            let finish_placement = crate::gui_position::at_pointer(cc);
            theme::install(&cc.egui_ctx);
            let logo = cc.egui_ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                ),
                egui::TextureOptions::LINEAR,
            );
            Ok(Box::new(App {
                exclusions: config.excluded_titles.join("\n"),
                saved: config.clone(),
                config,
                logo,
                finish_placement,
                page: Page::Workspace,
                status: "Connecting to your desktop…".into(),
                notifications: notifications::Notifications::default(),
                refreshed: Instant::now() - Duration::from_secs(10),
                update: None,
                inventory: None,
                monitor: 0,
                draft: Preset::from_layout("My workspace".into(), Layout::Master, 3, 0.6),
                editing_name: None,
                selected_tile: 0,
                editor_context: None,
                editor_baseline: None,
                editor_cache: Vec::new(),
                autosave: autosave::Autosave::default(),
                settings_save: settings_autosave::SettingsSave::default(),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Desktop GUI: {e}"))
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Workspace,
    Settings,
}

struct App {
    config: Config,
    saved: Config,
    logo: egui::TextureHandle,
    finish_placement: Option<Box<dyn FnOnce()>>,
    page: Page,
    status: String,
    notifications: notifications::Notifications,
    refreshed: Instant,
    update: Option<update_modal::UpdateModal>,
    inventory: Option<Snapshot>,
    monitor: usize,
    exclusions: String,
    draft: Preset,
    editing_name: Option<String>,
    selected_tile: usize,
    editor_context: Option<editor_state::Context>,
    editor_baseline: Option<Preset>,
    editor_cache: Vec<editor_state::Cached>,
    autosave: autosave::Autosave,
    settings_save: settings_autosave::SettingsSave,
}

impl App {
    fn save(&mut self) -> Result<()> {
        let _lock = service::lock("config")?.context("Configuration is busy; retry")?;
        anyhow::ensure!(
            Config::load()? == self.saved,
            "Settings changed elsewhere. Reopen the app to load the current settings before saving."
        );
        self.config.excluded_titles = self
            .exclusions
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        self.config.save()?;
        self.saved = self.config.clone();
        Ok(())
    }

    fn report(&mut self, result: Result<()>, success: &str) {
        match result {
            Ok(()) => self.notifications.success(success),
            Err(error) => self.notifications.error(v_concat::v_concat!("{error:#}")),
        }
    }

    fn refresh(&mut self) {
        if self.refreshed.elapsed() >= Duration::from_secs(1) {
            self.status = service::status_text().unwrap_or_else(|e| e.to_string());
            self.inventory = workspace::inventory();
            if let Err(e) = workspace::request_inventory() {
                self.status = e.to_string();
            }
            self.refreshed = Instant::now();
        }
    }

    fn disengage(&mut self) {
        let Some(area) = self.selected_monitor_area() else {
            return;
        };
        let connected = self
            .inventory
            .as_ref()
            .map(|s| s.monitors.as_slice())
            .unwrap_or_default();
        let result = crate::display_pause::pause(area, connected, &self.saved).map(|saved| {
            self.config.enabled = saved.enabled;
            self.config.engaged_displays = saved.engaged_displays.clone();
            self.saved = saved;
        });
        self.report(
            result,
            "Not Enforced for the selected display. Restoring its previous arrangement.",
        );
    }

    fn apply(&mut self, engage: bool) {
        let Some(area) = self.selected_monitor_area() else {
            self.notifications
                .error("Select a connected display first.");
            return;
        };
        let before = self.config.clone();
        let choice = self.selected_layout();
        // Other dropdown selections remain drafts until that display is applied.
        self.config.display_layouts = self.saved.display_layouts.clone();
        self.config.set_display_layout(area, choice.clone());
        self.config.standard_assignments = self.saved.standard_assignments.clone();
        if let crate::display_layout::Choice::Builtin(layout) = choice {
            self.config
                .standard_assignments
                .retain(|entry| entry.display != area || entry.layout != layout);
            self.config.standard_assignments.extend(
                before
                    .standard_assignments
                    .iter()
                    .filter(|entry| entry.display == area && entry.layout == layout)
                    .cloned(),
            );
        }
        let connected = self
            .inventory
            .as_ref()
            .map(|s| s.monitors.as_slice())
            .unwrap_or_default();
        self.config.set_display_engaged(area, engage, connected);
        let result = service::start().and_then(|_| self.save());
        let result = match result {
            Ok(()) => {
                self.config.standard_assignments = before.standard_assignments;
                for entry in before.display_layouts {
                    if entry.display != area {
                        self.config.set_display_layout(entry.display, entry.choice);
                    }
                }
                service::request_display_tile(Some(area))
            }
            Err(error) => {
                self.config = before;
                Err(error)
            }
        };
        self.report(
            result,
            if engage {
                "Apply & Enforce requested for the selected display."
            } else {
                "Arrange Once requested. Automatic arrangement is off for the selected display."
            },
        );
        self.refreshed = Instant::now() - Duration::from_secs(2);
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.image((self.logo.id(), egui::vec2(54.0, 54.0)));
        ui.add_space(12.0);
        ui.label(RichText::new("v_tileworkizer").size(19.0).strong());
        ui.label(
            RichText::new("YOUR DESKTOP, COMPOSED")
                .size(9.0)
                .color(theme::MUTED),
        );
        ui.add_space(34.0);
        for (page, title) in [(Page::Workspace, "Workspace"), (Page::Settings, "Settings")] {
            let selected = self.page == page;
            let button = egui::Button::new(RichText::new(title).color(if selected {
                theme::ACCENT
            } else {
                theme::TEXT
            }))
            .fill(if selected {
                theme::SELECTED
            } else {
                theme::SIDEBAR
            })
            .min_size(egui::vec2(ui.available_width(), 42.0));
            if ui.add(button).clicked() {
                self.page = page;
            }
            ui.add_space(14.0);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(self.update.is_none(), icons::update_button)
                        .inner
                        .clicked()
                    {
                        self.update = Some(update_modal::UpdateModal::start(ui.ctx()));
                    }
                    ui.label(
                        RichText::new(v_concat::v_concat!("v{}", env!("CARGO_PKG_VERSION")))
                            .color(theme::MUTED),
                    );
                });
            });
            ui.separator();
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if let Some(place) = self.finish_placement.take() {
            place();
        }
        self.refresh();
        self.autosave_workspace(ui.ctx());
        identify::show(ui.ctx());
        ui.ctx().request_repaint_after(Duration::from_secs(1));
        egui::Panel::left("navigation")
            .exact_size(218.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::SIDEBAR).inner_margin(20))
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND))
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("page_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::new().inner_margin(28).show(ui, |ui| {
                            if self.inventory.is_none() {
                                ui.colored_label(theme::AMBER, &self.status);
                            }
                            match self.page {
                                Page::Workspace => self.dashboard(ui),
                                Page::Settings => self.settings(ui),
                            }
                        });
                    });
            });
        self.autosave_settings(ui.ctx());
        self.notifications.show(ui.ctx());
        if let Some(update) = &mut self.update
            && update.show(ui.ctx())
        {
            self.update = None;
        }
    }
}
