//! Busy-state decision table (QUERY_USER_NOTIFICATION_STATE + mic probe -> mute reason).

#![cfg(windows)]

use prayertray::native::quiet::{classify, state, Busy};

const NOT_PRESENT: i32 = 1;
const BUSY: i32 = 2;
const D3D_FULL_SCREEN: i32 = 3;
const PRESENTATION: i32 = 4;
const ACCEPTS: i32 = 5;
const QUIET_TIME: i32 = 6;
const APP: i32 = 7;

#[test]
fn free_states_allow_sound() {
    assert_eq!(classify(ACCEPTS, false), None);
    assert_eq!(classify(NOT_PRESENT, false), None); // away from the desk, not busy
    assert_eq!(classify(99, false), None); // unknown future state must not silence the app
}

#[test]
fn busy_states_mute() {
    assert_eq!(classify(BUSY, false), Some(Busy::Fullscreen));
    assert_eq!(classify(D3D_FULL_SCREEN, false), Some(Busy::Fullscreen));
    assert_eq!(classify(APP, false), Some(Busy::Fullscreen));
    assert_eq!(classify(PRESENTATION, false), Some(Busy::Presenting));
    assert_eq!(classify(QUIET_TIME, false), Some(Busy::DoNotDisturb));
}

/// The verdict depends on the machine; this only asserts the shell call and the
/// ConsentStore walk don't fault.
#[test]
fn live_probe_runs_clean() {
    let _ = state();
}

#[test]
fn a_call_wins_over_every_other_reason() {
    // A windowed Teams/Zoom call leaves the shell state at "accepting".
    assert_eq!(classify(ACCEPTS, true), Some(Busy::Call));
    assert_eq!(classify(NOT_PRESENT, true), Some(Busy::Call));
    assert_eq!(classify(D3D_FULL_SCREEN, true), Some(Busy::Call));
}
