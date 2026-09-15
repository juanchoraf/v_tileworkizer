use crate::platform::Window;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowBinding {
    pub id: u64,
    pub title: String,
    pub session: String,
    #[serde(default)]
    pub app_id: String,
    /// Optional stable fragment used only when reconnecting, never for a live ID.
    #[serde(default)]
    pub title_filter: String,
}

impl WindowBinding {
    pub fn from_window(window: &Window) -> Self {
        Self {
            id: window.id,
            session: window.session.clone(),
            title: window.title.clone(),
            app_id: window.app_id.clone(),
            title_filter: String::new(),
        }
    }

    pub fn matches(&self, window: &Window) -> bool {
        (!self.session.is_empty() && self.session == window.session && self.id == window.id)
            || window
                .reconnects
                .iter()
                .any(|binding| self.same_selector(binding))
    }

    pub fn same_selector(&self, other: &Self) -> bool {
        self.id == other.id
            && self.session == other.session
            && self.app_id == other.app_id
            && self.title_filter == other.title_filter
    }

    pub fn valid(&self) -> bool {
        self.session.len() <= 128
            && self.title.len() <= 16384
            && self.app_id.len() <= 32768
            && self.title_filter.len() <= 1024
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Found(usize),
    Missing,
    Ambiguous,
}

impl Resolution {
    pub fn index(self) -> Option<usize> {
        if let Self::Found(index) = self {
            Some(index)
        } else {
            None
        }
    }
}

/// Live identities take precedence. Reconnection requires one unclaimed candidate
/// and one claimant, regardless of enumeration order or current window titles.
pub fn resolve(bindings: &[&WindowBinding], windows: &[&Window]) -> Vec<Resolution> {
    let mut result = vec![Resolution::Missing; bindings.len()];
    let mut reserved = vec![false; windows.len()];
    for (i, binding) in bindings.iter().enumerate() {
        let mut exact = windows
            .iter()
            .enumerate()
            .filter(|(_, window)| binding.matches(window));
        if let Some((index, _)) = exact.next() {
            if exact.next().is_some() {
                result[i] = Resolution::Ambiguous;
                continue;
            }
            if !reserved[index] {
                result[i] = Resolution::Found(index);
                reserved[index] = true;
            } else {
                result[i] = Resolution::Ambiguous;
            }
        }
    }
    let mut candidates = vec![None; bindings.len()];
    let mut claims = vec![0; windows.len()];
    for (i, binding) in bindings.iter().enumerate() {
        if result[i] != Resolution::Missing || binding.app_id.is_empty() {
            continue;
        }
        let filter = binding.title_filter.trim().to_lowercase();
        let matching: Vec<_> = windows
            .iter()
            .enumerate()
            .filter(|(index, window)| {
                !reserved[*index]
                    && !window.state.protected
                    && window.app_id == binding.app_id
                    && (filter.is_empty() || window.title.to_lowercase().contains(&filter))
            })
            .map(|(index, _)| index)
            .collect();
        match matching.as_slice() {
            [index] => {
                candidates[i] = Some(*index);
                claims[*index] += 1;
            }
            [] => {}
            _ => result[i] = Resolution::Ambiguous,
        }
    }
    for (i, candidate) in candidates.into_iter().enumerate() {
        if let Some(index) = candidate {
            result[i] = if claims[index] == 1 {
                Resolution::Found(index)
            } else {
                Resolution::Ambiguous
            };
        }
    }
    result
}

pub fn tile_resolutions(preset: &crate::presets::Preset, windows: &[&Window]) -> Vec<Resolution> {
    let bindings: Vec<_> = preset
        .tiles
        .iter()
        .filter_map(|tile| tile.window.as_ref())
        .collect();
    let mut resolved = resolve(&bindings, windows).into_iter();
    preset
        .tiles
        .iter()
        .map(|tile| {
            if tile.window.is_some() {
                resolved.next().unwrap_or(Resolution::Missing)
            } else {
                Resolution::Missing
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "window_binding_tests.rs"]
mod tests;
