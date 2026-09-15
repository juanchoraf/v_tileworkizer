use super::{App, theme};
use crate::{display_layout::Choice, presets::Preset};
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Autosave {
    result: Option<(bool, String, Instant)>,
    failed_draft: Option<Preset>,
}

impl Autosave {
    pub(super) fn record(&mut self, result: &anyhow::Result<()>, draft: &Preset) {
        self.failed_draft = result.is_err().then(|| draft.clone());
        self.result = Some(match result {
            Ok(()) => (true, "Saved automatically".into(), Instant::now()),
            Err(error) => (
                false,
                v_concat::v_concat!("Not saved: {error:#}"),
                Instant::now(),
            ),
        });
    }

    fn ready(&self, ctx: &egui::Context, draft: &Preset) -> bool {
        // Save a drag on release instead of blocking the UI with disk writes for
        // every pointer movement. Text edits and button actions save immediately.
        if ctx.input(|input| input.pointer.any_down()) {
            return false;
        }
        if let Some((false, _, attempted)) = &self.result
            && self.failed_draft.as_ref() == Some(draft)
        {
            let remaining = Duration::from_secs(2).saturating_sub(attempted.elapsed());
            if !remaining.is_zero() {
                ctx.request_repaint_after(remaining);
                return false;
            }
        }
        true
    }

    pub(super) fn show(&self, ui: &mut egui::Ui) {
        if let Some((saved, message, at)) = &self.result {
            let remaining = Duration::from_secs(3).saturating_sub(at.elapsed());
            if !saved || !remaining.is_zero() {
                ui.colored_label(
                    if *saved {
                        theme::ACCENT
                    } else {
                        egui::Color32::from_rgb(255, 125, 135)
                    },
                    message,
                );
                if *saved {
                    ui.ctx().request_repaint_after(remaining);
                }
            }
        }
    }
}

impl App {
    pub(super) fn autosave_workspace(&mut self, ctx: &egui::Context) {
        if self.editor_context.as_ref() != Some(&self.current_editor_context())
            || !self.autosave.ready(ctx, &self.draft)
        {
            return;
        }
        if !self.standard_layout_selected() && self.editor_baseline.as_ref() != Some(&self.draft) {
            self.save_preset();
            return;
        }
        let Some(area) = self.selected_monitor_area() else {
            return;
        };
        let choice = self.selected_layout();
        let assignments_changed = match choice {
            Choice::Builtin(layout) => {
                self.config.standard_windows(area, layout)
                    != self.saved.standard_windows(area, layout)
            }
            Choice::Preset(_) => false,
        };
        if !assignments_changed && choice == self.saved.layout_for(Some(area)) {
            if self
                .autosave
                .result
                .as_ref()
                .is_some_and(|(saved, _, _)| !saved)
            {
                self.autosave.result = None;
            }
            return;
        }
        let pending = self.config.clone();
        self.config.display_layouts = self.saved.display_layouts.clone();
        self.config.set_display_layout(area, choice.clone());
        self.config.standard_assignments = self.saved.standard_assignments.clone();
        if let Choice::Builtin(layout) = choice {
            self.config
                .standard_assignments
                .retain(|entry| entry.display != area || entry.layout != layout);
            self.config.standard_assignments.extend(
                pending
                    .standard_assignments
                    .iter()
                    .filter(|entry| entry.display == area && entry.layout == layout)
                    .cloned(),
            );
        }
        let result = self.save();
        self.config = pending;
        self.autosave.record(&result, &self.draft);
    }
}
