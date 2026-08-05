//! HKCU Run-key startup toggle honoring Task Manager's StartupApproved state,
//! port of the startup section of AppHost.cs.

use super::reg::{create_hkcu as create, open_hkcu as open, query, Key};
use windows::core::w;
use windows::Win32::System::Registry::{RegDeleteValueW, RegSetValueExW, REG_BINARY, REG_SZ};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
// Task Manager's Startup-apps enable/disable state; first byte odd = disabled.
const APPROVED_KEY: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
const APP_NAME: windows::core::PCWSTR = w!("PrayerTray");

fn startup_command() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    format!("\"{}\"", exe.display())
}

fn set_string(key: &Key, name: windows::core::PCWSTR, value: &str) -> bool {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
    unsafe { RegSetValueExW(key.0, name, None, REG_SZ, Some(bytes)) }.is_ok()
}

pub fn is_enabled() -> bool {
    let Some(run) = open(RUN_KEY, false) else { return false };
    if query(&run, APP_NAME).is_none() {
        return false;
    }
    match open(APPROVED_KEY, false).and_then(|k| query(&k, APP_NAME)) {
        Some((ty, b)) if ty == REG_BINARY && !b.is_empty() => (b[0] & 1) == 0,
        _ => true, // no approved entry (or empty) = enabled
    }
}

pub fn set_enabled(enable: bool) -> bool {
    let (Some(run), Some(approved)) = (create(RUN_KEY), create(APPROVED_KEY)) else {
        return false;
    };
    if enable {
        let ok = set_string(&run, APP_NAME, &startup_command());
        let blob = [2u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let ok2 = unsafe { RegSetValueExW(approved.0, APP_NAME, None, REG_BINARY, Some(&blob)) }.is_ok();
        ok && ok2
    } else {
        unsafe {
            let _ = RegDeleteValueW(run.0, APP_NAME);
            let _ = RegDeleteValueW(approved.0, APP_NAME);
        }
        true
    }
}

/// The Run value survives the exe being moved/renamed; re-point it at the running exe on launch.
pub fn heal_path() {
    let Some(run) = open(RUN_KEY, true) else { return };
    if let Some((ty, b)) = query(&run, APP_NAME) {
        if ty == REG_SZ {
            let cur = String::from_utf16_lossy(unsafe {
                std::slice::from_raw_parts(b.as_ptr() as *const u16, b.len() / 2)
            });
            let cur = cur.trim_end_matches('\0');
            let want = startup_command();
            if cur != want {
                set_string(&run, APP_NAME, &want);
            }
        }
    }
}
