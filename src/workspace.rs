//! Shared desktop inventory and layout planning, independent of GUI presentation.
use crate::{
    config::{self, Config},
    layout,
    platform::{Backend, Snapshot},
    service,
};
use anyhow::{Result, ensure};
use std::fs;

#[derive(Debug, PartialEq)]
pub struct Placement {
    pub id: u64,
    pub rect: layout::Rect,
    pub layer: i32,
}

pub fn plan(config: &Config, snapshot: &Snapshot) -> Result<Vec<Placement>> {
    ensure!(!snapshot.monitors.is_empty(), "No usable monitors detected");
    let mut plan = Vec::new();
    let mut tile_order = std::collections::BTreeMap::new();
    let mut reserved = std::collections::BTreeSet::new();
    let windows: Vec<_> = snapshot.windows.iter().collect();
    let mut bindings: Vec<&crate::presets::WindowBinding> = Vec::new();
    for area in &snapshot.monitors {
        if let Some(preset) = config.preset_for(*area) {
            for binding in preset.tiles.iter().filter_map(|tile| tile.window.as_ref()) {
                if !bindings.iter().any(|other| std::ptr::eq(*other, binding)) {
                    bindings.push(binding);
                }
            }
        }
    }
    let resolved = crate::window_binding::resolve(&bindings, &windows);
    // Reserve exact selections across all displays before filling automatic tiles.
    for area in &snapshot.monitors {
        let Some(preset) = config.preset_for(*area) else {
            continue;
        };
        for (tile_index, tile) in preset.tiles.iter().enumerate() {
            let Some(binding) = &tile.window else {
                continue;
            };
            let Some(window) = bindings
                .iter()
                .position(|other| std::ptr::eq(*other, binding))
                .and_then(|index| resolved[index].index())
                .map(|index| windows[index])
            else {
                continue;
            };
            let explicit = config
                .display_layouts
                .iter()
                .any(|entry| entry.display == *area);
            let destination = tile.display.unwrap_or_else(|| {
                if explicit {
                    *area
                } else {
                    snapshot.monitors[display_index(snapshot, window)]
                }
            });
            if destination != *area {
                if !snapshot.monitors.contains(&destination) {
                    reserved.insert(window.id);
                }
                continue;
            }
            if !reserved.insert(window.id) || !window.eligible() {
                continue;
            }
            tile_order.insert(window.id, tile_index);
            plan.push(Placement {
                id: window.id,
                rect: tile.rect(*area, 0),
                layer: tile.z_index,
            });
        }
    }
    for (index, area) in snapshot.monitors.iter().enumerate() {
        let windows: Vec<_> = snapshot
            .windows
            .iter()
            .filter(|w| {
                w.eligible() && !reserved.contains(&w.id) && display_index(snapshot, w) == index
            })
            .collect();
        if let Some(preset) = config.preset_for(*area) {
            let local = crate::presets::Preset {
                name: preset.name.clone(),
                tiles: preset
                    .tiles
                    .iter()
                    .filter(|t| t.window.is_none() && t.display.is_none_or(|d| d == *area))
                    .cloned()
                    .collect(),
            };
            let original_indices: Vec<_> = preset
                .tiles
                .iter()
                .enumerate()
                .filter(|(_, t)| t.window.is_none() && t.display.is_none_or(|d| d == *area))
                .map(|(i, _)| i)
                .collect();
            for ((tile, assigned), tile_index) in local
                .tiles
                .iter()
                .zip(local.assignments(&windows))
                .zip(original_indices)
            {
                if let Some(i) = assigned {
                    tile_order.insert(windows[i].id, tile_index);
                    plan.push(Placement {
                        id: windows[i].id,
                        rect: tile.rect(*area, 0),
                        layer: tile.z_index,
                    });
                }
            }
        } else {
            let selected = match config.layout_for(Some(*area)) {
                crate::display_layout::Choice::Builtin(layout) => layout,
                _ => config.layout,
            };
            let inventory: Vec<_> = snapshot
                .windows
                .iter()
                .filter(|window| display_index(snapshot, window) == index)
                .collect();
            let slots = crate::standard_layout::slots_with_inventory(
                config.standard_windows(*area, selected),
                &windows,
                &inventory,
            );
            let tiles = layout::arrange(
                *area,
                slots.len(),
                selected,
                config.gap,
                config.master_ratio,
            );
            ensure!(
                tiles.len() == slots.len(),
                "Too many windows for this monitor; reduce gaps or window count"
            );
            for (slot, rect) in slots.into_iter().zip(tiles) {
                if let Some(index) = slot {
                    plan.push(Placement {
                        id: windows[index].id,
                        rect,
                        layer: 0,
                    });
                }
            }
        }
    }
    // Stable sorting preserves tile order when layers are equal.
    plan.sort_by_key(|p| (p.layer, tile_order.get(&p.id).copied().unwrap_or(0)));
    Ok(plan)
}

pub fn display_index(snapshot: &Snapshot, window: &crate::platform::Window) -> usize {
    snapshot
        .monitors
        .iter()
        .position(|m| {
            m.contains(
                window.rect.x.saturating_add(window.rect.width / 2),
                window.rect.y.saturating_add(window.rect.height / 2),
            )
        })
        .unwrap_or(0)
}

pub fn display_label(snapshot: &Snapshot, index: usize, area: layout::Rect) -> String {
    v_concat::v_concat!(
        "{} · {}×{} · ({}, {})",
        snapshot.display_name(index),
        area.width,
        area.height,
        area.x,
        area.y
    )
}

pub fn apply(
    backend: &mut dyn Backend,
    original: &mut crate::restore::OriginalLayout,
    snapshot: &Snapshot,
    plan: &[Placement],
    layered: bool,
) -> Result<bool> {
    apply_with_cancel(backend, original, snapshot, plan, layered, || {
        Ok(disengaged()?
            || crate::display_pause::paused()?
                .iter()
                .any(|area| snapshot.monitors.contains(area)))
    })
}

fn apply_with_cancel(
    backend: &mut dyn Backend,
    original: &mut crate::restore::OriginalLayout,
    snapshot: &Snapshot,
    plan: &[Placement],
    layered: bool,
    mut cancelled: impl FnMut() -> Result<bool>,
) -> Result<bool> {
    // Every arrangement needs undo, regardless of whether automatic tiling is
    // enabled. Save first so cancellation or a partial failure can be undone too.
    original.capture(backend, snapshot, plan)?;
    for placement in plan {
        if cancelled()? {
            return Ok(false);
        }
        backend.place(placement.id, placement.rect)?;
    }
    if layered {
        for placement in plan {
            if cancelled()? {
                return Ok(false);
            }
            backend.raise(placement.id)?;
        }
    }
    Ok(true)
}

pub fn disengaged() -> Result<bool> {
    Ok(service::state_dir()?.join("disengage.request").exists())
}

pub fn request_inventory() -> Result<()> {
    config::atomic_write(&service::state_dir()?.join("inventory.request"), b"refresh")
}

pub fn publish(snapshot: &Snapshot) -> Result<()> {
    let path = service::state_dir()?.join("windows.json");
    let bytes = serde_json::to_vec(snapshot)?;
    if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        config::atomic_write(&path, &bytes)?;
    }
    Ok(())
}

pub fn inventory() -> Option<Snapshot> {
    serde_json::from_slice(&fs::read(service::state_dir().ok()?.join("windows.json")).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        platform::Window,
        presets::{Preset, Tile},
    };
    #[derive(Default)]
    struct Desktop {
        calls: Vec<(&'static str, u64)>,
    }
    impl Backend for Desktop {
        fn snapshot(&mut self) -> Result<Snapshot> {
            Ok(Snapshot {
                monitor_brands: Vec::new(),
                windows: vec![],
                monitors: vec![],
                workspace: 0,
            })
        }
        fn place(&mut self, id: u64, _: layout::Rect) -> Result<()> {
            self.calls.push(("move", id));
            Ok(())
        }
        fn raise(&mut self, id: u64) -> Result<()> {
            self.calls.push(("raise", id));
            Ok(())
        }
    }

    #[test]
    fn cancellation_stops_before_any_further_move_or_raise() {
        let area = layout::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let plan = vec![
            Placement {
                id: 1,
                rect: area,
                layer: 0,
            },
            Placement {
                id: 2,
                rect: area,
                layer: 10,
            },
        ];
        let mut desktop = Desktop::default();
        let mut original = crate::restore::OriginalLayout::default();
        let snapshot = Snapshot {
            windows: plan
                .iter()
                .map(|p| Window {
                    id: p.id,
                    rect: area,
                    title: String::new(),
                    session: "test".into(),
                    reconnects: Vec::new(),
                    app_id: String::new(),
                    state: Default::default(),
                })
                .collect(),
            monitors: vec![area],
            monitor_brands: Vec::new(),
            workspace: 0,
        };
        let mut checks = 0;
        assert!(
            !apply_with_cancel(&mut desktop, &mut original, &snapshot, &plan, true, || {
                checks += 1;
                Ok(checks > 1)
            })
            .unwrap()
        );
        assert_eq!(desktop.calls, vec![("move", 1)]);
        assert!(original.active());
        desktop.calls.clear();
        assert!(
            apply_with_cancel(&mut desktop, &mut original, &snapshot, &plan, true, || Ok(
                false
            ))
            .unwrap()
        );
        assert_eq!(
            desktop.calls,
            vec![("move", 1), ("move", 2), ("raise", 1), ("raise", 2)]
        );
        desktop.calls.clear();
        original = crate::restore::OriginalLayout::default();
        assert!(
            apply_with_cancel(&mut desktop, &mut original, &snapshot, &plan, false, || Ok(
                false
            ))
            .unwrap()
        );
        assert!(original.active());
        assert_eq!(desktop.calls, vec![("move", 1), ("move", 2)]);
    }
    #[test]
    fn custom_plan_orders_layers_and_does_not_move_overflow_windows() {
        let area = layout::Rect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let snapshot = Snapshot {
            monitor_brands: Vec::new(),
            monitors: vec![area],
            workspace: 0,
            windows: (1..=3)
                .map(|id| Window {
                    state: Default::default(),
                    session: String::new(),
                    reconnects: Vec::new(),
                    app_id: String::new(),
                    id,
                    title: id.to_string(),
                    rect: area,
                })
                .collect(),
        };
        let config = Config {
            active_preset: Some("Stack".into()),
            presets: vec![Preset {
                name: "Stack".into(),
                tiles: vec![
                    Tile {
                        z_index: 20,
                        ..Tile::default()
                    },
                    Tile::default(),
                ],
            }],
            ..Config::default()
        };
        let result = plan(&config, &snapshot).unwrap();
        assert_eq!(result.iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(result[0].rect, result[1].rect);
    }
    #[test]
    fn exact_assignment_moves_across_displays_and_reserves_duplicate_titles() {
        let left = layout::Rect {
            x: -1000,
            y: 0,
            width: 1000,
            height: 800,
        };
        let right = layout::Rect { x: 0, ..left };
        let mut snapshot = Snapshot {
            monitor_brands: Vec::new(),
            workspace: 0,
            monitors: vec![left, right],
            windows: (1..=2)
                .map(|id| Window {
                    id,
                    title: "Same title".into(),
                    rect: left,
                    session: v_concat::v_concat!("session-{id}"),
                    reconnects: Vec::new(),
                    app_id: String::new(),
                    state: Default::default(),
                })
                .collect(),
        };
        let selected = Tile {
            display: Some(right),
            window: Some(crate::presets::WindowBinding {
                id: 2,
                title: "Same title".into(),
                session: "session-2".into(),
                app_id: String::new(),
                ..Default::default()
            }),
            ..Tile::default()
        };
        let mut config = Config {
            active_preset: Some("Assigned".into()),
            presets: vec![Preset {
                name: "Assigned".into(),
                tiles: vec![Tile::default(), selected.clone()],
            }],
            ..Config::default()
        };
        let placements = plan(&config, &snapshot).unwrap();
        assert_eq!(
            placements.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(placements[0].rect, Tile::default().rect(left, 0));
        assert_eq!(placements[1].rect, selected.rect(right, 0));
        // Custom geometry is exact for both automatic and explicit assignments,
        // regardless of the gap configured for built-in layouts.
        for gap in [0, 13, 100] {
            config.gap = gap;
            assert_eq!(plan(&config, &snapshot).unwrap(), placements);
        }
        // Title edits preserve the exact identity and placement.
        snapshot.windows[1].title = "New document".into();
        assert_eq!(plan(&config, &snapshot).unwrap(), placements);
        // Disconnection reserves the selected window instead of filling an automatic tile.
        snapshot.monitors.pop();
        assert_eq!(plan(&config, &snapshot).unwrap().len(), 1);
        snapshot.monitors.push(right);
        // Each protected state defers an assignment without hiding it from inventory.
        for state in [
            crate::platform::WindowState {
                minimized: true,
                ..Default::default()
            },
            crate::platform::WindowState {
                fullscreen: true,
                ..Default::default()
            },
            crate::platform::WindowState {
                hidden: true,
                ..Default::default()
            },
            crate::platform::WindowState {
                protected: true,
                ..Default::default()
            },
        ] {
            snapshot.windows[1].state = state;
            assert_eq!(plan(&config, &snapshot).unwrap().len(), 1);
            assert_eq!(snapshot.windows.len(), 2);
        }
        snapshot.windows[1].state = Default::default();
        config.presets[0].tiles.remove(0);
        snapshot.windows[1].session = "restarted".into();
        assert!(plan(&config, &snapshot).unwrap().is_empty());
    }
}
