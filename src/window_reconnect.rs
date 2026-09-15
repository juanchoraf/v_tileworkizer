use crate::{
    config::Config, platform::Snapshot, presets::WindowBinding, window_binding, workspace,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Reconnector {
    aliases: BTreeMap<(u64, String), Vec<WindowBinding>>,
}

impl Reconnector {
    pub fn update(&mut self, config: &Config, snapshot: &mut Snapshot) {
        let configured: Vec<_> = config
            .presets
            .iter()
            .flat_map(|preset| preset.tiles.iter().filter_map(|tile| tile.window.as_ref()))
            .chain(
                config
                    .standard_assignments
                    .iter()
                    .flat_map(|entry| entry.windows.iter().flatten()),
            )
            .collect();
        self.aliases.retain(|(id, session), aliases| {
            aliases.retain(|binding| {
                configured
                    .iter()
                    .any(|current| current.same_selector(binding))
            });
            !aliases.is_empty()
                && snapshot
                    .windows
                    .iter()
                    .any(|w| w.id == *id && w.session == *session)
        });
        for window in &mut snapshot.windows {
            window.reconnects = self
                .aliases
                .get(&(window.id, window.session.clone()))
                .cloned()
                .unwrap_or_default();
        }
        let mut new_aliases = Vec::new();
        for (display, area) in snapshot.monitors.iter().enumerate() {
            let bindings: Vec<_> = if let Some(preset) = config.preset_for(*area) {
                preset
                    .tiles
                    .iter()
                    .filter(|tile| tile.display.is_none_or(|target| target == *area))
                    .filter_map(|tile| tile.window.as_ref())
                    .collect()
            } else {
                let layout = match config.layout_for(Some(*area)) {
                    crate::display_layout::Choice::Builtin(layout) => layout,
                    _ => config.layout,
                };
                config
                    .standard_windows(*area, layout)
                    .iter()
                    .flatten()
                    .collect()
            };
            let windows: Vec<_> = snapshot
                .windows
                .iter()
                .filter(|window| workspace::display_index(snapshot, window) == display)
                .collect();
            for (binding, resolved) in bindings
                .iter()
                .zip(window_binding::resolve(&bindings, &windows))
            {
                if let Some(index) = resolved.index() {
                    let window = windows[index];
                    if !snapshot
                        .windows
                        .iter()
                        .any(|current| binding.matches(current))
                    {
                        new_aliases.push((window.id, window.session.clone(), (*binding).clone()));
                    }
                }
            }
        }
        let unambiguous: Vec<_> = new_aliases
            .iter()
            .filter(|(_, _, binding)| {
                new_aliases
                    .iter()
                    .filter(|(_, _, other)| binding.same_selector(other))
                    .count()
                    == 1
            })
            .cloned()
            .collect();
        for (id, session, binding) in unambiguous {
            self.aliases
                .entry((id, session.clone()))
                .or_default()
                .push(binding.clone());
            if let Some(window) = snapshot
                .windows
                .iter_mut()
                .find(|w| w.id == id && w.session == session)
            {
                window.reconnects.push(binding);
            }
        }
    }
}
