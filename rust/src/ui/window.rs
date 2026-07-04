//! Window-class plumbing: a wndproc trampoline that routes to a boxed handler via GWLP_USERDATA.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowLongPtrW, RegisterClassW, SetWindowLongPtrW,
    CW_USEDEFAULT, GWLP_USERDATA, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE, WM_NCCREATE, WNDCLASSW,
    CREATESTRUCTW,
};

pub trait WindowHandler {
    fn message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT>;
}

pub fn utf16z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn trampoline<H: WindowHandler>(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = lparam.0 as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut H;
        if !ptr.is_null() {
            if let Some(r) = (*ptr).message(hwnd, msg, wparam, lparam) {
                return r;
            }
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

/// Registers `class_name` (once) and creates a window whose messages route to `handler`.
/// The handler must outlive the window; the caller keeps ownership.
pub fn create<H: WindowHandler>(
    class_name: PCWSTR,
    title: PCWSTR,
    ex_style: WINDOW_EX_STYLE,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    parent: Option<HWND>,
    handler: *mut H,
) -> windows::core::Result<HWND> {
    unsafe {
        let hinst = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(trampoline::<H>),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc); // 0 = already registered; CreateWindowExW will fail if truly broken

        CreateWindowExW(
            ex_style,
            class_name,
            title,
            style,
            if x == i32::MIN { CW_USEDEFAULT } else { x },
            if y == i32::MIN { CW_USEDEFAULT } else { y },
            w,
            h,
            parent,
            None::<HMENU>,
            Some(hinst.into()),
            Some(handler as *const core::ffi::c_void),
        )
    }
}

#[allow(unused)]
pub const HIDDEN_CLASS: PCWSTR = w!("PrayerTrayHidden");
