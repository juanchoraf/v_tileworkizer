//! Per-display overrides retain the legacy global layout as the fallback.
use crate::{
    config::Config,
    layout::{Layout, Rect},
    presets::Preset,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Builtin(Layout),
    Preset(String),
}
impl Choice {
    pub fn label(&self) -> &str {
        match self {
            Self::Builtin(Layout::Master) => "Master + stack",
            Self::Builtin(Layout::Columns) => "Columns",
            Self::Builtin(Layout::Rows) => "Rows",
            Self::Builtin(Layout::Grid) => "Grid",
            Self::Preset(name) => name,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayLayout {
    pub display: Rect,
    pub choice: Choice,
}

impl Config {
    pub fn layout_for(&self, display: Option<Rect>) -> Choice {
        display
            .and_then(|area| self.display_layouts.iter().find(|v| v.display == area))
            .map(|v| v.choice.clone())
            .unwrap_or_else(|| {
                self.active_preset
                    .as_ref()
                    .map(|name| Choice::Preset(name.clone()))
                    .unwrap_or(Choice::Builtin(self.layout))
            })
    }
    pub fn preset_for(&self, display: Rect) -> Option<&Preset> {
        match self.layout_for(Some(display)) {
            Choice::Preset(name) => self.presets.iter().find(|p| p.name == name),
            Choice::Builtin(_) => None,
        }
    }
    pub fn set_display_layout(&mut self, display: Rect, choice: Choice) {
        if let Some(entry) = self
            .display_layouts
            .iter_mut()
            .find(|v| v.display == display)
        {
            entry.choice = choice;
        } else {
            self.display_layouts.push(DisplayLayout { display, choice });
        }
    }
    pub fn validate_display_layouts(&self) -> Result<()> {
        ensure!(
            self.display_layouts.len() <= 64,
            "At most 64 display layout selections are supported"
        );
        for (i, entry) in self.display_layouts.iter().enumerate() {
            ensure!(
                entry.display.width > 0 && entry.display.height > 0,
                "Invalid layout display area"
            );
            ensure!(
                !self.display_layouts[..i]
                    .iter()
                    .any(|v| v.display == entry.display),
                "Duplicate display layout selection"
            );
            if let Choice::Preset(name) = &entry.choice {
                ensure!(
                    self.presets.iter().any(|p| &p.name == name),
                    "Display layout references a missing preset"
                );
            }
        }
        Ok(())
    }
    pub fn rename_layout_preset(&mut self, old: &str, new: &str) {
        if self.active_preset.as_deref() == Some(old) {
            self.active_preset = Some(new.into());
        }
        for entry in &mut self.display_layouts {
            if entry.choice == Choice::Preset(old.into()) {
                entry.choice = Choice::Preset(new.into());
            }
        }
    }
    pub fn remove_layout_preset(&mut self, name: &str) {
        if self.active_preset.as_deref() == Some(name) {
            self.active_preset = None;
        }
        for entry in &mut self.display_layouts {
            if entry.choice == Choice::Preset(name.into()) {
                entry.choice = Choice::Builtin(self.layout);
            }
        }
    }
}

#[cfg(test)]
#[path = "display_layout_tests.rs"]
mod tests;
