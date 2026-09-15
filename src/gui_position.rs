//! Initial GUI placement only; never follow the pointer after opening the window.
use eframe::CreationContext;

/// Position before the first frame, then center once more using the actual frame
/// size after the destination display's DPI has been applied by the event loop.
pub fn at_pointer(cc: &CreationContext<'_>) -> Option<Box<dyn FnOnce()>> {
    let pointer = pointer_position()?;
    let window = cc.winit_window()?.clone();
    let monitor = window.available_monitors().find(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        // CoreGraphics reports desktop coordinates in points; X11 and Win32
        // report physical pixels (Win32 is DPI-aware by the time this runs).
        let units = if cfg!(target_os = "macos") {
            monitor.scale_factor()
        } else {
            1.0
        };
        contains(
            [origin.x as f64 / units, origin.y as f64 / units],
            [size.width as f64 / units, size.height as f64 / units],
            pointer,
        )
    })?;

    let place = move |predict_dpi: bool| {
        let outer = window.outer_size();
        #[cfg(target_os = "macos")]
        {
            let mut position = monitor.position().to_logical::<f64>(monitor.scale_factor());
            let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
            let outer = outer.to_logical::<f64>(window.scale_factor());
            let center = centered(
                [position.x, position.y],
                [size.width, size.height],
                [outer.width, outer.height],
            );
            position.x = center[0];
            position.y = center[1];
            window.set_outer_position(position);
            let _ = predict_dpi;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut position = monitor.position();
            let size = monitor.size();
            let scale = if predict_dpi {
                monitor.scale_factor() / window.scale_factor()
            } else {
                1.0
            };
            let center = centered(
                [position.x as f64, position.y as f64],
                [size.width as f64, size.height as f64],
                [outer.width as f64 * scale, outer.height as f64 * scale],
            );
            position.x = center[0].round() as i32;
            position.y = center[1].round() as i32;
            window.set_outer_position(position);
        }
    };
    place(true);
    Some(Box::new(move || place(false)))
}

fn contains(origin: [f64; 2], size: [f64; 2], pointer: [f64; 2]) -> bool {
    (0..2).all(|axis| pointer[axis] >= origin[axis] && pointer[axis] < origin[axis] + size[axis])
}

fn centered(origin: [f64; 2], size: [f64; 2], window: [f64; 2]) -> [f64; 2] {
    // Oversized windows start at the display origin so their title bar stays reachable.
    std::array::from_fn(|axis| origin[axis] + ((size[axis] - window[axis]) / 2.0).max(0.0))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn pointer_position() -> Option<[f64; 2]> {
    use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
    // XWayland cannot position native Wayland windows or reliably query their cursor.
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return None;
    }
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots[screen].root;
    let pointer = connection.query_pointer(root).ok()?.reply().ok()?;
    pointer
        .same_screen
        .then_some([pointer.root_x as f64, pointer.root_y as f64])
}

#[cfg(windows)]
fn pointer_position() -> Option<[f64; 2]> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes to an initialized, live POINT on this stack.
    (unsafe { GetCursorPos(&mut point) } != 0).then_some([point.x as f64, point.y as f64])
}

#[cfg(target_os = "macos")]
fn pointer_position() -> Option<[f64; 2]> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const c_void) -> *const c_void;
        fn CGEventGetLocation(event: *const c_void) -> Point;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const c_void);
    }
    // SAFETY: a null source requests current input state; the returned Create-owned
    // event is checked before use and released once. No Accessibility access needed.
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);
        Some([point.x, point.y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_left_and_upper_displays_and_excludes_shared_edge() {
        assert!(contains([-1920.0, -200.0], [1920.0, 1080.0], [-1.0, 0.0]));
        assert!(!contains([-1920.0, -200.0], [1920.0, 1080.0], [0.0, 0.0]));
        assert!(contains([0.0, -1080.0], [1920.0, 1080.0], [500.0, -500.0]));
    }

    #[test]
    fn centers_scaled_window_and_keeps_oversized_title_bar_on_screen() {
        assert_eq!(
            centered([-2560.0, 0.0], [2560.0, 1440.0], [1560.0, 1320.0]),
            [-2060.0, 60.0]
        );
        assert_eq!(
            centered([1920.0, -200.0], [640.0, 480.0], [780.0, 660.0]),
            [1920.0, -200.0]
        );
    }
}
