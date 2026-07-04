//! Taskbar lookup/geometry, fullscreen detection, DPI, z-order — port of Native/Interop.cs.

use super::displays;
use windows::core::w;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetClassNameW, GetForegroundWindow, GetWindowRect,
    GetWindowThreadProcessId, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

pub fn taskbar() -> Option<HWND> {
    unsafe { FindWindowW(w!("Shell_TrayWnd"), None).ok().filter(|h| !h.is_invalid()) }
}

/// Taskbar window for a monitor device: primary's Shell_TrayWnd, or the matching
/// Shell_SecondaryTrayWnd; falls back to primary.
pub fn taskbar_for_device(device: Option<&str>) -> Option<HWND> {
    let primary = taskbar();
    let Some(d) = device.filter(|d| !d.is_empty()) else { return primary };
    if let Some(p) = displays::primary() {
        if p.device.eq_ignore_ascii_case(d) {
            return primary;
        }
    }
    let mut h = HWND::default();
    loop {
        h = match unsafe { FindWindowExW(None, Some(h), w!("Shell_SecondaryTrayWnd"), None) } {
            Ok(w) if !w.is_invalid() => w,
            _ => return primary,
        };
        if let Some(m) = displays::from_window(h) {
            if m.device.eq_ignore_ascii_case(d) {
                return Some(h);
            }
        }
    }
}

pub fn window_rect(h: HWND) -> Option<RECT> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(h, &mut r) }.is_ok().then_some(r)
}

/// Screen-x of the taskbar's tray (clock cluster) left edge, or 0 if not found.
pub fn tray_notify_left(taskbar: HWND) -> i32 {
    match unsafe { FindWindowExW(Some(taskbar), None, w!("TrayNotifyWnd"), None) } {
        Ok(h) if !h.is_invalid() => window_rect(h).map(|r| r.left).unwrap_or(0),
        _ => 0,
    }
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// True if the foreground window is a real app covering the whole given monitor.
pub fn is_fullscreen_app_on(bounds: &RECT) -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return false;
    }
    let Some(r) = window_rect(fg) else { return false };
    match class_name(fg).as_str() {
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "WorkerW" | "Progman" => return false,
        _ => {}
    }
    r.left <= bounds.left && r.top <= bounds.top && r.right >= bounds.right && r.bottom >= bounds.bottom
}

/// Re-assert top-most z-order without moving/activating.
pub fn raise_topmost(h: HWND) {
    unsafe {
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

pub fn scale_of(h: HWND) -> f32 {
    if h.is_invalid() {
        return 1.0;
    }
    let dpi = unsafe { GetDpiForWindow(h) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

pub fn thread_of(h: HWND) -> u32 {
    unsafe { GetWindowThreadProcessId(h, None) }
}
