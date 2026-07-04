//! Local date/time via Win32 (no chrono). Offset for arbitrary dates respects the
//! system's DST rules, matching C# TimeZoneInfo.Local.GetUtcOffset(date).

use crate::datetime::Date;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::Time::TzSpecificLocalTimeToSystemTime;
use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount64};

/// Milliseconds since boot (monotonic); matches C# Environment.TickCount64.
pub fn tick_count64() -> u64 {
    unsafe { GetTickCount64() }
}

/// Local date as "yyyy-MM-dd".
pub fn today_key() -> String {
    let st = unsafe { GetLocalTime() };
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}

/// Date `days` before today as "yyyy-MM-dd" (for retention cutoffs).
pub fn day_key_offset(days: i64) -> String {
    let (d, _, _) = now_local();
    let rd = d.to_rd() - days;
    let back = Date::from_rd(rd);
    format!("{:04}-{:02}-{:02}", back.year, back.month, back.day)
}

/// Current local date + minutes/seconds since midnight.
pub fn now_local() -> (Date, u32, u32) {
    let st = unsafe { GetLocalTime() };
    (
        Date::new(st.wYear as i32, st.wMonth as u32, st.wDay as u32),
        st.wHour as u32 * 60 + st.wMinute as u32,
        st.wSecond as u32,
    )
}

/// UTC offset in hours for local noon of the given date (DST-aware).
pub fn utc_offset_hours(date: Date) -> f64 {
    let local = SYSTEMTIME {
        wYear: date.year as u16,
        wMonth: date.month as u16,
        wDay: date.day as u16,
        wHour: 12,
        ..Default::default()
    };
    let mut utc = SYSTEMTIME::default();
    if unsafe { TzSpecificLocalTimeToSystemTime(None, &local, &mut utc) }.is_err() {
        return 0.0;
    }
    let local_min = Date::new(local.wYear as i32, local.wMonth as u32, local.wDay as u32).to_rd()
        * 1440
        + local.wHour as i64 * 60
        + local.wMinute as i64;
    let utc_min = Date::new(utc.wYear as i32, utc.wMonth as u32, utc.wDay as u32).to_rd() * 1440
        + utc.wHour as i64 * 60
        + utc.wMinute as i64;
    (local_min - utc_min) as f64 / 60.0
}
