//! NSScreen work areas, converted from Cocoa bottom-left to AX top-left coordinates.
use crate::layout::Rect;
use anyhow::{Result, ensure};
use std::ffi::{c_char, c_void};

type Object = *mut c_void;
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
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Bounds {
    origin: Point,
    size: Size,
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Object;
    fn sel_registerName(name: *const c_char) -> Object;
    fn objc_msgSend();
    #[cfg(target_arch = "x86_64")]
    fn objc_msgSend_stret();
    fn objc_autoreleasePoolPush() -> Object;
    fn objc_autoreleasePoolPop(pool: Object);
}

struct Pool(Object);
impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            objc_autoreleasePoolPop(self.0);
        }
    }
}

unsafe fn object(receiver: Object, selector: &'static std::ffi::CStr) -> Object {
    // SAFETY: each caller supplies an Objective-C method returning an object pointer.
    let send: unsafe extern "C" fn(Object, Object) -> Object =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { send(receiver, sel_registerName(selector.as_ptr())) }
}

unsafe fn bounds(receiver: Object, selector: &'static std::ffi::CStr) -> Bounds {
    // NSRect is four CGFloat doubles on both supported 64-bit architectures.
    // x86_64 uses the dedicated structure-return ABI; arm64 uses ordinary msgSend.
    #[cfg(target_arch = "x86_64")]
    {
        let send: unsafe extern "C" fn(*mut Bounds, Object, Object) =
            unsafe { std::mem::transmute(objc_msgSend_stret as *const ()) };
        let mut result = Bounds::default();
        unsafe {
            send(&mut result, receiver, sel_registerName(selector.as_ptr()));
        }
        result
    }
    #[cfg(target_arch = "aarch64")]
    {
        let send: unsafe extern "C" fn(Object, Object) -> Bounds =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { send(receiver, sel_registerName(selector.as_ptr())) }
    }
}

pub fn monitors() -> Result<Vec<(Rect, String)>> {
    // This is called by the service on its main thread. All Cocoa objects remain
    // borrowed inside this autorelease pool and no references escape it.
    unsafe {
        let _pool = Pool(objc_autoreleasePoolPush());
        let class = objc_getClass(c"NSScreen".as_ptr());
        ensure!(!class.is_null(), "NSScreen unavailable");
        let screens = object(class, c"screens");
        ensure!(!screens.is_null(), "No screens available");
        let count: unsafe extern "C" fn(Object, Object) -> usize =
            std::mem::transmute(objc_msgSend as *const ());
        let at: unsafe extern "C" fn(Object, Object, usize) -> Object =
            std::mem::transmute(objc_msgSend as *const ());
        let length = count(screens, sel_registerName(c"count".as_ptr()));
        ensure!(length > 0 && length <= 64, "No usable display list");
        let first = at(screens, sel_registerName(c"objectAtIndex:".as_ptr()), 0);
        let primary = bounds(first, c"frame");
        let top = primary.origin.y + primary.size.height;
        Ok((0..length)
            .map(|index| {
                let screen = at(screens, sel_registerName(c"objectAtIndex:".as_ptr()), index);
                let visible = bounds(screen, c"visibleFrame");
                let area = Rect {
                    x: visible.origin.x.round() as i32,
                    y: (top - visible.origin.y - visible.size.height).round() as i32,
                    width: visible.size.width.round() as i32,
                    height: visible.size.height.round() as i32,
                };
                (area, brand(screen))
            })
            .collect())
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayVendorNumber(display: u32) -> u32;
}

unsafe fn brand(screen: Object) -> String {
    // SAFETY: typed Objective-C calls match NSString, NSDictionary and NSNumber methods.
    unsafe {
        let send_arg: unsafe extern "C" fn(Object, Object, Object) -> Object =
            std::mem::transmute(objc_msgSend as *const ());
        let key = send_arg(
            objc_getClass(c"NSString".as_ptr()),
            sel_registerName(c"stringWithUTF8String:".as_ptr()),
            c"NSScreenNumber".as_ptr() as Object,
        );
        let number = send_arg(
            object(screen, c"deviceDescription"),
            sel_registerName(c"objectForKey:".as_ptr()),
            key,
        );
        if number.is_null() {
            return "Unknown brand".into();
        }
        let unsigned: unsafe extern "C" fn(Object, Object) -> u32 =
            std::mem::transmute(objc_msgSend as *const ());
        let display = unsigned(number, sel_registerName(c"unsignedIntValue".as_ptr()));
        super::display_brand::vendor(CGDisplayVendorNumber(display) as u16)
    }
}
