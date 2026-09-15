// CoreFoundation values follow Create/Copy ownership; Owned releases exactly once.
// Accessibility references stay alive in the window map until the next snapshot.
use super::{Backend, Snapshot, Window};
use crate::layout::Rect;
use anyhow::{Result, ensure};
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_void},
    ptr,
};

type Ref = *const c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    width: f64,
    height: f64,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Ref);
    fn CFRetain(value: Ref) -> Ref;
    fn CFEqual(left: Ref, right: Ref) -> bool;
    fn CFStringCreateWithCString(alloc: Ref, text: *const c_char, encoding: u32) -> Ref;
    fn CFStringGetCString(value: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
    fn CFArrayGetCount(value: Ref) -> isize;
    fn CFArrayGetValueAtIndex(value: Ref, index: isize) -> Ref;
    fn CFDictionaryGetValue(value: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(value: Ref, kind: isize, result: *mut c_void) -> bool;
    fn CFBooleanGetValue(value: Ref) -> bool;
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> Ref;
    fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    fn AXUIElementSetAttributeValue(element: Ref, attribute: Ref, value: Ref) -> i32;
    fn AXUIElementPerformAction(element: Ref, action: Ref) -> i32;
    fn AXUIElementIsAttributeSettable(element: Ref, attribute: Ref, settable: *mut bool) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
    fn AXValueCreate(kind: u32, value: *const c_void) -> Ref;
    fn AXValueGetValue(value: Ref, kind: u32, result: *mut c_void) -> bool;
    fn CGWindowListCopyWindowInfo(option: u32, relative: u32) -> Ref;
}

struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}
fn string(text: &str) -> Owned {
    let text = std::ffi::CString::new(text).expect("constant AX attribute");
    Owned(unsafe { CFStringCreateWithCString(ptr::null(), text.as_ptr(), 0x08000100) })
}
fn attribute(element: Ref, name: &str) -> Option<Owned> {
    let mut value = ptr::null();
    let status = unsafe { AXUIElementCopyAttributeValue(element, string(name).0, &mut value) };
    if status == 0 && !value.is_null() {
        Some(Owned(value))
    } else {
        None
    }
}
fn boolean(element: Ref, name: &str) -> bool {
    attribute(element, name).is_some_and(|v| unsafe { CFBooleanGetValue(v.0) })
}
fn text(value: Ref) -> String {
    let mut bytes = [0u8; 4096];
    if unsafe {
        CFStringGetCString(
            value,
            bytes.as_mut_ptr().cast(),
            bytes.len() as isize,
            0x08000100,
        )
    } {
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    } else {
        String::new()
    }
}

pub struct MacOS {
    windows: BTreeMap<u64, Owned>,
    next_id: u64,
    stack: Vec<u64>,
}
impl MacOS {
    pub fn connect() -> Result<Self> {
        ensure!(
            unsafe { AXIsProcessTrusted() },
            "Grant v_tileworkizer Accessibility access in System Settings → Privacy & Security → Accessibility, then retry"
        );
        Ok(Self {
            windows: BTreeMap::new(),
            next_id: 1,
            stack: Vec::new(),
        })
    }

    fn window(&mut self, value: Ref, id: u64) -> Option<Window> {
        let standard =
            attribute(value, "AXSubrole").is_some_and(|v| text(v.0) == "AXStandardWindow");
        let mut settable = false;
        let protected =
            unsafe { AXUIElementIsAttributeSettable(value, string("AXSize").0, &mut settable) }
                != 0
                || !settable
                || !standard;
        let position = attribute(value, "AXPosition")?;
        let size = attribute(value, "AXSize")?;
        let mut p = Point::default();
        let mut s = Size::default();
        if !unsafe { AXValueGetValue(position.0, 1, (&mut p as *mut Point).cast()) }
            || !unsafe { AXValueGetValue(size.0, 2, (&mut s as *mut Size).cast()) }
        {
            return None;
        }
        self.windows.insert(id, Owned(unsafe { CFRetain(value) }));
        Some(Window {
            state: super::WindowState {
                minimized: boolean(value, "AXMinimized"),
                fullscreen: boolean(value, "AXFullScreen"),
                protected,
                ..Default::default()
            },
            session: String::new(),
            reconnects: Vec::new(),
            app_id: String::new(),
            id,
            title: attribute(value, "AXTitle")
                .map(|v| text(v.0))
                .unwrap_or_default(),
            rect: Rect {
                x: p.x as i32,
                y: p.y as i32,
                width: s.width as i32,
                height: s.height as i32,
            },
        })
    }
}

impl Backend for MacOS {
    fn stacking_order(&mut self) -> Result<Vec<u64>> {
        Ok(self.stack.clone())
    }
    fn raise(&mut self, id: u64) -> Result<()> {
        let Some(window) = self.windows.get(&id) else {
            return Ok(());
        };
        // SAFETY: the window is retained in the current snapshot and action is a CFString.
        let result = unsafe { AXUIElementPerformAction(window.0, string("AXRaise").0) };
        ensure!(result == 0, "Application refused stacking ({result})");
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        ensure!(
            unsafe { AXIsProcessTrusted() },
            "Accessibility permission was revoked; grant access in System Settings and retry"
        );
        let previous = std::mem::take(&mut self.windows);
        // Include offscreen windows so minimized windows and other Spaces remain visible in inventory.
        let list = Owned(unsafe { CGWindowListCopyWindowInfo(16, 0) });
        ensure!(!list.0.is_null(), "Cannot enumerate desktop windows");
        let mut ranks = BTreeMap::new();
        let mut window_ranks = Vec::new();
        let mut onscreen = std::collections::BTreeSet::new();
        let pid_key = string("kCGWindowOwnerPID");
        let layer_key = string("kCGWindowLayer");
        let bounds_key = string("kCGWindowBounds");
        let number_key = string("kCGWindowNumber");
        let mut visible: BTreeMap<i32, Vec<(u64, Rect)>> = BTreeMap::new();
        for i in 0..unsafe { CFArrayGetCount(list.0) } {
            let info = unsafe { CFArrayGetValueAtIndex(list.0, i) };
            let pid_value = unsafe { CFDictionaryGetValue(info, pid_key.0) };
            let layer_value = unsafe { CFDictionaryGetValue(info, layer_key.0) };
            if pid_value.is_null() || layer_value.is_null() {
                continue;
            }
            let mut pid: i32 = 0;
            let mut layer: i32 = 0;
            unsafe {
                CFNumberGetValue(pid_value, 3, (&mut pid as *mut i32).cast());
                CFNumberGetValue(layer_value, 3, (&mut layer as *mut i32).cast());
            }
            if layer != 0 || pid == std::process::id() as i32 {
                continue;
            }
            let number = unsafe { CFDictionaryGetValue(info, number_key.0) };
            if number.is_null() {
                continue;
            }
            let mut id: i64 = 0;
            unsafe {
                CFNumberGetValue(number, 4, (&mut id as *mut i64).cast());
            }
            ranks.insert(id as u64, i);
            let shown = unsafe { CFDictionaryGetValue(info, string("kCGWindowIsOnscreen").0) };
            if !shown.is_null() && unsafe { CFBooleanGetValue(shown) } {
                onscreen.insert(id as u64);
            }
            let bounds = unsafe { CFDictionaryGetValue(info, bounds_key.0) };
            if bounds.is_null() {
                continue;
            }
            let read = |key| {
                let v = unsafe { CFDictionaryGetValue(bounds, string(key).0) };
                let mut n: f64 = 0.0;
                if !v.is_null() {
                    unsafe {
                        CFNumberGetValue(v, 6, (&mut n as *mut f64).cast());
                    }
                }
                n as i32
            };
            visible.entry(pid).or_default().push((
                id as u64,
                Rect {
                    x: read("X"),
                    y: read("Y"),
                    width: read("Width"),
                    height: read("Height"),
                },
            ));
        }
        let mut windows = Vec::new();
        for (pid, bounds) in visible {
            let app_id = super::app_identity::macos(pid);
            let app = Owned(unsafe { AXUIElementCreateApplication(pid) });
            if app.0.is_null() {
                continue;
            }
            unsafe {
                AXUIElementSetMessagingTimeout(app.0, 0.2);
            }
            let Some(list) = attribute(app.0, "AXWindows") else {
                continue;
            };
            for i in 0..unsafe { CFArrayGetCount(list.0) } {
                let value = unsafe { CFArrayGetValueAtIndex(list.0, i) };
                // AX references, rather than geometry, identify overlapping windows.
                // Apple documents CFEqual support for AXUIElement references.
                let id = previous
                    .iter()
                    .find(|(_, old)| unsafe { CFEqual(old.0, value) })
                    .map(|(id, _)| *id)
                    .unwrap_or_else(|| {
                        let id = self.next_id;
                        self.next_id += 1;
                        id
                    });
                if let Some(mut window) = self.window(value, id) {
                    window.app_id = app_id.clone();
                    // CG supplies visibility only. Ambiguous geometry never changes identity.
                    window.state.hidden = !bounds.iter().any(|(cg_id, b)| {
                        onscreen.contains(cg_id)
                            && (i64::from(b.x) - i64::from(window.rect.x)).abs() <= 2
                            && (i64::from(b.y) - i64::from(window.rect.y)).abs() <= 2
                            && (i64::from(b.width) - i64::from(window.rect.width)).abs() <= 2
                            && (i64::from(b.height) - i64::from(window.rect.height)).abs() <= 2
                    });
                    if let Some((cg_id, _)) = bounds.iter().find(|(_, b)| *b == window.rect)
                        && let Some(rank) = ranks.get(cg_id)
                    {
                        window_ranks.push((*rank, window.id));
                    }
                    windows.push(window);
                }
            }
        }
        window_ranks.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
        self.stack = window_ranks.into_iter().map(|(_, id)| id).collect();
        let (monitors, monitor_brands): (Vec<_>, Vec<_>) =
            super::macos_screens::monitors()?.into_iter().unzip();
        for window in &mut windows {
            // macOS has no universal maximized flag; report work-area-sized windows.
            window.state.maximized = !window.state.fullscreen
                && !window.state.minimized
                && monitors.iter().any(|r| *r == window.rect);
        }
        Ok(Snapshot {
            monitor_brands,
            windows,
            monitors,
            workspace: 0,
        })
    }

    fn place(&mut self, id: u64, rect: Rect) -> Result<()> {
        let Some(window) = self.windows.get(&id) else {
            return Ok(());
        };
        let p = Point {
            x: rect.x as f64,
            y: rect.y as f64,
        };
        let s = Size {
            width: rect.width as f64,
            height: rect.height as f64,
        };
        let position = Owned(unsafe { AXValueCreate(1, (&p as *const Point).cast()) });
        let size = Owned(unsafe { AXValueCreate(2, (&s as *const Size).cast()) });
        ensure!(
            !position.0.is_null() && !size.0.is_null(),
            "Cannot allocate AX geometry"
        );
        let move_result =
            unsafe { AXUIElementSetAttributeValue(window.0, string("AXPosition").0, position.0) };
        let size_result =
            unsafe { AXUIElementSetAttributeValue(window.0, string("AXSize").0, size.0) };
        ensure!(
            move_result == 0 && size_result == 0,
            "Application refused window geometry ({move_result}, {size_result})"
        );
        Ok(())
    }
}
