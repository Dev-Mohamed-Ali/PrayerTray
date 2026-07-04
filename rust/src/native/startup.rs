//! HKCU Run-key startup toggle honoring Task Manager's StartupApproved state,
//! port of the startup section of AppHost.cs.

use windows::core::w;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_BINARY,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
// Task Manager's Startup-apps enable/disable state; first byte odd = disabled.
const APPROVED_KEY: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
const APP_NAME: windows::core::PCWSTR = w!("PrayerTray");

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn open(path: windows::core::PCWSTR, write: bool) -> Option<Key> {
    let mut h = HKEY::default();
    let access = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path, None, access, &mut h) };
    r.is_ok().then_some(Key(h))
}

fn create(path: windows::core::PCWSTR) -> Option<Key> {
    let mut h = HKEY::default();
    let r = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut h,
            None,
        )
    };
    r.is_ok().then_some(Key(h))
}

fn query(key: &Key, name: windows::core::PCWSTR) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
    let mut ty = REG_VALUE_TYPE::default();
    let mut len = 0u32;
    unsafe { RegQueryValueExW(key.0, name, None, Some(&mut ty), None, Some(&mut len)) }
        .is_ok()
        .then(|| {
            let mut buf = vec![0u8; len as usize];
            let mut len2 = len;
            unsafe {
                RegQueryValueExW(key.0, name, None, Some(&mut ty), Some(buf.as_mut_ptr()), Some(&mut len2))
            }
            .is_ok()
            .then(|| {
                buf.truncate(len2 as usize);
                (ty, buf)
            })
        })
        .flatten()
}

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
