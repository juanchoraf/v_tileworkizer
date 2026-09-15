use super::*;
use crate::{
    display_layout::Choice,
    layout::Layout,
    platform::Window,
    presets::{Preset, Tile, WindowBinding},
};

fn desktop() -> Snapshot {
    let left = Rect {
        x: -1000,
        y: 0,
        width: 1000,
        height: 800,
    };
    let right = Rect { x: 0, ..left };
    Snapshot {
        monitors: vec![left, right],
        monitor_brands: vec!["Dell".into(), "ASUS".into()],
        workspace: 0,
        windows: (1..=4)
            .map(|id| Window {
                id,
                title: id.to_string(),
                session: "test".into(),
                reconnects: Vec::new(),
                app_id: String::new(),
                rect: if id <= 2 { left } else { right },
                state: Default::default(),
            })
            .collect(),
    }
}

#[test]
fn explicit_request_only_moves_selected_display_with_its_own_layout() {
    let snapshot = desktop();
    let mut config = Config {
        enabled: true,
        ..Config::default()
    };
    config.set_display_layout(snapshot.monitors[0], Choice::Builtin(Layout::Columns));
    config.set_display_layout(snapshot.monitors[1], Choice::Builtin(Layout::Rows));
    for (index, ids) in [(0, vec![1, 2]), (1, vec![3, 4])] {
        let (undo, placements, _) = plan(
            &config,
            &snapshot,
            Some(Request {
                display: Some(snapshot.monitors[index]),
            }),
        )
        .unwrap();
        assert_eq!(placements.iter().map(|p| p.id).collect::<Vec<_>>(), ids);
        assert_eq!(undo.windows.iter().map(|w| w.id).collect::<Vec<_>>(), ids);
        assert_eq!(undo.monitors, vec![snapshot.monitors[index]]);
        if index == 0 {
            assert_ne!(placements[0].rect.x, placements[1].rect.x);
            assert_eq!(placements[0].rect.y, placements[1].rect.y);
        } else {
            assert_eq!(placements[0].rect.x, placements[1].rect.x);
            assert_ne!(placements[0].rect.y, placements[1].rect.y);
        }
    }
}

#[test]
fn arranging_once_on_one_display_preserves_other_display_engagement() {
    let snapshot = desktop();
    let [left, right] = snapshot.monitors[..] else {
        unreachable!()
    };
    let mut config = Config::default();
    config.set_display_engaged(left, true, &snapshot.monitors);
    config.set_display_engaged(right, true, &snapshot.monitors);
    config.set_display_engaged(left, false, &snapshot.monitors);
    assert!(!config.display_engaged(left));
    assert!(config.display_engaged(right));
    let (_, once, _) = plan(
        &config,
        &snapshot,
        Some(Request {
            display: Some(left),
        }),
    )
    .unwrap();
    assert_eq!(once.iter().map(|p| p.id).collect::<Vec<_>>(), vec![1, 2]);
    let (_, automatic, _) = plan(&config, &snapshot, None).unwrap();
    assert_eq!(
        automatic.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![3, 4]
    );
    config.enabled = false;
    config.set_display_engaged(left, true, &snapshot.monitors);
    assert!(!config.display_engaged(right));
}

#[test]
fn scoped_presets_cannot_pull_windows_from_other_displays_or_push_tiles_to_them() {
    let snapshot = desktop();
    let left = snapshot.monitors[0];
    let right = snapshot.monitors[1];
    let mut config = Config::default();
    config.presets.push(Preset {
        name: "Scoped".into(),
        tiles: vec![
            Tile {
                window: Some(WindowBinding {
                    id: 3,
                    title: "3".into(),
                    session: "test".into(),
                    app_id: String::new(),
                    ..Default::default()
                }),
                ..Tile::default()
            },
            Tile {
                display: Some(right),
                ..Tile::default()
            },
            Tile::default(),
        ],
    });
    config.set_display_layout(left, Choice::Preset("Scoped".into()));
    let (_, placements, layered) = plan(
        &config,
        &snapshot,
        Some(Request {
            display: Some(left),
        }),
    )
    .unwrap();
    assert!(layered);
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].id, 1);
    assert!(placements[0].rect.x < 0);
}

#[test]
fn disconnected_display_never_falls_back_to_another_display() {
    let mut snapshot = desktop();
    let area = snapshot.monitors[1];
    let mut config = Config::default();
    config.set_display_engaged(area, true, &snapshot.monitors);
    snapshot.monitors.pop();
    snapshot.windows.retain(|w| w.id <= 2);
    assert!(
        plan(
            &config,
            &snapshot,
            Some(Request {
                display: Some(area)
            })
        )
        .is_err()
    );
    assert!(plan(&config, &snapshot, None).unwrap().1.is_empty());
    assert!(for_display(&snapshot, area).windows.is_empty());
}

#[test]
fn engagement_serialization_and_legacy_global_configuration_are_preserved() {
    let snapshot = desktop();
    let mut config: Config = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
    assert_eq!(plan(&config, &snapshot, None).unwrap().1.len(), 4);
    config.set_display_engaged(snapshot.monitors[0], false, &snapshot.monitors);
    let restored: Config = serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
    restored.validate().unwrap();
    assert!(!restored.display_engaged(snapshot.monitors[0]));
    assert!(restored.display_engaged(snapshot.monitors[1]));
    // The explicit whole-desktop CLI command remains available.
    assert_eq!(
        plan(&restored, &snapshot, Some(Request { display: None }))
            .unwrap()
            .1
            .len(),
        4
    );
}
