use crate::{
    config::Config,
    display_layout::Choice,
    layout::{Layout, Rect},
    platform::{Snapshot, Window},
    presets::{Preset, Tile, WindowBinding},
    workspace,
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
                title: v_concat::v_concat!("Window {id}"),
                rect: if id <= 2 { left } else { right },
                session: v_concat::v_concat!("session-{id}"),
                reconnects: Vec::new(),
                app_id: String::new(),
                state: Default::default(),
            })
            .collect(),
    }
}

#[test]
fn independent_display_layouts_survive_serialization_and_monitor_reordering() {
    let mut snapshot = desktop();
    let mut config = Config::default();
    config.set_display_layout(snapshot.monitors[0], Choice::Builtin(Layout::Columns));
    config.set_display_layout(snapshot.monitors[1], Choice::Builtin(Layout::Rows));
    let config: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
    config.validate().unwrap();
    let mut plan = workspace::plan(&config, &snapshot).unwrap();
    plan.sort_by_key(|p| p.id);
    assert_ne!(plan[0].rect.x, plan[1].rect.x);
    assert_eq!(plan[0].rect.y, plan[1].rect.y);
    assert_eq!(plan[2].rect.x, plan[3].rect.x);
    assert_ne!(plan[2].rect.y, plan[3].rect.y);
    snapshot.monitors.reverse();
    let mut reordered = workspace::plan(&config, &snapshot).unwrap();
    reordered.sort_by_key(|p| p.id);
    assert_eq!(plan, reordered);
}

#[test]
fn custom_layout_on_one_display_can_pull_a_window_without_double_assignment() {
    let snapshot = desktop();
    let mut config = Config::default();
    config.presets.push(Preset {
        name: "Overlap".into(),
        tiles: vec![
            Tile {
                window: Some(WindowBinding {
                    id: 1,
                    title: "Window 1".into(),
                    session: "session-1".into(),
                    app_id: String::new(),
                    ..Default::default()
                }),
                z_index: 20,
                ..Tile::default()
            },
            Tile::default(),
        ],
    });
    config.set_display_layout(snapshot.monitors[0], Choice::Builtin(Layout::Columns));
    config.set_display_layout(snapshot.monitors[1], Choice::Preset("Overlap".into()));
    let plan = workspace::plan(&config, &snapshot).unwrap();
    assert_eq!(plan.len(), 3);
    assert_eq!(plan.iter().filter(|p| p.id == 1).count(), 1);
    assert!(plan.iter().find(|p| p.id == 1).unwrap().rect.x >= 0);
    assert!(plan.iter().find(|p| p.id == 2).unwrap().rect.x < 0);
    assert_eq!(plan.last().unwrap().id, 1);
}

#[test]
fn preset_rename_and_delete_update_all_display_references() {
    let snapshot = desktop();
    let mut config = Config::default();
    config
        .presets
        .push(Preset::from_layout("Work".into(), Layout::Grid, 2, 0.6));
    config.active_preset = Some("Work".into());
    for area in &snapshot.monitors {
        config.set_display_layout(*area, Choice::Preset("Work".into()));
    }
    config.rename_layout_preset("Work", "Renamed");
    config.presets[0].name = "Renamed".into();
    config.validate().unwrap();
    assert_eq!(
        config.layout_for(Some(snapshot.monitors[1])),
        Choice::Preset("Renamed".into())
    );
    config.remove_layout_preset("Renamed");
    config.presets.clear();
    config.validate().unwrap();
    assert_eq!(
        config.layout_for(Some(snapshot.monitors[0])),
        Choice::Builtin(Layout::Master)
    );
    config.display_layouts[0].choice = Choice::Preset("Missing".into());
    assert!(config.validate().is_err());
}

#[test]
fn old_global_layout_is_retained_for_unconfigured_displays() {
    let config: Config = serde_json::from_str(r#"{"layout":"rows","future_option":42}"#).unwrap();
    for area in desktop().monitors {
        assert_eq!(config.layout_for(Some(area)), Choice::Builtin(Layout::Rows));
    }
    assert_eq!(serde_json::to_value(config).unwrap()["future_option"], 42);
}
