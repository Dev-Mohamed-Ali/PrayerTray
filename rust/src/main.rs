#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use prayertray::app::App;
use prayertray::config::AppConfig;
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};

fn main() {
    // Single instance: mutex held for the process lifetime (self-update releases it explicitly).
    // A release build ALWAYS uses the shared name, so a shipped exe can never run twice — there is
    // no environment escape hatch. Only debug builds get a separate name, so `cargo run` can sit
    // beside an installed release during development.
    let mutex_name = if cfg!(debug_assertions) {
        w!("PrayerTray.SingleInstance.Dev")
    } else {
        w!("PrayerTray.SingleInstance")
    };
    let mutex = unsafe { CreateMutexW(None, true, mutex_name) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }
    if let Ok(h) = mutex {
        prayertray::app::set_instance_mutex(h.0 as isize);
    }

    cleanup_old_update();

    std::panic::set_hook(Box::new(|info| {
        let dir = AppConfig::dir();
        let _ = std::fs::create_dir_all(&dir);
        let msg = format!("{info}\n\n");
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("error.log"))
            .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
        unsafe {
            MessageBoxW(
                None,
                w!("Prayer Tray hit an unexpected error. Details were written to error.log."),
                w!("Prayer Tray"),
                MB_OK | MB_ICONWARNING,
            );
        }
    }));

    App::run();
}

/// The previous exe left behind by a self-update; may still be locked — next launch retries.
fn cleanup_old_update() {
    if let Ok(exe) = std::env::current_exe() {
        let old = exe.with_extension("exe.old");
        if old.exists() {
            let _ = std::fs::remove_file(old);
        }
    }
}
