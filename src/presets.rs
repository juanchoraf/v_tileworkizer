use crate::{
    layout::{self, Layout, Rect},
    platform::Window,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub name: String,
    /// Percentages of the monitor work area. Overlap is intentional and allowed.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub title_match: String,
    #[serde(default)]
    pub display: Option<Rect>,
    #[serde(default)]
    pub window: Option<WindowBinding>,
}

pub use crate::window_binding::WindowBinding;

impl Default for Tile {
    fn default() -> Self {
        Self {
            name: "New tile".into(),
            x: 10.0,
            y: 10.0,
            width: 60.0,
            height: 70.0,
            z_index: 0,
            title_match: String::new(),
            display: None,
            window: None,
        }
    }
}

impl Tile {
    pub fn rect(&self, area: Rect, gap: i32) -> Rect {
        let x = (area.width as f32 * self.x / 100.0).round() as i32;
        let y = (area.height as f32 * self.y / 100.0).round() as i32;
        let right = (area.width as f32 * (self.x + self.width) / 100.0).round() as i32;
        let bottom = (area.height as f32 * (self.y + self.height) / 100.0).round() as i32;
        let width = (right - x).max(1);
        let height = (bottom - y).max(1);
        let inset = gap.max(0).min((width - 1) / 2).min((height - 1) / 2);
        Rect {
            x: area.x + x + inset,
            y: area.y + y + inset,
            width: width - 2 * inset,
            height: height - 2 * inset,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub tiles: Vec<Tile>,
}

impl Preset {
    pub fn from_layout(name: String, layout: Layout, count: usize, ratio: f32) -> Self {
        let area = Rect {
            x: 0,
            y: 0,
            width: 10000,
            height: 10000,
        };
        Self {
            name,
            tiles: layout::arrange(area, count, layout, 0, ratio)
                .iter()
                .enumerate()
                .map(|(i, r)| Tile {
                    name: v_concat::v_concat!("Tile {}", i + 1),
                    x: r.x as f32 / 100.0,
                    y: r.y as f32 / 100.0,
                    width: r.width as f32 / 100.0,
                    height: r.height as f32 / 100.0,
                    z_index: i as i32,
                    title_match: String::new(),
                    display: None,
                    window: None,
                })
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 80,
            "Preset name must contain 1–80 bytes"
        );
        ensure!(
            !self.tiles.is_empty() && self.tiles.len() <= 64,
            "A preset needs 1–64 tiles"
        );
        for tile in &self.tiles {
            ensure!(
                !tile.name.trim().is_empty() && tile.name.len() <= 80,
                "Give every tile a name (up to 80 bytes)"
            );
            ensure!(
                [tile.x, tile.y, tile.width, tile.height]
                    .iter()
                    .all(|v| v.is_finite()),
                "Tile geometry must be finite"
            );
            ensure!(
                tile.x >= 0.0
                    && tile.y >= 0.0
                    && tile.width >= 1.0
                    && tile.height >= 1.0
                    && tile.x + tile.width <= 100.001
                    && tile.y + tile.height <= 100.001,
                "Tiles must fit inside the display and be at least 1% wide/high"
            );
            ensure!(
                (-1000..=1000).contains(&tile.z_index),
                "Layer must be between -1000 and 1000"
            );
            ensure!(
                tile.window.as_ref().is_none_or(WindowBinding::valid),
                "Invalid window binding"
            );
            ensure!(
                tile.display.is_none_or(|r| r.width > 0 && r.height > 0),
                "Invalid target display"
            );
            ensure!(
                tile.title_match.len() <= 1024,
                "Window title rule is too long"
            );
        }
        Ok(())
    }

    /// Explicit title rules reserve their windows before unassigned tiles are filled.
    /// Each window appears at most once. Extra windows remain untouched.
    pub fn assignments(&self, windows: &[&Window]) -> Vec<Option<usize>> {
        let mut assigned = vec![None; self.tiles.len()];
        let mut used = vec![false; windows.len()];
        for explicit in [true, false] {
            for (i, tile) in self.tiles.iter().enumerate() {
                if tile.window.is_some() {
                    continue;
                }
                let rule = tile.title_match.trim().to_lowercase();
                if !rule.is_empty() != explicit {
                    continue;
                }
                if let Some(index) = windows.iter().enumerate().position(|(j, window)| {
                    !used[j] && (rule.is_empty() || window.title.to_lowercase().contains(&rule))
                }) {
                    assigned[i] = Some(index);
                    used[index] = true;
                }
            }
        }
        assigned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_rules_win_over_generic_tiles_without_duplicate_windows() {
        let mut preset = Preset::from_layout("Work".into(), Layout::Columns, 3, 0.6);
        preset.tiles[1].title_match = "EDITOR".into();
        preset.tiles[2].title_match = "editor".into();
        let r = Rect {
            x: 0,
            y: 0,
            width: 1000,
            height: 1000,
        };
        let editor = Window {
            state: Default::default(),
            session: String::new(),
            reconnects: Vec::new(),
            app_id: String::new(),
            id: 1,
            title: "My editor".into(),
            rect: r,
        };
        let browser = Window {
            state: Default::default(),
            session: String::new(),
            reconnects: Vec::new(),
            app_id: String::new(),
            id: 2,
            title: "Browser".into(),
            rect: r,
        };
        assert_eq!(
            preset.assignments(&[&editor, &browser]),
            vec![Some(1), Some(0), None]
        );
    }
    #[test]
    fn overlapping_tiles_validate_but_offscreen_tiles_do_not() {
        let mut p = Preset {
            name: "Overlap".into(),
            tiles: vec![Tile::default(), Tile::default()],
        };
        assert!(p.validate().is_ok());
        p.tiles[0].x = 90.0;
        assert!(p.validate().is_err());
        p.tiles[0].x = f32::NAN;
        assert!(p.validate().is_err());
    }
}
