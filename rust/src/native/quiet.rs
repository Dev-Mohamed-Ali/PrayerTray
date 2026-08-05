//! Is this a bad moment for sound? Shell notification state plus a microphone-in-use probe
//! (a windowed Teams/Zoom call is invisible to SHQueryUserNotificationState).

use super::reg;
use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::Win32::UI::Shell::SHQueryUserNotificationState;

const CONSENT_MIC: PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Busy {
    Call,
    Fullscreen,
    Presenting,
    DoNotDisturb,
}

/// Decision table over a raw QUERY_USER_NOTIFICATION_STATE.
pub fn classify(quns: i32, mic_in_use: bool) -> Option<Busy> {
    if mic_in_use {
        return Some(Busy::Call);
    }
    match quns {
        2 | 3 | 7 => Some(Busy::Fullscreen), // BUSY | RUNNING_D3D_FULL_SCREEN | APP
        4 => Some(Busy::Presenting),         // PRESENTATION_MODE
        6 => Some(Busy::DoNotDisturb),       // QUIET_TIME — Focus assist / Do not disturb
        _ => None,                           // 1 = away (locked/screensaver), 5 = accepting
    }
}

pub fn state() -> Option<Busy> {
    let quns = unsafe { SHQueryUserNotificationState() }.map(|s| s.0).unwrap_or(5);
    classify(quns, mic_in_use())
}

fn mic_in_use() -> bool {
    let Some(root) = reg::open(HKEY_CURRENT_USER, CONSENT_MIC, false) else { return false };
    holds_mic(&root) || reg::open_sub(&root, "NonPackaged").is_some_and(|k| holds_mic(&k))
}

/// An app holds the mic while its ConsentStore entry has a start time and no stop time.
fn holds_mic(parent: &reg::Key) -> bool {
    reg::subkey_names(parent).into_iter().any(|name| {
        if name.eq_ignore_ascii_case("NonPackaged") {
            return false;
        }
        reg::open_sub(parent, &name).is_some_and(|k| {
            let start = reg::query_u64(&k, w!("LastUsedTimeStart")).unwrap_or(0);
            let stop = reg::query_u64(&k, w!("LastUsedTimeStop")).unwrap_or(0);
            start != 0 && stop == 0
        })
    })
}
