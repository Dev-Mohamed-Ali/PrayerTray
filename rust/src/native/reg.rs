//! Minimal registry wrappers shared by the startup toggle and the busy-state probe.

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE,
};

pub struct Key(pub HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn open(root: HKEY, path: PCWSTR, write: bool) -> Option<Key> {
    let mut h = HKEY::default();
    let access = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    let r = unsafe { RegOpenKeyExW(root, path, None, access, &mut h) };
    r.is_ok().then_some(Key(h))
}

pub fn open_hkcu(path: PCWSTR, write: bool) -> Option<Key> {
    open(HKEY_CURRENT_USER, path, write)
}

pub fn open_sub(parent: &Key, name: &str) -> Option<Key> {
    let w = wide(name); // must outlive the call; PCWSTR only borrows
    open(parent.0, PCWSTR(w.as_ptr()), false)
}

pub fn create_hkcu(path: PCWSTR) -> Option<Key> {
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

pub fn query(key: &Key, name: PCWSTR) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
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

pub fn query_u64(key: &Key, name: PCWSTR) -> Option<u64> {
    let (_, b) = query(key, name)?;
    (b.len() >= 8).then(|| u64::from_le_bytes(b[..8].try_into().unwrap()))
}

pub fn subkey_names(key: &Key) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0.. {
        let mut buf = [0u16; 260]; // registry key names cap at 255 chars
        let mut len = buf.len() as u32;
        let r = unsafe {
            RegEnumKeyExW(key.0, i, Some(windows::core::PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None)
        };
        if r.is_err() {
            break;
        }
        out.push(String::from_utf16_lossy(&buf[..len as usize]));
    }
    out
}
