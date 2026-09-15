use super::*;
use crate::{display_layout::Choice, layout, platform::Snapshot, workspace};

fn desktop() -> Snapshot {
    let left = Rect {
        x: -1920,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let right = Rect { x: 0, ..left };
    Snapshot {
        monitors: vec![left, right],
        monitor_brands: Vec::new(),
        workspace: 0,
        windows: (1..=6)
            .map(|id| Window {
                id,
                title: "Same title".into(),
                rect: if id <= 3 { left } else { right },
                session: "desktop-session".into(),
                reconnects: Vec::new(),
                app_id: String::new(),
                state: Default::default(),
            })
            .collect(),
    }
}

fn binding(window: &Window) -> Option<WindowBinding> {
    Some(WindowBinding {
        id: window.id,
        title: window.title.clone(),
        session: window.session.clone(),
        app_id: String::new(),
        ..Default::default()
    })
}

#[test]
fn assigning_standard_tiles_preserves_geometry_and_other_displays_after_reload() {
    let snapshot = desktop();
    let area = snapshot.monitors[0];
    for selected in [Layout::Master, Layout::Columns, Layout::Rows, Layout::Grid] {
        let mut config = Config::default();
        config.set_display_layout(area, Choice::Builtin(selected));
        let original = workspace::plan(&config, &snapshot).unwrap();
        config.assign_standard_window(area, selected, 0, binding(&snapshot.windows[2]));
        config.assign_standard_window(area, selected, 2, binding(&snapshot.windows[0]));
        let config: Config = serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        config.validate().unwrap();
        assert!(config.presets.is_empty());
        assert_eq!(config.layout_for(Some(area)), Choice::Builtin(selected));
        let arranged = workspace::plan(&config, &snapshot).unwrap();
        let geometry = layout::arrange(area, 3, selected, config.gap, config.master_ratio);
        assert_eq!(
            arranged.iter().find(|p| p.id == 3).unwrap().rect,
            geometry[0]
        );
        assert_eq!(
            arranged.iter().find(|p| p.id == 2).unwrap().rect,
            geometry[1]
        );
        assert_eq!(
            arranged.iter().find(|p| p.id == 1).unwrap().rect,
            geometry[2]
        );
        assert_eq!(
            original.iter().filter(|p| p.id > 3).collect::<Vec<_>>(),
            arranged.iter().filter(|p| p.id > 3).collect::<Vec<_>>()
        );
    }
}

#[test]
fn reassignment_moves_a_binding_and_clearing_it_restores_automatic_slots() {
    let snapshot = desktop();
    let area = snapshot.monitors[0];
    let mut config = Config::default();
    config.assign_standard_window(area, Layout::Master, 0, binding(&snapshot.windows[1]));
    config.assign_standard_window(area, Layout::Master, 2, binding(&snapshot.windows[1]));
    config.validate().unwrap();
    assert_eq!(
        config.standard_windows(area, Layout::Master),
        &[None, None, binding(&snapshot.windows[1])]
    );
    assert!(config.standard_windows(area, Layout::Rows).is_empty());
    config.assign_standard_window(area, Layout::Master, 2, None);
    assert!(config.standard_assignments.is_empty());
    assert_eq!(
        workspace::plan(&config, &snapshot).unwrap(),
        workspace::plan(&Config::default(), &snapshot).unwrap()
    );
}

#[test]
fn unavailable_windows_keep_empty_slots_without_dropping_other_windows() {
    let mut snapshot = desktop();
    let area = snapshot.monitors[0];
    let mut config = Config::default();
    config.assign_standard_window(area, Layout::Master, 0, binding(&snapshot.windows[0]));
    snapshot.windows[0].state.minimized = true;
    let plan = workspace::plan(&config, &snapshot).unwrap();
    assert_eq!(plan.len(), 5);
    let geometry = layout::arrange(area, 3, Layout::Master, config.gap, config.master_ratio);
    assert!(!plan.iter().any(|p| p.id == 1 || p.rect == geometry[0]));
    // A reused native ID in a new session does not steal the original binding.
    snapshot.windows[0].state.minimized = false;
    snapshot.windows[0].session = "new-session".into();
    let plan = workspace::plan(&config, &snapshot).unwrap();
    assert_eq!(plan.len(), 6);
    let geometry = layout::arrange(area, 4, Layout::Master, config.gap, config.master_ratio);
    assert!(!plan.iter().any(|p| p.rect == geometry[0]));
}

#[test]
fn scoped_assignment_never_pulls_a_window_from_another_display() {
    let snapshot = desktop();
    let area = snapshot.monitors[0];
    let mut config = Config::default();
    config.assign_standard_window(area, Layout::Master, 0, binding(&snapshot.windows[4]));
    let (_, plan, _) = crate::display_scope::plan(
        &config,
        &snapshot,
        Some(crate::display_scope::Request {
            display: Some(area),
        }),
    )
    .unwrap();
    assert_eq!(plan.len(), 3);
    assert!(
        plan.iter()
            .all(|placement| placement.id <= 3 && placement.rect.x < 0)
    );
}

#[test]
fn malformed_assignments_are_rejected_and_legacy_config_defaults_to_empty() {
    let old: Config = serde_json::from_str(r#"{"layout":"grid"}"#).unwrap();
    old.validate().unwrap();
    assert!(old.standard_assignments.is_empty());
    let snapshot = desktop();
    let mut config = Config::default();
    config.assign_standard_window(
        snapshot.monitors[0],
        Layout::Grid,
        0,
        binding(&snapshot.windows[0]),
    );
    config.standard_assignments[0]
        .windows
        .push(binding(&snapshot.windows[0]));
    assert!(config.validate().is_err());
    config.standard_assignments[0].windows.pop();
    config
        .standard_assignments
        .push(config.standard_assignments[0].clone());
    assert!(config.validate().is_err());
}
