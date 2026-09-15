use super::*;
use crate::{
    config::Config,
    layout::Rect,
    platform::Snapshot,
    presets::{Preset, Tile},
};

fn window(id: u64, app: &str, title: &str) -> Window {
    Window {
        id,
        app_id: app.into(),
        title: title.into(),
        session: v_concat::v_concat!("live-{id}"),
        state: Default::default(),
        reconnects: Vec::new(),
        rect: Rect {
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
        },
    }
}

#[test]
fn live_identity_survives_title_changes_and_wins_over_other_app_windows() {
    let mut original = window(1, "x11:Spotify", "Song A");
    let binding = WindowBinding::from_window(&original);
    original.title = "Song B".into();
    let other = window(2, "x11:Spotify", "Song A");
    assert_eq!(
        resolve(&[&binding], &[&other, &original]),
        vec![Resolution::Found(1)]
    );
}

#[test]
fn application_reconnects_after_restart_but_titles_alone_never_match() {
    let binding = WindowBinding::from_window(&window(1, "x11:Spotify", "Song A"));
    let reopened = window(2, "x11:Spotify", "Song B");
    let impostor = window(3, "x11:Other", "Song A");
    assert_eq!(
        resolve(&[&binding], &[&impostor, &reopened]),
        vec![Resolution::Found(1)]
    );
    assert_eq!(
        resolve(&[&binding], &[&impostor]),
        vec![Resolution::Missing]
    );
}

#[test]
fn ambiguous_app_windows_need_a_unique_fragment_and_never_depend_on_order() {
    let mut binding = WindowBinding::from_window(&window(1, "x11:Code", "Old title"));
    let a = window(2, "x11:Code", "file.rs - Project A");
    let b = window(3, "x11:Code", "file.rs - Project B");
    assert_eq!(resolve(&[&binding], &[&a, &b]), vec![Resolution::Ambiguous]);
    assert_eq!(resolve(&[&binding], &[&b, &a]), vec![Resolution::Ambiguous]);
    binding.title_filter = "project b".into();
    assert_eq!(resolve(&[&binding], &[&a, &b]), vec![Resolution::Found(1)]);
    assert_eq!(resolve(&[&binding], &[&b, &a]), vec![Resolution::Found(0)]);
}

#[test]
fn two_stale_tiles_cannot_both_claim_one_reopened_window() {
    let a = WindowBinding::from_window(&window(1, "x11:Code", "A"));
    let b = WindowBinding::from_window(&window(2, "x11:Code", "B"));
    let reopened = window(3, "x11:Code", "C");
    assert_eq!(
        resolve(&[&a, &b], &[&reopened]),
        vec![Resolution::Ambiguous; 2]
    );
    let exact = WindowBinding::from_window(&reopened);
    assert_eq!(
        resolve(&[&a, &exact], &[&reopened]),
        vec![Resolution::Missing, Resolution::Found(0)]
    );
}

#[test]
fn hidden_windows_count_toward_ambiguity_and_protected_windows_are_not_reconnected() {
    let binding = WindowBinding::from_window(&window(1, "x11:Code", "Old"));
    let a = window(2, "x11:Code", "Visible");
    let mut b = window(3, "x11:Code", "Hidden");
    b.state.minimized = true;
    assert_eq!(resolve(&[&binding], &[&a, &b]), vec![Resolution::Ambiguous]);
    b.state.protected = true;
    assert_eq!(resolve(&[&binding], &[&b]), vec![Resolution::Missing]);
}

#[test]
fn old_saved_bindings_load_without_enabling_unsafe_title_fallback() {
    let binding: WindowBinding =
        serde_json::from_str(r#"{"id":1,"title":"Old","session":"old"}"#).unwrap();
    assert!(binding.app_id.is_empty());
    assert!(binding.title_filter.is_empty());
    assert_eq!(
        resolve(&[&binding], &[&window(1, "x11:Code", "Old")]),
        vec![Resolution::Missing]
    );
    let current = WindowBinding::from_window(&window(1, "x11:Code", "Old"));
    let roundtrip: WindowBinding =
        serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
    assert_eq!(current, roundtrip);
}

#[test]
fn reconnected_identity_is_pinned_until_that_window_closes_or_the_rule_changes() {
    let mut binding = WindowBinding::from_window(&window(1, "x11:Code", "Old"));
    binding.title_filter = "Project A".into();
    let mut snapshot = Snapshot {
        monitors: vec![window(2, "", "").rect],
        monitor_brands: Vec::new(),
        workspace: 0,
        windows: vec![window(2, "x11:Code", "Project A")],
    };
    let mut config = Config {
        active_preset: Some("Work".into()),
        presets: vec![Preset {
            name: "Work".into(),
            tiles: vec![Tile {
                window: Some(binding.clone()),
                ..Tile::default()
            }],
        }],
        ..Config::default()
    };
    let mut cache = crate::window_reconnect::Reconnector::default();
    cache.update(&config, &mut snapshot);
    assert!(binding.matches(&snapshot.windows[0]));
    snapshot.windows[0].title = "Completely different title".into();
    snapshot.windows.push(window(3, "x11:Code", "Project A"));
    cache.update(&config, &mut snapshot);
    assert!(binding.matches(&snapshot.windows[0]));
    assert!(!binding.matches(&snapshot.windows[1]));
    let plan = crate::workspace::plan(&config, &snapshot).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].id, 2);
    config.presets[0].tiles[0]
        .window
        .as_mut()
        .unwrap()
        .title_filter = "Other project".into();
    cache.update(&config, &mut snapshot);
    assert!(snapshot.windows[0].reconnects.is_empty());
    config.presets[0].tiles[0]
        .window
        .as_mut()
        .unwrap()
        .title_filter = "Project A".into();
    snapshot.windows.remove(0);
    cache.update(&config, &mut snapshot);
    assert!(binding.matches(&snapshot.windows[0]));
}

#[test]
fn standard_layouts_reconnect_without_changing_slot_geometry() {
    let binding = WindowBinding::from_window(&window(1, "x11:Spotify", "A"));
    let reopened = window(2, "x11:Spotify", "B");
    let other = window(3, "x11:Code", "C");
    assert_eq!(
        crate::standard_layout::slots(&[Some(binding), None], &[&other, &reopened]),
        vec![Some(1), Some(0)]
    );
}

#[test]
fn standard_layout_does_not_replace_a_minimized_assignment_with_another_app_window() {
    let mut hidden = window(1, "x11:Code", "A");
    hidden.state.minimized = true;
    let binding = WindowBinding::from_window(&hidden);
    let visible = window(2, "x11:Code", "B");
    assert_eq!(
        crate::standard_layout::slots_with_inventory(
            &[Some(binding)],
            &[&visible],
            &[&hidden, &visible]
        ),
        vec![None, Some(0)]
    );
}

#[test]
fn shared_preset_does_not_pin_the_same_stale_assignment_to_two_displays() {
    let binding = WindowBinding::from_window(&window(1, "x11:Spotify", "Old"));
    let a = window(2, "x11:Spotify", "A");
    let mut b = window(3, "x11:Spotify", "B");
    b.rect.x = 1000;
    let mut snapshot = Snapshot {
        monitors: vec![a.rect, b.rect],
        monitor_brands: Vec::new(),
        workspace: 0,
        windows: vec![a, b],
    };
    let config = Config {
        active_preset: Some("Shared".into()),
        presets: vec![Preset {
            name: "Shared".into(),
            tiles: vec![Tile {
                window: Some(binding),
                ..Tile::default()
            }],
        }],
        ..Config::default()
    };
    let mut cache = crate::window_reconnect::Reconnector::default();
    cache.update(&config, &mut snapshot);
    assert!(
        snapshot
            .windows
            .iter()
            .all(|window| window.reconnects.is_empty())
    );
    assert!(
        crate::workspace::plan(&config, &snapshot)
            .unwrap()
            .is_empty()
    );
}
