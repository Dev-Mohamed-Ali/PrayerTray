//! Action Center toasts, port of Services/ToastService.cs. Unpackaged apps need a
//! Start-Menu shortcut tagged with the process AUMID before Windows shows a toast.
//! Callers fall back to a tray balloon when show() returns false.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{w, Interface, HSTRING, PCWSTR, PWSTR};
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER,
};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink};

/// Must match the C# build so upgrading users keep one Start-Menu entry.
pub const AUMID: PCWSTR = w!("DynamicEG.PrayerTray");

static READY: AtomicBool = AtomicBool::new(false);

pub fn init() -> bool {
    let ok = unsafe { SetCurrentProcessExplicitAppUserModelID(AUMID) }.is_ok() && ensure_shortcut().is_ok();
    READY.store(ok, Ordering::Relaxed);
    ok
}

pub fn show(title: &str, body: &str) -> bool {
    if !READY.load(Ordering::Relaxed) {
        return false;
    }
    let xml = format!(
        "<toast><visual><binding template='ToastGeneric'><text>{}</text><text>{}</text></binding></visual><audio silent='true'/></toast>",
        esc(title),
        esc(body)
    );
    (|| -> windows::core::Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let notifier =
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from_wide(unsafe { AUMID.as_wide() }))?;
        notifier.Show(&ToastNotification::CreateToastNotification(&doc)?)?;
        Ok(())
    })()
    .is_ok()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn shortcut_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    base.join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Prayer Tray.lnk")
}

// PKEY_AppUserModel_ID = {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, 5
const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
    pid: 5,
};

fn ensure_shortcut() -> windows::core::Result<()> {
    let exe = std::env::current_exe().map_err(|_| windows::core::Error::empty())?;
    let lnk = shortcut_path();
    if lnk.exists() && shortcut_target().map(|t| t.eq_ignore_ascii_case(&exe.to_string_lossy())).unwrap_or(false) {
        return Ok(());
    }

    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let exe_w: Vec<u16> = exe.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        link.SetPath(PCWSTR(exe_w.as_ptr()))?;
        link.SetArguments(w!(""))?;
        if let Some(dir) = exe.parent() {
            let dir_w: Vec<u16> = dir.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
            link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))?;
        }

        let store: IPropertyStore = link.cast()?;
        let pv = PROPVARIANT::from(AUMID.to_string()?.as_str());
        store.SetValue(&PKEY_APPUSERMODEL_ID, &pv)?;
        store.Commit()?;

        if let Some(parent) = lnk.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file: IPersistFile = link.cast()?;
        let lnk_w: Vec<u16> = lnk.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        file.Save(PCWSTR(lnk_w.as_ptr()), true)?;
    }
    Ok(())
}

fn shortcut_target() -> Option<String> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        let lnk_w: Vec<u16> = shortcut_path().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        file.Load(PCWSTR(lnk_w.as_ptr()), windows::Win32::System::Com::STGM(0)).ok()?;
        let mut buf = [0u16; 260];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        Some(PWSTR(buf.as_mut_ptr()).to_string().ok()?)
    }
}
