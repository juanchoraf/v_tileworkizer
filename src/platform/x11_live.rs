//! Opt-in integration test; creates and manipulates only its own temporary windows.
use super::*;
use std::time::{Duration, Instant};
use x11rb::wrapper::ConnectionExt as _;

fn wait_for(backend: &mut X11, id: u32, matches: impl Fn(&Window) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = backend.snapshot().unwrap();
        if snapshot
            .windows
            .iter()
            .any(|w| w.id == u64::from(id) && matches(w))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Window state was not discovered before timeout"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "requires a live EWMH X11 desktop; creates temporary test windows"]
fn discovers_duplicate_titles_minimized_maximized_and_fullscreen_windows() {
    let mut backend = X11::connect().unwrap();
    // Dropping this separate connection destroys its windows even after a failed assertion.
    let (conn, screen) = x11rb::connect(None).unwrap();
    let root = conn.setup().roots[screen].root;
    let mut ids = Vec::new();
    for i in 0..2 {
        let id = conn.generate_id().unwrap();
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            id,
            root,
            100 + i * 250,
            100,
            240,
            160,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixel(0x152b39),
        )
        .unwrap()
        .check()
        .unwrap();
        conn.change_property8(
            PropMode::REPLACE,
            id,
            backend.atoms._NET_WM_NAME,
            backend.atoms.UTF8_STRING,
            b"v_tileworkizer discovery test",
        )
        .unwrap()
        .check()
        .unwrap();
        conn.change_property32(
            PropMode::REPLACE,
            id,
            backend.atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            &[backend.atoms._NET_WM_WINDOW_TYPE_NORMAL],
        )
        .unwrap()
        .check()
        .unwrap();
        conn.map_window(id).unwrap().check().unwrap();
        ids.push(id);
    }
    conn.flush().unwrap();
    for id in &ids {
        wait_for(&mut backend, *id, |w| w.eligible());
    }
    let snapshot = backend.snapshot().unwrap();
    assert_eq!(
        snapshot
            .windows
            .iter()
            .filter(|w| ids.contains(&(w.id as u32)))
            .count(),
        2
    );
    restore_round_trip(&mut backend, &ids);
    let change_state = conn
        .intern_atom(false, b"WM_CHANGE_STATE")
        .unwrap()
        .reply()
        .unwrap()
        .atom;
    backend
        .message(ids[0], change_state, [3, 0, 0, 0, 0])
        .unwrap();
    backend.conn.flush().unwrap();
    wait_for(&mut backend, ids[0], |w| w.state.minimized && !w.eligible());
    backend
        .message(
            ids[1],
            backend.atoms._NET_WM_STATE,
            [
                1,
                backend.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                backend.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                2,
                0,
            ],
        )
        .unwrap();
    backend.conn.flush().unwrap();
    wait_for(&mut backend, ids[1], |w| w.state.maximized && w.eligible());
    restore_round_trip(&mut backend, &ids);
    backend
        .message(
            ids[1],
            backend.atoms._NET_WM_STATE,
            [1, backend.atoms._NET_WM_STATE_FULLSCREEN, 0, 2, 0],
        )
        .unwrap();
    backend.conn.flush().unwrap();
    wait_for(&mut backend, ids[1], |w| {
        w.state.fullscreen && !w.eligible()
    });
    for id in ids {
        conn.destroy_window(id).unwrap().check().unwrap();
    }
    conn.flush().unwrap();
}

fn restore_round_trip(backend: &mut X11, ids: &[u32]) {
    let mut original = backend.snapshot().unwrap();
    original.windows.retain(|w| ids.contains(&(w.id as u32)));
    let plans: Vec<_> = original
        .windows
        .iter()
        .filter(|w| w.eligible())
        .map(|w| crate::workspace::Placement {
            id: w.id,
            rect: Rect {
                x: 800,
                y: 400,
                width: 360,
                height: 240,
            },
            layer: 0,
        })
        .collect();
    let eligible: Vec<_> = plans.iter().map(|p| p.id).collect();
    let order: Vec<_> = backend
        .stacking_order()
        .unwrap()
        .into_iter()
        .filter(|id| eligible.contains(id))
        .collect();
    let mut undo = crate::restore::OriginalLayout::default();
    undo.capture(backend, &original, &plans).unwrap();
    for p in &plans {
        backend.place(p.id, p.rect).unwrap();
        let before = original.windows.iter().find(|w| w.id == p.id).unwrap();
        wait_for(backend, p.id as u32, |w| {
            !w.state.maximized && w.rect != before.rect
        });
    }
    for id in order.iter().rev() {
        backend.raise(*id).unwrap();
    }
    let current = backend.snapshot().unwrap();
    let result = undo.restore(backend, &current);
    assert!(!undo.active(), "{result}");
    for before in original.windows.iter().filter(|w| eligible.contains(&w.id)) {
        wait_for(backend, before.id as u32, |w| {
            w.rect == before.rect && w.state.maximized == before.state.maximized
        });
    }
    let actual: Vec<_> = backend
        .stacking_order()
        .unwrap()
        .into_iter()
        .filter(|id| eligible.contains(id))
        .collect();
    assert_eq!(
        actual, order,
        "Not Enforce must restore the original stacking order"
    );
}
