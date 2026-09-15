//! GUI commands and automatic reflows operate on explicitly selected displays.
use crate::{config::Config, layout::Rect, platform::Snapshot, workspace};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Request {
    pub display: Option<Rect>,
}

impl Config {
    pub fn display_engaged(&self, display: Rect) -> bool {
        self.enabled
            && self
                .engaged_displays
                .as_ref()
                .is_none_or(|areas| areas.contains(&display))
    }

    pub fn set_display_engaged(&mut self, display: Rect, engage: bool, connected: &[Rect]) {
        let mut areas = if self.enabled {
            self.engaged_displays
                .clone()
                .unwrap_or_else(|| connected.to_vec())
        } else {
            Vec::new()
        };
        areas.retain(|area| *area != display);
        if engage {
            areas.push(display);
        }
        self.enabled = !areas.is_empty();
        self.engaged_displays = Some(areas);
    }

    pub fn validate_display_scope(&self) -> Result<()> {
        if let Some(areas) = &self.engaged_displays {
            ensure!(
                areas.len() <= 64,
                "At most 64 enforced displays are supported"
            );
            for (index, area) in areas.iter().enumerate() {
                ensure!(
                    area.width > 0 && area.height > 0,
                    "Invalid enforced display area"
                );
                ensure!(!areas[..index].contains(area), "Duplicate enforced display");
            }
        }
        Ok(())
    }
}

pub fn for_display(snapshot: &Snapshot, area: Rect) -> Snapshot {
    let mut selected = snapshot.clone();
    let index = snapshot.monitors.iter().position(|m| *m == area);
    selected.monitors = index.map(|_| vec![area]).unwrap_or_default();
    selected.monitor_brands = index
        .and_then(|i| snapshot.monitor_brands.get(i).cloned())
        .into_iter()
        .collect();
    selected
        .windows
        .retain(|window| index == Some(workspace::display_index(snapshot, window)));
    selected
}

/// Return both the plan and its scoped undo snapshot. A request takes precedence
/// over the automatic engagement set for this iteration.
pub fn plan(
    config: &Config,
    snapshot: &Snapshot,
    request: Option<Request>,
) -> Result<(Snapshot, Vec<workspace::Placement>, bool)> {
    if request.is_some_and(|r| r.display.is_none())
        || (request.is_none() && config.enabled && config.engaged_displays.is_none())
    {
        // Preserve the legacy whole-desktop CLI/configuration behavior.
        let layered = snapshot
            .monitors
            .iter()
            .any(|area| config.preset_for(*area).is_some());
        return Ok((
            snapshot.clone(),
            workspace::plan(config, snapshot)?,
            layered,
        ));
    }
    let areas: Vec<_> = match request.and_then(|r| r.display) {
        Some(area) => {
            ensure!(
                snapshot.monitors.contains(&area),
                "Selected display is no longer connected; select it again"
            );
            vec![area]
        }
        None => snapshot
            .monitors
            .iter()
            .copied()
            .filter(|area| config.display_engaged(*area))
            .collect(),
    };
    let mut scoped = snapshot.clone();
    scoped.windows.clear();
    scoped.monitors.clear();
    scoped.monitor_brands.clear();
    let mut placements = Vec::new();
    let mut layered = false;
    for area in areas {
        let local = for_display(snapshot, area);
        placements.extend(workspace::plan(config, &local)?);
        layered |= config.preset_for(area).is_some();
        scoped.windows.extend(local.windows);
        scoped.monitors.extend(local.monitors);
        scoped.monitor_brands.extend(local.monitor_brands);
    }
    Ok((scoped, placements, layered))
}

#[cfg(test)]
#[path = "display_scope_tests.rs"]
mod tests;
