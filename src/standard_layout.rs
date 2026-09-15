//! Window selections are independent of the immutable standard layout geometry.
use crate::{
    config::Config,
    layout::{Layout, Rect},
    platform::Window,
    presets::WindowBinding,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignments {
    pub display: Rect,
    pub layout: Layout,
    pub windows: Vec<Option<WindowBinding>>,
}

impl Config {
    pub fn standard_windows(&self, display: Rect, layout: Layout) -> &[Option<WindowBinding>] {
        self.standard_assignments
            .iter()
            .find(|entry| entry.display == display && entry.layout == layout)
            .map(|entry| entry.windows.as_slice())
            .unwrap_or_default()
    }

    pub fn assign_standard_window(
        &mut self,
        display: Rect,
        layout: Layout,
        index: usize,
        window: Option<WindowBinding>,
    ) {
        if index >= 64 {
            return;
        }
        let position = self
            .standard_assignments
            .iter()
            .position(|entry| entry.display == display && entry.layout == layout);
        let position = position.unwrap_or_else(|| {
            self.standard_assignments.push(Assignments {
                display,
                layout,
                windows: Vec::new(),
            });
            self.standard_assignments.len() - 1
        });
        let windows = &mut self.standard_assignments[position].windows;
        windows.resize(windows.len().max(index + 1), None);
        if let Some(binding) = &window {
            for other in windows.iter_mut() {
                if other
                    .as_ref()
                    .is_some_and(|other| other.id == binding.id && other.session == binding.session)
                {
                    *other = None;
                }
            }
        }
        windows[index] = window;
        while windows.last().is_some_and(Option::is_none) {
            windows.pop();
        }
        if windows.is_empty() {
            self.standard_assignments.remove(position);
        }
    }

    pub fn validate_standard_assignments(&self) -> Result<()> {
        ensure!(
            self.standard_assignments.len() <= 256,
            "Too many standard layout assignments"
        );
        for (index, entry) in self.standard_assignments.iter().enumerate() {
            ensure!(
                entry.display.width > 0 && entry.display.height > 0,
                "Invalid assignment display"
            );
            ensure!(
                entry.windows.len() <= 64,
                "At most 64 assigned standard tiles are supported"
            );
            ensure!(
                !self.standard_assignments[..index].iter().any(|other| {
                    other.display == entry.display && other.layout == entry.layout
                }),
                "Duplicate standard layout assignments"
            );
            let mut identities = std::collections::BTreeSet::new();
            for binding in entry.windows.iter().flatten() {
                ensure!(
                    !binding.session.is_empty() && binding.valid(),
                    "Invalid standard window binding"
                );
                ensure!(
                    identities.insert((binding.id, &binding.session)),
                    "A window can only occupy one standard tile"
                );
            }
        }
        Ok(())
    }
}

/// Reserve exact selections first, then fill free tiles. Missing or ineligible
/// selections keep their slots; all other eligible windows still get a tile.
#[cfg(test)]
pub fn slots(bindings: &[Option<WindowBinding>], windows: &[&Window]) -> Vec<Option<usize>> {
    slots_with_inventory(bindings, windows, windows)
}

pub fn slots_with_inventory(
    bindings: &[Option<WindowBinding>],
    windows: &[&Window],
    inventory: &[&Window],
) -> Vec<Option<usize>> {
    let mut used = vec![false; windows.len()];
    let mut assigned = Vec::with_capacity(bindings.len().max(windows.len()));
    let explicit: Vec<_> = bindings.iter().flatten().collect();
    let resolutions = crate::window_binding::resolve(&explicit, inventory);
    let mut resolutions = resolutions.into_iter();
    for binding in bindings {
        let found = binding
            .as_ref()
            .and_then(|_| resolutions.next().and_then(|r| r.index()))
            .and_then(|index| {
                windows.iter().position(|window| {
                    window.id == inventory[index].id && window.session == inventory[index].session
                })
            });
        if let Some(index) = found {
            used[index] = true;
        }
        assigned.push(found);
    }
    let mut available = (0..windows.len()).filter(|index| !used[*index]);
    for (binding, slot) in bindings.iter().zip(&mut assigned) {
        if binding.is_none() {
            *slot = available.next();
        }
    }
    assigned.extend(available.map(Some));
    assigned
}

#[cfg(test)]
#[path = "standard_layout_tests.rs"]
mod tests;
