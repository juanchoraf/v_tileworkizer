#[cfg(windows)]
pub(super) fn windows(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::{
        Foundation::CloseHandle, System::Threading::*,
        UI::WindowsAndMessaging::GetWindowThreadProcessId,
    };
    // SAFETY: Windows validates the borrowed HWND. The owned process handle is
    // closed on every path; output storage remains alive for the API call.
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return String::new();
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return String::new();
        }
        let mut path = vec![0_u16; 32768];
        let mut size = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size);
        CloseHandle(process);
        if ok == 0 {
            return String::new();
        }
        v_concat::v_concat!(
            "windows:{}",
            String::from_utf16_lossy(&path[..size as usize]).to_lowercase()
        )
    }
}

#[cfg(target_os = "macos")]
pub(super) fn macos(pid: i32) -> String {
    use std::ffi::c_void;
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidpath(pid: i32, buffer: *mut c_void, buffersize: u32) -> i32;
    }
    let mut path = [0_u8; 4096];
    // SAFETY: libproc writes at most the given capacity into live writable storage.
    let length = unsafe { proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    if length <= 0 {
        return String::new();
    }
    let end = path
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(path.len());
    v_concat::v_concat!("macos:{}", String::from_utf8_lossy(&path[..end]))
}
