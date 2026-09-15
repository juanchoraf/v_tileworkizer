use super::{Backend, Snapshot, Window};
use crate::layout::Rect;
use anyhow::{Result, ensure};
use std::{mem::size_of, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::Console::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

pub struct Windows;
impl Windows {
    pub fn new() -> Self {
        // SAFETY: process-wide initialization before any service window API calls.
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        Self
    }
}

pub fn powershell(args: &[String]) -> Result<bool> {
    if args.iter().any(|a| a == "--service" || a == "--supervise") {
        return Ok(false);
    }
    if std::env::var_os("V_TILEWORKIZER_POWERSHELL").is_some() {
        // SAFETY: console handles are borrowed, and mode points to initialized storage.
        unsafe {
            let output = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(output, &mut mode) != 0 {
                SetConsoleMode(output, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
        return Ok(false);
    }
    let quote = |s: &str| v_concat::v_concat!("'{}'", s.replace('\'', "''"));
    let exe = std::env::current_exe()?;
    let command = v_concat::v_concat!(
        "& {} {}; exit $LASTEXITCODE",
        quote(&exe.to_string_lossy()),
        args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
    );
    let shell = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let status = std::process::Command::new(shell)
        .env("V_TILEWORKIZER_POWERSHELL", "1")
        .args(["-NoLogo", "-NoProfile", "-Command", &command])
        .status()?;
    ensure!(
        status.success(),
        "PowerShell application exited with {status}"
    );
    Ok(true)
}

unsafe extern "system" fn collect_window(hwnd: HWND, data: LPARAM) -> i32 {
    // SAFETY: EnumWindows calls synchronously; data is the live exclusive Vec pointer
    // passed by snapshot. All HWND-dependent results are checked before use.
    unsafe {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let extended = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let protected = style & WS_THICKFRAME == 0
            || extended & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0
            || !GetWindow(hwnd, GW_OWNER).is_null();
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast(),
            4,
        );
        let mut title = [0u16; 1024];
        let count = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        if count <= 0 {
            return 1;
        }
        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return 1;
        }
        // Borderless/fullscreen windows are deliberately left under application control.
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        let fullscreen = GetMonitorInfoW(monitor, &mut info) != 0
            && rect.left <= info.rcMonitor.left
            && rect.top <= info.rcMonitor.top
            && rect.right >= info.rcMonitor.right
            && rect.bottom >= info.rcMonitor.bottom
            && IsZoomed(hwnd) == 0
            && IsIconic(hwnd) == 0;
        (&mut *(data as *mut Vec<Window>)).push(Window {
            state: super::WindowState {
                minimized: IsIconic(hwnd) != 0,
                maximized: IsZoomed(hwnd) != 0,
                fullscreen,
                hidden: cloaked != 0,
                protected,
                floating: None,
            },
            session: String::new(),
            reconnects: Vec::new(),
            app_id: super::app_identity::windows(hwnd),
            id: hwnd as usize as u64,
            title: String::from_utf16_lossy(&title[..count as usize]),
            rect: Rect {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            },
        });
    }
    1
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> i32 {
    // SAFETY: EnumDisplayMonitors lends the live vector for this synchronous callback.
    unsafe {
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info.monitorInfo) != 0 {
            let mut device: DISPLAY_DEVICEW = std::mem::zeroed();
            device.cb = size_of::<DISPLAY_DEVICEW>() as u32;
            let mut brand = "Unknown brand".to_owned();
            if EnumDisplayDevicesW(info.szDevice.as_ptr(), 0, &mut device, 0) != 0 {
                let end = device
                    .DeviceID
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(device.DeviceID.len());
                let id = String::from_utf16_lossy(&device.DeviceID[..end]);
                if let Some(code) = id.split('\\').nth(1).and_then(|s| s.get(..3)) {
                    brand = super::display_brand::pnp(code);
                }
            }
            let r = info.monitorInfo.rcWork;
            (&mut *(data as *mut Vec<(Rect, String)>)).push((
                Rect {
                    x: r.left,
                    y: r.top,
                    width: r.right - r.left,
                    height: r.bottom - r.top,
                },
                brand,
            ));
        }
    }
    1
}

impl Backend for Windows {
    fn stacking_order(&mut self) -> Result<Vec<u64>> {
        Ok(self
            .snapshot()?
            .windows
            .iter()
            .rev()
            .map(|w| w.id)
            .collect())
    }
    fn restore_window(&mut self, window: &Window) -> Result<()> {
        self.place(window.id, window.rect)?;
        if window.state.maximized {
            // SAFETY: this is a borrowed HWND; Windows validates it.
            unsafe {
                ShowWindowAsync(window.id as usize as HWND, SW_MAXIMIZE);
            }
        }
        Ok(())
    }
    fn raise(&mut self, id: u64) -> Result<()> {
        // SAFETY: Win32 validates the borrowed window handle; this sets no topmost flag.
        unsafe {
            ensure!(
                SetWindowPos(
                    id as usize as HWND,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS
                ) != 0,
                "Cannot change window stacking: {}",
                std::io::Error::last_os_error()
            );
        }
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        let mut windows: Vec<Window> = Vec::new();
        let mut displays: Vec<(Rect, String)> = Vec::new();
        // SAFETY: callbacks only use these vectors during these synchronous calls.
        unsafe {
            ensure!(
                EnumWindows(
                    Some(collect_window),
                    (&mut windows as *mut Vec<Window>) as isize
                ) != 0,
                "EnumWindows failed: {}",
                std::io::Error::last_os_error()
            );
            ensure!(
                EnumDisplayMonitors(
                    null_mut(),
                    std::ptr::null(),
                    Some(collect_monitor),
                    (&mut displays as *mut Vec<(Rect, String)>) as isize
                ) != 0,
                "Monitor enumeration failed"
            );
        }
        let (monitors, monitor_brands) = displays.into_iter().unzip();
        Ok(Snapshot {
            monitor_brands,
            windows,
            monitors,
            workspace: 0,
        })
    }

    fn place(&mut self, id: u64, rect: Rect) -> Result<()> {
        let hwnd = id as usize as HWND;
        // SAFETY: OS checks the handle on every call. No pointer is dereferenced by Rust.
        unsafe {
            if IsWindow(hwnd) == 0 {
                return Ok(());
            }
            if IsZoomed(hwnd) != 0 {
                ShowWindowAsync(hwnd, SW_RESTORE);
            }
            ensure!(
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_ASYNCWINDOWPOS
                ) != 0,
                "Cannot move window (another privilege level may own it): {}",
                std::io::Error::last_os_error()
            );
        }
        Ok(())
    }
}
