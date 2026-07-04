//! Monitor enumeration (WinForms Screen equivalent) + CCD friendly names, port of Native/Displays.cs.

use std::collections::HashMap;
use windows::core::PCWSTR;
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow,
    DISPLAY_DEVICEW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};

fn wsz(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub device: String,
    pub bounds: RECT,
    pub work: RECT,
    pub primary: bool,
}

fn info_of(h: HMONITOR) -> Option<Monitor> {
    let mut mi = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(h, &mut mi.monitorInfo as *mut MONITORINFO) }
        .as_bool()
        .then(|| {
            Monitor {
                device: wsz(&mi.szDevice),
                bounds: mi.monitorInfo.rcMonitor,
                work: mi.monitorInfo.rcWork,
                primary: (mi.monitorInfo.dwFlags & 1) != 0, // MONITORINFOF_PRIMARY
            }
        })
}

pub fn all() -> Vec<Monitor> {
    unsafe extern "system" fn cb(h: HMONITOR, _dc: HDC, _rc: *mut RECT, lp: LPARAM) -> windows::core::BOOL {
        let list = unsafe { &mut *(lp.0 as *mut Vec<Monitor>) };
        if let Some(m) = info_of(h) {
            list.push(m);
        }
        true.into()
    }
    let mut list: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut list as *mut _ as isize));
    }
    list
}

pub fn primary() -> Option<Monitor> {
    let list = all();
    list.iter().find(|m| m.primary).cloned().or_else(|| list.into_iter().next())
}

pub fn from_window(h: HWND) -> Option<Monitor> {
    info_of(unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) })
}

pub fn at_point(pt: POINT) -> Option<Monitor> {
    info_of(unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) })
}

/// CCD map: GDI source name (`\\.\DISPLAYx`) -> monitor friendly name ("DELL U2419H").
/// Not called on the widget reposition path — CCD costs a few syscalls per query.
pub fn friendly_names() -> HashMap<String, String> {
    let mut map = HashMap::new();
    unsafe {
        let (mut n_path, mut n_mode) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_path, &mut n_mode) != ERROR_SUCCESS {
            return map;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); n_path as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_mode as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut n_path,
            paths.as_mut_ptr(),
            &mut n_mode,
            modes.as_mut_ptr(),
            None,
        ) != ERROR_SUCCESS
        {
            return map;
        }
        for p in &paths[..(n_path as usize).min(paths.len())] {
            let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: p.sourceInfo.adapterId,
                    id: p.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                continue;
            }
            let mut tgt = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                    adapterId: p.targetInfo.adapterId,
                    id: p.targetInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut tgt.header) != 0 {
                continue;
            }
            let gdi = wsz(&src.viewGdiDeviceName);
            let name = wsz(&tgt.monitorFriendlyDeviceName);
            if !gdi.trim().is_empty() && !name.trim().is_empty() {
                map.insert(gdi, name);
            }
        }
    }
    map
}

/// EnumDisplayDevices adapter string (e.g. "Generic PnP Monitor") when CCD has no name.
pub fn fallback_name(device: &str) -> String {
    let wide: Vec<u16> = device.encode_utf16().chain(std::iter::once(0)).collect();
    let mut dd = DISPLAY_DEVICEW {
        cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    if unsafe { EnumDisplayDevicesW(PCWSTR(wide.as_ptr()), 0, &mut dd, 0) }.as_bool() {
        let s = wsz(&dd.DeviceString);
        if !s.trim().is_empty() {
            return s;
        }
    }
    device.trim_start_matches("\\\\.\\").to_string()
}

/// Combo label for a monitor: CCD friendly name, else adapter string, never blank.
pub fn friendly_label(m: &Monitor, names: &HashMap<String, String>) -> String {
    names
        .iter()
        .find(|(k, v)| k.eq_ignore_ascii_case(&m.device) && !v.trim().is_empty())
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| fallback_name(&m.device))
}

/// Monitor by device name (case-insensitive), else primary — WinForms TargetScreen semantics.
pub fn by_device(device: Option<&str>) -> Option<Monitor> {
    if let Some(d) = device {
        if !d.is_empty() {
            if let Some(m) = all().into_iter().find(|m| m.device.eq_ignore_ascii_case(d)) {
                return Some(m);
            }
        }
    }
    primary()
}
