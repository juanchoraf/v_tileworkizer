use super::{App, canvas};
use crate::{display_layout::Choice, layout::Rect, presets::Preset};
use eframe::egui;

#[derive(Clone, PartialEq)]
pub(super) struct Context {
    display: Option<Rect>,
    choice: Choice,
}

pub(super) struct Cached {
    context: Context,
    draft: Preset,
    baseline: Option<Preset>,
    editing_name: Option<String>,
    selected_tile: usize,
}

impl App {
    pub(super) fn current_editor_context(&self) -> Context {
        Context {
            display: self.selected_monitor_area(),
            choice: self.selected_layout(),
        }
    }

    pub(super) fn sync_workspace_editor(&mut self) {
        let context = self.current_editor_context();
        if self.editor_context.as_ref() == Some(&context) {
            if self.standard_layout_selected() {
                self.load_standard_preview();
            }
            return;
        }
        if let Some(previous) = self.editor_context.take()
            && self.editor_baseline.as_ref() != Some(&self.draft)
        {
            self.editor_cache.retain(|entry| entry.context != previous);
            self.editor_cache.push(Cached {
                context: previous,
                draft: self.draft.clone(),
                baseline: self.editor_baseline.clone(),
                editing_name: self.editing_name.clone(),
                selected_tile: self.selected_tile,
            });
        }
        if let Some(index) = self
            .editor_cache
            .iter()
            .position(|entry| entry.context == context)
        {
            let cached = self.editor_cache.remove(index);
            self.draft = cached.draft;
            self.editor_baseline = cached.baseline;
            self.editing_name = cached.editing_name;
            self.selected_tile = cached.selected_tile;
        } else {
            let preset = match &context.choice {
                Choice::Preset(name) => self
                    .config
                    .presets
                    .iter()
                    .find(|p| &p.name == name)
                    .cloned(),
                _ => None,
            };
            self.editing_name = preset.as_ref().map(|p| p.name.clone());
            self.draft = preset.unwrap_or_else(|| {
                Preset::from_layout(
                    self.unique_name("My workspace"),
                    match context.choice {
                        Choice::Builtin(layout) => layout,
                        _ => self.config.layout,
                    },
                    self.monitor_windows().len().clamp(1, 64),
                    self.config.master_ratio,
                )
            });
            self.editor_baseline = Some(self.draft.clone());
            self.selected_tile = 0;
        }
        self.editor_context = Some(context);
        if self.standard_layout_selected() {
            self.load_standard_preview();
        }
    }

    pub(super) fn standard_layout_selected(&self) -> bool {
        matches!(self.selected_layout(), Choice::Builtin(_))
    }

    fn load_standard_preview(&mut self) {
        let Choice::Builtin(layout) = self.selected_layout() else {
            return;
        };
        let windows = self.monitor_windows();
        let bindings = self
            .selected_monitor_area()
            .map(|area| self.config.standard_windows(area, layout))
            .unwrap_or_default();
        let local = self
            .inventory
            .as_ref()
            .zip(self.selected_monitor_area())
            .map(|(snapshot, area)| crate::display_scope::for_display(snapshot, area));
        let inventory: Vec<_> = local
            .as_ref()
            .map(|snapshot| snapshot.windows.iter().collect())
            .unwrap_or_default();
        let count = crate::standard_layout::slots_with_inventory(
            bindings,
            &windows.iter().collect::<Vec<_>>(),
            &inventory,
        )
        .len()
        .clamp(1, 64);
        self.draft = Preset::from_layout(
            self.unique_name("My workspace"),
            layout,
            count,
            self.config.master_ratio,
        );
        for (tile, binding) in self.draft.tiles.iter_mut().zip(bindings) {
            tile.window = binding.clone();
        }
        self.editing_name = None;
        self.editor_baseline = Some(self.draft.clone());
        self.selected_tile = self.selected_tile.min(count - 1);
    }

    pub(super) fn visible_editor_tiles(&self) -> Vec<usize> {
        let area = self.selected_monitor_area();
        self.draft
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| tile.display.is_none() || tile.display == area)
            .map(|(index, _)| index)
            .collect()
    }

    pub(super) fn workspace_canvas(&mut self, ui: &mut egui::Ui) {
        let visible = self.visible_editor_tiles();
        let mut drawing = self.draft.clone();
        drawing.tiles = visible
            .iter()
            .map(|index| self.draft.tiles[*index].clone())
            .collect();
        let mut selected = visible
            .iter()
            .position(|index| *index == self.selected_tile)
            .unwrap_or(0);
        let height = canvas::monitor_height(ui.available_width(), self.selected_monitor_area());
        let local = self
            .inventory
            .as_ref()
            .zip(self.selected_monitor_area())
            .map(|(snapshot, area)| crate::display_scope::for_display(snapshot, area));
        let windows: Vec<_> = local
            .as_ref()
            .map(|snapshot| snapshot.windows.iter().collect())
            .unwrap_or_default();
        let captions: Vec<_> = crate::window_binding::tile_resolutions(&drawing, &windows)
            .into_iter()
            .map(|resolved| resolved.index().map(|index| windows[index].title.clone()))
            .collect();
        canvas::show(
            ui,
            &mut drawing,
            &mut selected,
            !self.standard_layout_selected(),
            height,
            &captions,
        );
        for (index, tile) in visible.iter().zip(drawing.tiles) {
            self.draft.tiles[*index] = tile;
        }
        if let Some(index) = visible.get(selected) {
            self.selected_tile = *index;
        }
    }

    pub(super) fn apply_workspace(&mut self, engage: bool) {
        if !self.standard_layout_selected()
            && self.editor_baseline.as_ref() != Some(&self.draft)
            && !self.save_preset()
        {
            return;
        }
        self.apply(engage);
    }
}
