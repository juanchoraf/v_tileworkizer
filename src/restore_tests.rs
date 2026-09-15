use super::*;
use crate::{layout::Rect, platform::WindowState};

#[derive(Default)]
struct Desktop {
    restored: Vec<Window>,
    raised: Vec<u64>,
    fail: Option<u64>,
}
impl Backend for Desktop {
    fn snapshot(&mut self) -> Result<Snapshot> {
        unreachable!()
    }
    fn place(&mut self, _: u64, _: Rect) -> Result<()> {
        unreachable!()
    }
    fn stacking_order(&mut self) -> Result<Vec<u64>> {
        Ok(vec![2, 1])
    }
    fn raise(&mut self, id: u64) -> Result<()> {
        self.raised.push(id);
        Ok(())
    }
    fn restore_window(&mut self, window: &Window) -> Result<()> {
        anyhow::ensure!(self.fail != Some(window.id), "refused");
        self.restored.push(window.clone());
        Ok(())
    }
}
fn snapshot() -> Snapshot {
    Snapshot {
        monitor_brands: Vec::new(),
        windows: (1..=2)
            .map(|id| Window {
                id,
                title: "Same title".into(),
                session: v_concat::v_concat!("window-{id}"),
                reconnects: Vec::new(),
                app_id: String::new(),
                rect: Rect {
                    x: id as i32 * 100,
                    y: 50,
                    width: 200,
                    height: 150,
                },
                state: WindowState {
                    maximized: id == 2,
                    ..Default::default()
                },
            })
            .collect(),
        monitors: vec![Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }],
        workspace: 0,
    }
}
fn plan(s: &Snapshot) -> Vec<Placement> {
    s.windows
        .iter()
        .map(|w| Placement {
            id: w.id,
            rect: w.rect,
            layer: 0,
        })
        .collect()
}
#[test]
fn repeated_apply_keeps_original_geometry_state_and_stack() {
    let mut desktop = Desktop::default();
    let before = snapshot();
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    let mut tiled = before.clone();
    for w in &mut tiled.windows {
        w.rect.x += 500;
        w.state.maximized = false;
    }
    undo.capture(&mut desktop, &tiled, &plan(&tiled)).unwrap();
    undo.restore(&mut desktop, &tiled);
    assert!(undo.active());
    undo.restore(&mut desktop, &before);
    assert_eq!(desktop.restored[0].rect, before.windows[0].rect);
    assert!(desktop.restored[1].state.maximized);
    assert_eq!(desktop.raised, vec![2, 1]);
    assert!(!undo.active());
    // The next engagement takes a fresh baseline.
    undo.capture(&mut desktop, &tiled, &plan(&tiled)).unwrap();
    undo.restore(&mut desktop, &tiled);
    undo.restore(&mut desktop, &tiled);
    assert_eq!(desktop.restored[2].rect, tiled.windows[0].rect);
}
#[test]
fn closed_or_reused_ids_are_skipped_and_new_windows_get_their_own_baseline() {
    let mut desktop = Desktop::default();
    let mut before = snapshot();
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    before.windows[0].session = "replacement".into();
    before.windows[1].id = 3;
    before.windows[1].session = "new window".into();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    undo.restore(&mut desktop, &before);
    undo.restore(&mut desktop, &before);
    assert_eq!(desktop.restored.len(), 2);
    assert_eq!(desktop.restored[0].session, "replacement");
    assert_eq!(desktop.restored[1].id, 3);
    assert!(!undo.active());
}
#[test]
fn one_failed_or_hidden_window_does_not_block_others_and_can_retry() {
    let mut desktop = Desktop {
        fail: Some(1),
        ..Default::default()
    };
    let mut before = snapshot();
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    assert!(undo.restore(&mut desktop, &before).contains("incomplete"));
    assert_eq!(desktop.restored.len(), 1);
    assert!(undo.active());
    desktop.fail = None;
    before.windows[0].state.minimized = true;
    assert!(undo.restore(&mut desktop, &before).contains("waiting"));
    before.windows[0].state.minimized = false;
    undo.restore(&mut desktop, &before);
    undo.restore(&mut desktop, &before);
    assert_eq!(desktop.restored.len(), 2);
    assert!(!undo.active());
}
#[test]
fn missing_display_waits_and_an_unseen_reused_id_is_not_restored() {
    let mut desktop = Desktop::default();
    let mut before = snapshot();
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    before.monitors.clear();
    assert!(undo.restore(&mut desktop, &before).contains("waiting"));
    assert!(desktop.restored.is_empty());
    before = snapshot();
    before.windows[0].session = "replacement".into();
    undo.restore(&mut desktop, &before);
    undo.restore(&mut desktop, &before);
    assert_eq!(desktop.restored.len(), 1);
    assert_eq!(desktop.restored[0].id, 2);
}

#[test]
fn ignored_native_move_keeps_baseline_and_retries_until_confirmed() {
    let mut desktop = Desktop::default();
    let before = snapshot();
    let mut tiled = before.clone();
    tiled.windows[0].rect.x += 500;
    tiled.windows[1].state.maximized = false;
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    for _ in 0..2 {
        assert!(undo.restore(&mut desktop, &tiled).contains("waiting"));
        assert!(undo.active());
        assert!(desktop.raised.is_empty());
    }
    assert_eq!(desktop.restored.len(), 4);
    assert!(
        undo.restore(&mut desktop, &before)
            .contains("restored (2 windows)")
    );
    assert!(!undo.active());
    assert_eq!(desktop.raised, vec![2, 1]);
}

#[test]
fn restores_all_movable_baseline_windows_even_outside_the_selected_tiles() {
    let mut desktop = Desktop::default();
    let mut before = snapshot();
    let mut protected = before.windows[0].clone();
    protected.id = 3;
    protected.state.protected = true;
    before.windows.push(protected);
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)[..1])
        .unwrap();
    let mut current = before.clone();
    for window in &mut current.windows {
        window.rect.x += 500;
    }
    undo.restore(&mut desktop, &current);
    assert_eq!(
        desktop.restored.iter().map(|w| w.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    undo.restore(&mut desktop, &before);
    assert!(!undo.active());
}

#[test]
fn pausing_one_display_preserves_other_baselines_and_pending_confirmations() {
    let mut desktop = Desktop::default();
    let mut before = snapshot();
    let left = before.monitors[0];
    let right = Rect { x: 1920, ..left };
    before.monitors.push(right);
    before.windows[1].rect.x += right.x;
    let mut undo = OriginalLayout::default();
    undo.capture(&mut desktop, &before, &plan(&before)).unwrap();
    let mut tiled = before.clone();
    for window in &mut tiled.windows {
        window.rect.y += 100;
    }

    undo.restore_displays(&mut desktop, &tiled, Some(&[left]));
    assert_eq!(
        desktop.restored.iter().map(|w| w.id).collect::<Vec<_>>(),
        vec![1]
    );
    assert!(desktop.raised.is_empty());
    // Reflow on the other display must preserve the pending restore confirmation.
    let other = crate::display_scope::for_display(&tiled, right);
    undo.capture(&mut desktop, &other, &plan(&other)).unwrap();
    tiled.windows[0] = before.windows[0].clone();
    undo.restore_displays(&mut desktop, &tiled, Some(&[left]));
    assert_eq!(desktop.restored.len(), 1);
    assert_eq!(desktop.raised, vec![1]);
    assert!(undo.active());

    // Arranging this display again captures its new freely moved geometry.
    tiled.windows[0].rect.y += 40;
    let local = crate::display_scope::for_display(&tiled, left);
    undo.capture(&mut desktop, &local, &plan(&local)).unwrap();
    undo.restore_displays(&mut desktop, &tiled, Some(&[right]));
    assert_eq!(
        desktop.restored.last().unwrap().rect,
        before.windows[1].rect
    );
    tiled.windows[1] = before.windows[1].clone();
    undo.restore_displays(&mut desktop, &tiled, Some(&[right]));
    assert_eq!(desktop.raised, vec![1, 2]);
    assert!(undo.active());
    undo.restore(&mut desktop, &tiled);
    assert_eq!(desktop.restored.last().unwrap().rect, local.windows[0].rect);
    undo.restore(&mut desktop, &tiled);
    assert!(!undo.active());
}
