//! Undo the current engagement without replacing its baseline on each reflow.
use crate::{
    platform::{Backend, Snapshot, Window},
    workspace::Placement,
};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct OriginalLayout {
    windows: Vec<Window>,
    displays: BTreeMap<u64, crate::layout::Rect>,
    order: Vec<u64>,
    changed: BTreeSet<u64>,
    requested: BTreeSet<u64>,
    restored: usize,
    active: bool,
}

impl OriginalLayout {
    pub fn active(&self) -> bool {
        self.active
    }

    pub fn has_displays(&self, areas: &[crate::layout::Rect]) -> bool {
        self.windows.iter().any(|window| {
            self.displays
                .get(&window.id)
                .is_some_and(|area| areas.contains(area))
        })
    }

    pub fn capture(
        &mut self,
        backend: &mut dyn Backend,
        snapshot: &Snapshot,
        plan: &[Placement],
    ) -> Result<()> {
        if !self.active {
            self.order = backend.stacking_order()?;
            self.windows = snapshot.windows.clone();
            self.changed.extend(
                snapshot
                    .windows
                    .iter()
                    .filter(|w| w.eligible())
                    .map(|w| w.id),
            );
            self.active = true;
        }
        for window in &snapshot.windows {
            let destination = plan.iter().find(|p| p.id == window.id).map(|p| p.rect);
            let area = destination
                .and_then(|rect| {
                    snapshot.monitors.iter().find(|area| {
                        area.contains(
                            rect.x.saturating_add(rect.width / 2),
                            rect.y.saturating_add(rect.height / 2),
                        )
                    })
                })
                .or_else(|| {
                    snapshot
                        .monitors
                        .get(crate::workspace::display_index(snapshot, window))
                });
            if let Some(area) = area {
                self.displays.insert(window.id, *area);
            }

            if window.eligible() || plan.iter().any(|p| p.id == window.id) {
                // A newly opened window gets its own pre-tiling baseline. Never reuse
                // an old native ID after its previous window has disappeared.
                if !self.windows.iter().any(|w| same(w, window)) {
                    self.windows.retain(|w| w.id != window.id);
                    self.requested.remove(&window.id);
                    self.windows.push(window.clone());
                    if !self.order.contains(&window.id) {
                        self.order.push(window.id);
                    }
                }
                self.changed.insert(window.id);
                // Applying another display must not clear pending restore confirmations.
                self.requested.remove(&window.id);
            }
        }
        Ok(())
    }

    /// Failures stay pending so one uncooperative app cannot prevent other restores.
    pub fn restore(&mut self, backend: &mut dyn Backend, snapshot: &Snapshot) -> String {
        self.restore_displays(backend, snapshot, None)
    }

    pub fn restore_displays(
        &mut self,
        backend: &mut dyn Backend,
        snapshot: &Snapshot,
        areas: Option<&[crate::layout::Rect]>,
    ) -> String {
        let selected: BTreeSet<_> = self
            .windows
            .iter()
            .filter(|window| {
                areas.is_none_or(|areas| {
                    self.displays
                        .get(&window.id)
                        .is_some_and(|area| areas.contains(area))
                })
            })
            .map(|window| window.id)
            .collect();
        let mut errors = Vec::new();
        for original in &self.windows {
            if !selected.contains(&original.id) || !self.changed.contains(&original.id) {
                continue;
            }
            let Some(current) = snapshot.windows.iter().find(|w| same(original, w)) else {
                self.changed.remove(&original.id);
                continue;
            };
            let on_connected_display = snapshot.monitors.iter().any(|m| {
                original.rect.x < m.x.saturating_add(m.width)
                    && original.rect.x.saturating_add(original.rect.width) > m.x
                    && original.rect.y < m.y.saturating_add(m.height)
                    && original.rect.y.saturating_add(original.rect.height) > m.y
            });
            if !current.eligible() || !on_connected_display {
                continue;
            }
            if self.requested.contains(&original.id) && restored_geometry(original, current) {
                self.changed.remove(&original.id);
                self.requested.remove(&original.id);
                self.restored += 1;
                continue;
            }
            match backend.restore_window(original) {
                Ok(()) => {
                    // Native window movement is asynchronous. Keep the original until
                    // a later snapshot confirms it, and retry ignored/refused requests.
                    self.requested.insert(original.id);
                }
                Err(e) => errors.push(v_concat::v_concat!("Window {}: {e:#}", original.id)),
            }
        }
        let restored = self.restored;
        let pending = self.changed.intersection(&selected).count();
        if pending == 0 {
            // Raise bottom to top, including untouched eligible baseline windows,
            // so their relative position in the original stack is preserved too.
            for id in &self.order {
                if selected.contains(id)
                    && let Some(original) = self.windows.iter().find(|w| w.id == *id)
                    && original.eligible()
                    && snapshot
                        .windows
                        .iter()
                        .any(|w| same(original, w) && w.eligible())
                    && let Err(e) = backend.raise(*id)
                {
                    errors.push(v_concat::v_concat!("Stacking window {id}: {e:#}"));
                }
            }
            if errors.is_empty() {
                self.windows.retain(|w| !selected.contains(&w.id));
                self.order.retain(|id| !selected.contains(id));
                self.displays.retain(|id, _| !selected.contains(id));
                self.requested.retain(|id| !selected.contains(id));
                self.restored = 0;
                self.active = !self.windows.is_empty();
                if !self.active {
                    *self = Self::default();
                }
            }
        }
        if !errors.is_empty() {
            v_concat::v_concat!(
                "Not Enforced · restoration incomplete: {}",
                errors.join("; ")
            )
        } else if pending > 0 {
            v_concat::v_concat!(
                "Not Enforced · {} windows waiting for restoration confirmation, visibility or a connected display",
                pending
            )
        } else {
            v_concat::v_concat!("Not Enforced · previous arrangement restored ({restored} windows)")
        }
    }
}

fn same(a: &Window, b: &Window) -> bool {
    a.id == b.id && a.session == b.session
}

fn restored_geometry(original: &Window, current: &Window) -> bool {
    // Sway owns the geometry of tiled containers; restoring floating mode is the
    // available native undo operation for those windows.
    if original.state.floating == Some(false) {
        return current.state.floating == Some(false);
    }
    original.rect == current.rect
        && original.state.maximized == current.state.maximized
        && original.state.floating == current.state.floating
}

#[cfg(test)]
#[path = "restore_tests.rs"]
mod tests;
