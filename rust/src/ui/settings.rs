//! Themed settings dialog, port of UI/SettingsForm.cs: side-nav sections, live preview,
//! Save persists / Cancel reverts to the opening snapshot / language change closes with Retry.

use crate::calc::praytimes::METHODS;
use crate::config::AppConfig;
use crate::i18n;
use crate::native::displays::{self, Monitor};
use crate::services::{audio, location, location::DetectedLocation};
use crate::ui::controls::{self, ButtonKind};
use crate::ui::theme;
use crate::ui::window::{self, WindowHandler};
use std::path::PathBuf;
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, DeleteObject, DrawTextW, EndPaint, InvalidateRect, RoundRect,
    SelectObject, SetBkMode, SetTextColor, DT_SINGLELINE, HFONT, PAINTSTRUCT, PS_SOLID,
    TRANSPARENT,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, DispatchMessageW, GetDlgItem, GetMessageW, IsDialogMessageW, MessageBoxW,
    MoveWindow, PostMessageW, PostQuitMessage, SetForegroundWindow, ShowWindow, TranslateMessage,
    AdjustWindowRectEx, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_RIGHT,
    MB_RTLREADING, MESSAGEBOX_STYLE, MSG, SW_HIDE, SW_SHOW, WM_APP, WM_CLOSE, WM_COMMAND,
    WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DRAWITEM,
    WM_ERASEBKGND, WM_PAINT, WINDOW_EX_STYLE, WS_CAPTION, WS_EX_DLGMODALFRAME, WS_EX_LAYOUTRTL,
    WS_EX_TOPMOST, WS_SYSMENU,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsResult {
    Ok,
    Cancel,
    Retry,
}

/// What the dialog needs from the app; App implements it.
pub trait SettingsHost {
    fn cfg(&mut self) -> &mut AppConfig;
    /// Re-apply theme/widget config from the (mutated-in-place) cfg, without raising windows.
    fn live_preview(&mut self);
    fn test_notify(&mut self, use_toast: bool);
}

const HIGHLAT_IDS: [&str; 4] = ["AngleBased", "MidNight", "OneSeventh", "None"];
const SIZE_STEPS: [i32; 6] = [80, 90, 100, 110, 125, 150];
const LANG_IDS: [&str; 7] = ["auto", "en", "ar", "fr", "tr", "ur", "id"];
const LANG_NAMES: [&str; 6] = ["English", "العربية", "Français", "Türkçe", "اردو", "Bahasa Indonesia"];
const ADJ_PRAYERS: [&str; 5] = ["fajr", "dhuhr", "asr", "maghrib", "isha"];
const PAGES: usize = 6;

const WM_DETECT_RESULT: u32 = WM_APP + 40;
const WM_PARSE_RESULT: u32 = WM_APP + 41;

// Control ids. 1/2 double as IDOK/IDCANCEL so IsDialogMessageW's ESC handling works.
const ID_SAVE: i32 = 1;
const ID_CANCEL: i32 = 2;
const ID_EXPORT: i32 = 10;
const ID_IMPORT: i32 = 11;
const ID_NAV_BASE: i32 = 100;
const ID_CITY: i32 = 201;
const ID_DETECT: i32 = 202;
const ID_LAT: i32 = 203;
const ID_LNG: i32 = 204;
const ID_OPENMAP: i32 = 205;
const ID_PASTE: i32 = 206;
const ID_SETPASTE: i32 = 207;
const ID_METHOD: i32 = 210;
const ID_ASR: i32 = 211;
const ID_HIGHLAT: i32 = 212;
const ID_ADJ_BASE: i32 = 220; // +0..4 (fajr..isha)
const ID_LANGUAGE: i32 = 240;
const ID_THEME: i32 = 241;
const ID_FONT: i32 = 242;
const ID_FONTSIZE: i32 = 243;
const ID_POSITION: i32 = 244;
const ID_OFFSET: i32 = 245;
const ID_MONITOR: i32 = 246;
const ID_H24: i32 = 247;
const ID_HIDEFS: i32 = 248;
const ID_NETSPEED: i32 = 280;
const ID_PING: i32 = 281;
const ID_PINGHOST: i32 = 282;
const ID_SYSMETERS: i32 = 284;
const ID_ROTATE: i32 = 283;
const ID_COMPACT: i32 = 285;
const ID_TRACKUSAGE: i32 = 286;
const ID_SHOWUSAGE: i32 = 287;
const ID_SHOWHIJRI: i32 = 250;
const ID_HIJRIADJ: i32 = 251;
const ID_SHOWEVENTS: i32 = 252;
const ID_SUNNAH: i32 = 253;
const ID_FRIDAY: i32 = 254;
const ID_RICH: i32 = 260;
const ID_TESTTOAST: i32 = 261;
const ID_REMENABLE: i32 = 262;
const ID_REMMINS: i32 = 263;
const ID_REMSOUND: i32 = 264;
const ID_REMSOUNDCB: i32 = 265;
const ID_REMFILE: i32 = 266;
const ID_REMBROWSE: i32 = 267;
const ID_REMTEST: i32 = 268;
const ID_AZAN: i32 = 269;
const ID_AZANFILE: i32 = 270;
const ID_AZANBROWSE: i32 = 271;
const ID_AZANTEST: i32 = 272;
const ID_AZANSTOP: i32 = 273;
const ID_MUTEBUSY: i32 = 274;

const BN_CLICKED: u32 = 0;
const CBN_SELCHANGE: u32 = 1;
const EN_KILLFOCUS: u32 = 0x0200;

// Base layout metrics at 96 dpi (scaled by the window's dpi).
const M: i32 = 12;
const NAV_W: i32 = 136;
const NAV_H: i32 = 34;
const CARD_X: i32 = M + NAV_W + 12;
const CARD_W: i32 = 440;
const CARD_PAD: i32 = 16;
const TITLE_H: i32 = 34;
const CARD_Y: i32 = M;
const ROW_H: i32 = 28;
const CARD_H: i32 = CARD_PAD + TITLE_H + 10 * ROW_H + CARD_PAD; // 10 = tallest page (Appearance)
const LBL_X: i32 = CARD_X + CARD_PAD;
const LABEL_W: i32 = 130;
const CTRL_X: i32 = LBL_X + LABEL_W + 8;
const CTRL_H: i32 = 23;
const Y0: i32 = CARD_Y + CARD_PAD + TITLE_H;
const BTN_Y: i32 = CARD_Y + CARD_H + 8;
const BTN_H: i32 = 30;
const CLIENT_W: i32 = CARD_X + CARD_W + M;
const CLIENT_H: i32 = BTN_Y + BTN_H + M;

struct Dialog {
    host: *mut dyn SettingsHost,
    snapshot: AppConfig,
    hwnd: HWND,
    result: Option<SettingsResult>,
    ready: bool,
    page: usize,
    scale: f32,
    brushes: controls::Brushes,
    font: HFONT,
    font_title: HFONT,
    pages: Vec<Vec<HWND>>,
    dim: Vec<HWND>,          // permanently-dim labels (section subheadings)
    dim_dyn: Vec<HWND>,      // labels dimmed because their field is currently disabled
    edits: Vec<HWND>,        // every edit control (read-only ones dim via WM_CTLCOLORSTATIC)
    checks: Vec<HWND>,       // owner-drawn checkboxes (routed to draw_checkbox / toggle)
    labeled: Vec<(i32, HWND)>, // (disable-able field id, its label) so labels dim in lockstep
    titles: Vec<String>,
    monitors: Vec<Monitor>,
    fonts: Vec<String>,
    rem_ids: Vec<&'static str>,
    azan_ids: Vec<String>,
}

/// Modal settings dialog on the current thread (nested message loop).
pub fn run(host: &mut dyn SettingsHost, snapshot: &AppConfig, prefill: Option<&DetectedLocation>) -> SettingsResult {
    let mut azan_ids = vec!["None".to_string()];
    azan_ids.extend(audio::BUILTIN_ADHANS.iter().map(|(id, _)| id.to_string()));
    azan_ids.push("Custom".into());
    let mut rem_ids: Vec<&'static str> = audio::REMINDER_SOUNDS.iter().map(|(id, _)| *id).collect();
    rem_ids.push("custom");

    // Erase the borrow lifetime; the nested modal loop below keeps `host` alive and on-thread.
    let host_ptr: *mut (dyn SettingsHost + 'static) = unsafe { std::mem::transmute(host) };
    let mut dlg = Box::new(Dialog {
        host: host_ptr,
        snapshot: snapshot.clone(),
        hwnd: HWND::default(),
        result: None,
        ready: false,
        page: 0,
        scale: 1.0,
        brushes: controls::Brushes::new(),
        font: HFONT::default(),
        font_title: HFONT::default(),
        pages: (0..PAGES).map(|_| Vec::new()).collect(),
        dim: Vec::new(),
        dim_dyn: Vec::new(),
        edits: Vec::new(),
        checks: Vec::new(),
        labeled: Vec::new(),
        titles: ["card.location", "card.calculation", "card.appearance", "card.network", "card.religious", "card.notifications"]
            .iter()
            .map(|k| i18n::t(k).to_string())
            .collect(),
        monitors: {
            let mut v = displays::all();
            v.sort_by_key(|m| m.bounds.left);
            v
        },
        fonts: controls::font_families(),
        rem_ids,
        azan_ids,
    });

    let rtl = if i18n::is_rtl() { WS_EX_LAYOUTRTL } else { WINDOW_EX_STYLE::default() };
    let title = window::utf16z(i18n::t("settings.title"));
    let hwnd = window::create(
        w!("PrayerTraySettings"),
        PCWSTR(title.as_ptr()),
        WS_EX_DLGMODALFRAME | WS_EX_TOPMOST | rtl,
        WS_CAPTION | WS_SYSMENU,
        i32::MIN,
        i32::MIN,
        200,
        200,
        None,
        dlg.as_mut() as *mut Dialog,
    )
    .expect("settings window");
    dlg.hwnd = hwnd;

    if theme::current().is_dark {
        let dark: i32 = 1;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const _ as *const core::ffi::c_void,
                4,
            );
        }
    }

    let dpi = unsafe { GetDpiForWindow(hwnd) };
    dlg.scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
    dlg.font = controls::ui_font(&theme::family(), 9.0, false, dpi.max(96));
    dlg.font_title = controls::ui_font(&theme::family(), 12.0, true, dpi.max(96));
    dlg.place_window();
    dlg.build();
    dlg.load_values();
    if let Some(p) = prefill {
        dlg.apply_detected(p);
    }
    dlg.sync_enabled();
    dlg.select_section(0);
    dlg.ready = true;
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }

    let mut msg = MSG::default();
    unsafe {
        while dlg.result.is_none() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32); // re-post so the outer loop exits too
                break;
            }
            if IsDialogMessageW(dlg.hwnd, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    dlg.result.take().unwrap_or(SettingsResult::Cancel)
}

impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }
            if !self.font_title.is_invalid() {
                let _ = DeleteObject(self.font_title.into());
            }
        }
    }
}

impl Dialog {
    fn host(&mut self) -> &mut dyn SettingsHost {
        unsafe { &mut *self.host }
    }

    fn s(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    fn item(&self, id: i32) -> HWND {
        unsafe { GetDlgItem(Some(self.hwnd), id).unwrap_or_default() }
    }

    fn place_window(&self) {
        let style = WS_CAPTION | WS_SYSMENU;
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: self.s(CLIENT_W),
            bottom: self.s(CLIENT_H),
        };
        unsafe {
            let _ = AdjustWindowRectEx(&mut rc, style, false, WS_EX_DLGMODALFRAME);
        }
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let work = displays::primary().map(|m| m.work).unwrap_or(RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        });
        let x = work.left + ((work.right - work.left) - w) / 2;
        let y = work.top + ((work.bottom - work.top) - h) / 2;
        unsafe {
            let _ = MoveWindow(self.hwnd, x, y.max(work.top), w, h, false);
        }
    }

    // --- construction ---

    fn lbl(&mut self, page: usize, text: &str, x: i32, y: i32, w: i32, dim: bool) -> HWND {
        let h = controls::label(self.hwnd, text, self.s(x), self.s(y + 4), self.s(w), self.s(18));
        controls::set_font(h, self.font);
        if dim {
            self.dim.push(h);
        }
        self.pages[page].push(h);
        h
    }

    fn edit_at(&mut self, page: usize, id: i32, x: i32, y: i32, w: i32) -> HWND {
        let h = controls::edit(self.hwnd, "", self.s(x), self.s(y), self.s(w), self.s(CTRL_H), id);
        controls::set_font(h, self.font);
        self.pages[page].push(h);
        self.edits.push(h);
        h
    }

    fn combo_at(&mut self, page: usize, id: i32, x: i32, y: i32, w: i32) -> HWND {
        let h = controls::combo(self.hwnd, self.s(x), self.s(y), self.s(w), self.s(260), id);
        controls::set_font(h, self.font);
        self.pages[page].push(h);
        h
    }

    fn check_at(&mut self, page: usize, id: i32, text: &str, x: i32, y: i32, w: i32) -> HWND {
        let h = controls::checkbox(self.hwnd, text, self.s(x), self.s(y + 2), self.s(w), self.s(20), id);
        controls::set_font(h, self.font);
        self.pages[page].push(h);
        self.checks.push(h);
        h
    }

    /// A field label paired to its control id, so the label dims when the field is disabled.
    fn lbl_for(&mut self, page: usize, id: i32, text: &str, x: i32, y: i32, w: i32) {
        let h = self.lbl(page, text, x, y, w, false);
        self.labeled.push((id, h));
    }

    fn btn_at(&mut self, page: Option<usize>, id: i32, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
        let b = controls::button(self.hwnd, text, self.s(x), self.s(y), self.s(w), self.s(h), id);
        controls::set_font(b, self.font);
        if let Some(p) = page {
            self.pages[p].push(b);
        }
        b
    }

    fn build(&mut self) {
        // Side nav.
        for i in 0..PAGES {
            let title = self.titles[i].clone();
            self.btn_at(None, ID_NAV_BASE + i as i32, &title, M, CARD_Y + i as i32 * (NAV_H + 6), NAV_W, NAV_H);
        }

        // --- Location ---
        let mut y = Y0;
        self.lbl(0, i18n::t("label.city"), LBL_X, y, LABEL_W, false);
        self.edit_at(0, ID_CITY, CTRL_X, y, 200);
        self.btn_at(Some(0), ID_DETECT, i18n::t("btn.detect"), CTRL_X + 204, y, 64, CTRL_H);
        y += ROW_H;
        self.lbl(0, i18n::t("label.lat"), LBL_X, y, LABEL_W, false);
        self.edit_at(0, ID_LAT, CTRL_X, y, 200);
        y += ROW_H;
        self.lbl(0, i18n::t("label.lng"), LBL_X, y, LABEL_W, false);
        self.edit_at(0, ID_LNG, CTRL_X, y, 200);
        y += ROW_H;
        self.lbl(0, i18n::t("label.pickMap"), LBL_X, y, LABEL_W, false);
        self.btn_at(Some(0), ID_OPENMAP, i18n::t("btn.openMaps"), CTRL_X, y, 130, CTRL_H);
        y += ROW_H;
        self.lbl(0, i18n::t("label.pasteResult"), LBL_X, y, LABEL_W, false);
        let paste = self.edit_at(0, ID_PASTE, CTRL_X, y, 200);
        controls::cue_banner(paste, i18n::t("ph.paste"));
        self.btn_at(Some(0), ID_SETPASTE, i18n::t("btn.set"), CTRL_X + 204, y, 64, CTRL_H);

        // --- Calculation ---
        let mut y = Y0;
        self.lbl(1, i18n::t("label.method"), LBL_X, y, LABEL_W, false);
        let method = self.combo_at(1, ID_METHOD, CTRL_X, y, 268);
        for m in METHODS.iter() {
            controls::combo_add(method, &format!("{} — {}", m.key, m.name));
        }
        y += ROW_H;
        self.lbl(1, i18n::t("label.asr"), LBL_X, y, LABEL_W, false);
        let asr = self.combo_at(1, ID_ASR, CTRL_X, y, 200);
        controls::combo_add(asr, i18n::t("asr.standard"));
        controls::combo_add(asr, i18n::t("asr.hanafi"));
        y += ROW_H;
        self.lbl(1, i18n::t("label.highLat"), LBL_X, y, LABEL_W, false);
        let hl = self.combo_at(1, ID_HIGHLAT, CTRL_X, y, 200);
        for id in HIGHLAT_IDS {
            controls::combo_add(hl, i18n::t(&format!("highLat.{id}")));
        }
        y += ROW_H;
        self.lbl(1, i18n::t("label.tuneTimes"), LBL_X, y, 380, true);
        y += ROW_H;
        for (i, key) in ADJ_PRAYERS.iter().enumerate() {
            self.lbl(1, i18n::prayer(key), LBL_X, y, LABEL_W, false);
            self.edit_at(1, ID_ADJ_BASE + i as i32, CTRL_X, y, 60);
            y += ROW_H;
        }

        // --- Appearance ---
        let mut y = Y0;
        self.lbl(2, i18n::t("label.language"), LBL_X, y, LABEL_W, false);
        let lang = self.combo_at(2, ID_LANGUAGE, CTRL_X, y, 200);
        controls::combo_add(lang, i18n::t("lang.auto"));
        for n in LANG_NAMES {
            controls::combo_add(lang, n);
        }
        y += ROW_H;
        self.lbl(2, i18n::t("label.theme"), LBL_X, y, LABEL_W, false);
        let th = self.combo_at(2, ID_THEME, CTRL_X, y, 200);
        for n in theme::NAMES {
            controls::combo_add(th, n);
        }
        y += ROW_H;
        self.lbl(2, i18n::t("label.font"), LBL_X, y, LABEL_W, false);
        let fc = self.combo_at(2, ID_FONT, CTRL_X, y, 268);
        for f in &self.fonts {
            controls::combo_add(fc, f);
        }
        y += ROW_H;
        self.lbl(2, i18n::t("label.fontSize"), LBL_X, y, LABEL_W, false);
        let fs = self.combo_at(2, ID_FONTSIZE, CTRL_X, y, 200);
        for n in SIZE_STEPS {
            controls::combo_add(fs, &format!("{n}%"));
        }
        y += ROW_H;
        self.lbl(2, i18n::t("label.widgetSide"), LBL_X, y, LABEL_W, false);
        let pos = self.combo_at(2, ID_POSITION, CTRL_X, y, 200);
        controls::combo_add(pos, i18n::t("side.right"));
        controls::combo_add(pos, i18n::t("side.left"));
        y += ROW_H;
        self.lbl(2, i18n::t("label.widgetGap"), LBL_X, y, LABEL_W, false);
        self.edit_at(2, ID_OFFSET, CTRL_X, y, 200);
        y += ROW_H;
        self.lbl(2, i18n::t("label.monitor"), LBL_X, y, LABEL_W, false);
        let mon = self.combo_at(2, ID_MONITOR, CTRL_X, y, 268);
        let names = displays::friendly_names();
        for m in &self.monitors {
            let name = displays::friendly_label(m, &names);
            let primary = if m.primary { i18n::t("monitor.primary") } else { "" };
            controls::combo_add(
                mon,
                &format!("{} ({}x{}){}", name, m.bounds.right - m.bounds.left, m.bounds.bottom - m.bounds.top, primary),
            );
        }
        y += ROW_H;
        self.check_at(2, ID_H24, i18n::t("chk.use24"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(2, ID_HIDEFS, i18n::t("chk.hideFs"), LBL_X, y, 380);

        // --- Network ---
        let mut y = Y0;
        self.check_at(3, ID_NETSPEED, i18n::t("chk.netSpeed"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(3, ID_PING, i18n::t("chk.ping"), LBL_X, y, 380);
        y += ROW_H;
        self.lbl_for(3, ID_PINGHOST, i18n::t("label.pingHost"), LBL_X, y, LABEL_W);
        self.edit_at(3, ID_PINGHOST, CTRL_X, y, 200);
        y += ROW_H;
        self.check_at(3, ID_SYSMETERS, i18n::t("chk.sysMeters"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(3, ID_COMPACT, i18n::t("chk.compactMeters"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(3, ID_ROTATE, i18n::t("chk.rotateMeters"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(3, ID_TRACKUSAGE, i18n::t("chk.trackUsage"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(3, ID_SHOWUSAGE, i18n::t("chk.showUsage"), LBL_X, y, 380);

        // --- Religious ---
        let mut y = Y0;
        self.check_at(4, ID_SHOWHIJRI, i18n::t("chk.showHijri"), LBL_X, y, 380);
        y += ROW_H;
        self.lbl_for(4, ID_HIJRIADJ, i18n::t("label.hijriAdjust"), LBL_X, y, LABEL_W);
        self.edit_at(4, ID_HIJRIADJ, CTRL_X, y, 60);
        y += ROW_H;
        self.check_at(4, ID_SHOWEVENTS, i18n::t("chk.showEvents"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(4, ID_SUNNAH, i18n::t("chk.sunnahFast"), LBL_X, y, 380);
        y += ROW_H;
        self.check_at(4, ID_FRIDAY, i18n::t("chk.fridayReminder"), LBL_X, y, 380);

        // --- Notifications ---
        let mut y = Y0;
        self.check_at(5, ID_RICH, i18n::t("chk.richToasts"), LBL_X, y, 240);
        self.btn_at(Some(5), ID_TESTTOAST, i18n::t("btn.test"), LBL_X + 248, y, 60, CTRL_H);
        y += ROW_H;
        self.check_at(5, ID_REMENABLE, i18n::t("chk.remind"), LBL_X, y, 380);
        y += ROW_H;
        self.lbl_for(5, ID_REMMINS, i18n::t("label.minutesBefore"), LBL_X, y, LABEL_W);
        self.edit_at(5, ID_REMMINS, CTRL_X, y, 60);
        y += ROW_H;
        self.check_at(5, ID_REMSOUND, i18n::t("chk.playSound"), LBL_X, y, 380);
        y += ROW_H;
        self.lbl_for(5, ID_REMSOUNDCB, i18n::t("label.sound"), LBL_X, y, LABEL_W);
        let rs = self.combo_at(5, ID_REMSOUNDCB, CTRL_X, y, 200);
        for (id, _) in audio::REMINDER_SOUNDS {
            controls::combo_add(rs, i18n::t(&format!("sound.{id}")));
        }
        controls::combo_add(rs, i18n::t("combo.customFile"));
        y += ROW_H;
        self.lbl_for(5, ID_REMFILE, i18n::t("label.customFile"), LBL_X, y, LABEL_W);
        let rf = self.edit_at(5, ID_REMFILE, CTRL_X, y, 160);
        controls::cue_banner(rf, i18n::t("ph.customFile"));
        self.btn_at(Some(5), ID_REMBROWSE, "…", CTRL_X + 164, y, 28, CTRL_H);
        self.btn_at(Some(5), ID_REMTEST, i18n::t("btn.test"), CTRL_X + 196, y, 50, CTRL_H);
        y += ROW_H;
        self.lbl(5, i18n::t("label.azan"), LBL_X, y, LABEL_W, false);
        let az = self.combo_at(5, ID_AZAN, CTRL_X, y, 200);
        controls::combo_add(az, i18n::t("azan.off"));
        for (id, _) in audio::BUILTIN_ADHANS {
            controls::combo_add(az, i18n::t(&format!("adhan.{id}")));
        }
        controls::combo_add(az, i18n::t("combo.customFile"));
        y += ROW_H;
        self.lbl_for(5, ID_AZANFILE, i18n::t("label.azanFile"), LBL_X, y, LABEL_W);
        let af = self.edit_at(5, ID_AZANFILE, CTRL_X, y, 160);
        controls::cue_banner(af, ".mp3 / .wav");
        self.btn_at(Some(5), ID_AZANBROWSE, "…", CTRL_X + 164, y, 28, CTRL_H);
        y += ROW_H;
        self.btn_at(Some(5), ID_AZANTEST, i18n::t("btn.test"), CTRL_X, y, 60, CTRL_H);
        self.btn_at(Some(5), ID_AZANSTOP, i18n::t("btn.stop"), CTRL_X + 64, y, 60, CTRL_H);
        y += ROW_H;
        self.check_at(5, ID_MUTEBUSY, i18n::t("chk.muteBusy"), LBL_X, y, 400);

        // --- bottom buttons ---
        let save_x = CLIENT_W - M - 90;
        self.btn_at(None, ID_SAVE, i18n::t("btn.save"), save_x, BTN_Y, 90, BTN_H);
        self.btn_at(None, ID_CANCEL, i18n::t("btn.cancel"), save_x - 96, BTN_Y, 90, BTN_H);
        self.btn_at(None, ID_EXPORT, i18n::t("btn.exportCfg"), save_x - 96 - 108, BTN_Y, 90, BTN_H);
        self.btn_at(None, ID_IMPORT, i18n::t("btn.importCfg"), save_x - 96 - 108 - 96, BTN_Y, 90, BTN_H);
    }

    // --- values <-> controls ---

    fn set_num(&self, id: i32, v: i32) {
        controls::set_text(self.item(id), &v.to_string());
    }

    fn get_num(&self, id: i32, lo: i32, hi: i32) -> Option<i32> {
        controls::get_text(self.item(id)).trim().parse::<i32>().ok().map(|v| v.clamp(lo, hi))
    }

    fn load_values(&mut self) {
        let cfg = self.host().cfg().clone();
        controls::set_text(self.item(ID_CITY), &cfg.city);
        controls::set_text(self.item(ID_LAT), &cfg.latitude.to_string());
        controls::set_text(self.item(ID_LNG), &cfg.longitude.to_string());
        controls::combo_set(self.item(ID_METHOD), index_of_method(&cfg.method));
        controls::combo_set(self.item(ID_ASR), if cfg.asr == 2 { 1 } else { 0 });
        let hli = HIGHLAT_IDS.iter().position(|s| *s == cfg.high_lats).unwrap_or(0);
        controls::combo_set(self.item(ID_HIGHLAT), hli as i32);
        let adj = [cfg.fajr_adjust, cfg.dhuhr_adjust, cfg.asr_adjust, cfg.maghrib_adjust, cfg.isha_adjust];
        for i in 0..5 {
            self.set_num(ID_ADJ_BASE + i, adj[i as usize].clamp(-60, 60));
        }
        let li = LANG_IDS.iter().position(|s| *s == cfg.language).unwrap_or(0);
        controls::combo_set(self.item(ID_LANGUAGE), li as i32);
        let ti = theme::NAMES.iter().position(|s| *s == cfg.theme).unwrap_or(0);
        controls::combo_set(self.item(ID_THEME), ti as i32);
        let fi = self.fonts.iter().position(|f| f.eq_ignore_ascii_case(&cfg.font_family)).map(|i| i as i32).unwrap_or(-1);
        controls::combo_set(self.item(ID_FONT), fi);
        controls::combo_set(self.item(ID_FONTSIZE), nearest_size(cfg.font_scale_pct) as i32);
        controls::combo_set(self.item(ID_POSITION), if cfg.widget_anchor.eq_ignore_ascii_case("Left") { 1 } else { 0 });
        controls::set_text(self.item(ID_OFFSET), &cfg.widget_offset.to_string());
        let mi = self
            .monitors
            .iter()
            .position(|m| match &cfg.monitor_device_name {
                None => m.primary,
                Some(d) => m.device.eq_ignore_ascii_case(d),
            })
            .or_else(|| self.monitors.iter().position(|m| m.primary))
            .unwrap_or(0);
        controls::combo_set(self.item(ID_MONITOR), mi as i32);
        controls::set_checked(self.item(ID_H24), cfg.use24_hour);
        controls::set_checked(self.item(ID_HIDEFS), cfg.hide_on_fullscreen);
        controls::set_checked(self.item(ID_NETSPEED), cfg.show_net_speed);
        controls::set_checked(self.item(ID_PING), cfg.show_ping);
        controls::set_text(self.item(ID_PINGHOST), &cfg.ping_host);
        controls::set_checked(self.item(ID_SYSMETERS), cfg.show_sys_meters);
        controls::set_checked(self.item(ID_COMPACT), cfg.compact_meters);
        controls::set_checked(self.item(ID_ROTATE), cfg.rotate_meters);
        controls::set_checked(self.item(ID_TRACKUSAGE), cfg.track_data_usage);
        controls::set_checked(self.item(ID_SHOWUSAGE), cfg.show_data_usage);
        controls::set_checked(self.item(ID_SHOWHIJRI), cfg.show_hijri_date);
        self.set_num(ID_HIJRIADJ, cfg.hijri_adjust.clamp(-2, 2));
        controls::set_checked(self.item(ID_SHOWEVENTS), cfg.show_islamic_events);
        controls::set_checked(self.item(ID_SUNNAH), cfg.sunnah_fast_reminder);
        controls::set_checked(self.item(ID_FRIDAY), cfg.friday_reminder);
        controls::set_checked(self.item(ID_RICH), cfg.rich_toasts);
        controls::set_checked(self.item(ID_REMENABLE), cfg.reminder_enabled);
        self.set_num(ID_REMMINS, cfg.reminder_minutes.clamp(1, 60));
        controls::set_checked(self.item(ID_REMSOUND), cfg.reminder_sound);
        let si = self.rem_ids.iter().position(|s| *s == cfg.reminder_sound_id).unwrap_or(0);
        controls::combo_set(self.item(ID_REMSOUNDCB), si as i32);
        controls::set_text(self.item(ID_REMFILE), cfg.reminder_sound_path.as_deref().unwrap_or(""));
        let ai = self
            .azan_ids
            .iter()
            .position(|s| *s == cfg.azan_mode)
            .or_else(|| (cfg.azan_mode == "Builtin" && self.azan_ids.len() > 2).then_some(1))
            .unwrap_or(0);
        controls::combo_set(self.item(ID_AZAN), ai as i32);
        controls::set_text(self.item(ID_AZANFILE), cfg.azan_custom_path.as_deref().unwrap_or(""));
        controls::set_checked(self.item(ID_MUTEBUSY), cfg.mute_when_busy);
    }

    fn try_lat(&self) -> Option<f64> {
        let v: f64 = controls::get_text(self.item(ID_LAT)).trim().parse().ok()?;
        (-90.0..=90.0).contains(&v).then_some(v)
    }

    fn try_lng(&self) -> Option<f64> {
        let v: f64 = controls::get_text(self.item(ID_LNG)).trim().parse().ok()?;
        (-180.0..=180.0).contains(&v).then_some(v)
    }

    /// Validate the form and write every control into `c`; false (after a warn) if invalid.
    fn collect(&mut self, c: &mut AppConfig) -> bool {
        let Some(lat) = self.try_lat() else {
            self.warn(i18n::t("msg.latRange"));
            return false;
        };
        let Some(lng) = self.try_lng() else {
            self.warn(i18n::t("msg.lngRange"));
            return false;
        };
        let azan_mode = self.azan_ids[controls::combo_sel(self.item(ID_AZAN)).max(0) as usize].clone();
        let azan_file = controls::get_text(self.item(ID_AZANFILE));
        if azan_mode == "Custom" && azan_file.trim().is_empty() {
            self.warn(i18n::t("msg.azanFile"));
            return false;
        }

        let city = controls::get_text(self.item(ID_CITY));
        c.city = if city.trim().is_empty() { "Custom".into() } else { city.trim().to_string() };
        c.latitude = lat;
        c.longitude = lng;
        c.method = METHODS[controls::combo_sel(self.item(ID_METHOD)).clamp(0, METHODS.len() as i32 - 1) as usize]
            .key
            .to_string();
        c.asr = if controls::combo_sel(self.item(ID_ASR)) == 1 { 2 } else { 1 };
        c.high_lats = HIGHLAT_IDS[controls::combo_sel(self.item(ID_HIGHLAT)).clamp(0, 3) as usize].to_string();
        let adj: Vec<i32> = (0..5).map(|i| self.get_num(ID_ADJ_BASE + i, -60, 60).unwrap_or(0)).collect();
        c.fajr_adjust = adj[0];
        c.dhuhr_adjust = adj[1];
        c.asr_adjust = adj[2];
        c.maghrib_adjust = adj[3];
        c.isha_adjust = adj[4];
        c.widget_anchor = if controls::combo_sel(self.item(ID_POSITION)) == 1 { "Left" } else { "Right" }.into();
        c.theme = theme::NAMES[controls::combo_sel(self.item(ID_THEME)).clamp(0, 5) as usize].to_string();
        if let Some(f) = usize::try_from(controls::combo_sel(self.item(ID_FONT))).ok().and_then(|i| self.fonts.get(i)) {
            c.font_family = f.clone();
        }
        c.font_scale_pct = SIZE_STEPS[controls::combo_sel(self.item(ID_FONTSIZE)).clamp(0, 5) as usize];
        if let Some(m) = self.monitors.get(controls::combo_sel(self.item(ID_MONITOR)).max(0) as usize) {
            c.monitor_device_name = if m.primary { None } else { Some(m.device.clone()) };
        }
        if let Ok(off) = controls::get_text(self.item(ID_OFFSET)).trim().parse::<i32>() {
            c.widget_offset = off.clamp(0, 2000);
        }
        c.use24_hour = controls::checked(self.item(ID_H24));
        c.hide_on_fullscreen = controls::checked(self.item(ID_HIDEFS));
        c.show_net_speed = controls::checked(self.item(ID_NETSPEED));
        c.show_ping = controls::checked(self.item(ID_PING));
        let ping_host = controls::get_text(self.item(ID_PINGHOST));
        c.ping_host = if ping_host.trim().is_empty() { "1.1.1.1".into() } else { ping_host.trim().to_string() };
        c.show_sys_meters = controls::checked(self.item(ID_SYSMETERS));
        c.compact_meters = controls::checked(self.item(ID_COMPACT));
        c.rotate_meters = controls::checked(self.item(ID_ROTATE));
        c.track_data_usage = controls::checked(self.item(ID_TRACKUSAGE));
        c.show_data_usage = controls::checked(self.item(ID_SHOWUSAGE));
        c.show_hijri_date = controls::checked(self.item(ID_SHOWHIJRI));
        c.hijri_adjust = self.get_num(ID_HIJRIADJ, -2, 2).unwrap_or(0);
        c.show_islamic_events = controls::checked(self.item(ID_SHOWEVENTS));
        c.sunnah_fast_reminder = controls::checked(self.item(ID_SUNNAH));
        c.friday_reminder = controls::checked(self.item(ID_FRIDAY));
        c.rich_toasts = controls::checked(self.item(ID_RICH));
        c.reminder_enabled = controls::checked(self.item(ID_REMENABLE));
        c.reminder_minutes = self.get_num(ID_REMMINS, 1, 60).unwrap_or(10);
        c.reminder_sound = controls::checked(self.item(ID_REMSOUND));
        c.reminder_sound_id = self.rem_ids[controls::combo_sel(self.item(ID_REMSOUNDCB)).max(0) as usize].to_string();
        let rem_file = controls::get_text(self.item(ID_REMFILE));
        c.reminder_sound_path = (!rem_file.trim().is_empty()).then(|| rem_file.trim().to_string());
        c.azan_mode = azan_mode;
        c.azan_custom_path = (!azan_file.trim().is_empty()).then(|| azan_file.trim().to_string());
        c.mute_when_busy = controls::checked(self.item(ID_MUTEBUSY));
        true
    }

    // --- behavior ---

    fn live(&mut self, f: impl FnOnce(&mut AppConfig)) {
        if !self.ready {
            return;
        }
        f(self.host().cfg());
        self.host().live_preview();
    }

    fn sync_enabled(&mut self) {
        // Edits soft-disable via read-only (keeps our WM_CTLCOLORSTATIC color); everything else via
        // EnableWindow. Labels dim in lockstep through `dim_dyn` (see the states table below).
        let rem = controls::checked(self.item(ID_REMENABLE));
        controls::set_readonly(self.item(ID_REMMINS), !rem);
        controls::enable(self.item(ID_REMSOUND), rem);
        let snd = rem && controls::checked(self.item(ID_REMSOUND));
        controls::enable(self.item(ID_REMSOUNDCB), snd);
        let sel = controls::combo_sel(self.item(ID_REMSOUNDCB)).max(0) as usize;
        let custom = snd && self.rem_ids.get(sel).copied() == Some("custom");
        controls::set_readonly(self.item(ID_REMFILE), !custom);
        controls::enable(self.item(ID_REMBROWSE), custom);
        controls::enable(self.item(ID_REMTEST), snd);
        let asel = controls::combo_sel(self.item(ID_AZAN)).max(0) as usize;
        let amode = self.azan_ids.get(asel).map(String::as_str).unwrap_or("None");
        let azan_custom = amode == "Custom";
        controls::set_readonly(self.item(ID_AZANFILE), !azan_custom);
        controls::enable(self.item(ID_AZANBROWSE), azan_custom);
        controls::enable(self.item(ID_AZANTEST), amode != "None");
        controls::enable(self.item(ID_MUTEBUSY), snd || amode != "None");
        let show_hijri = controls::checked(self.item(ID_SHOWHIJRI));
        controls::set_readonly(self.item(ID_HIJRIADJ), !show_hijri);

        let netspeed = controls::checked(self.item(ID_NETSPEED));
        let ping = controls::checked(self.item(ID_PING));
        let track = controls::checked(self.item(ID_TRACKUSAGE));
        let show_usage = controls::checked(self.item(ID_SHOWUSAGE));
        controls::set_readonly(self.item(ID_PINGHOST), !ping);
        controls::enable(self.item(ID_SHOWUSAGE), track);
        let sysm = controls::checked(self.item(ID_SYSMETERS));
        let any_meter = netspeed || ping || sysm || (track && show_usage);
        controls::enable(self.item(ID_COMPACT), any_meter);
        controls::enable(self.item(ID_ROTATE), any_meter);

        let states = [
            (ID_PINGHOST, ping),
            (ID_HIJRIADJ, show_hijri),
            (ID_REMMINS, rem),
            (ID_REMSOUNDCB, snd),
            (ID_REMFILE, custom),
            (ID_AZANFILE, azan_custom),
        ];
        self.dim_dyn.clear();
        for (id, on) in states {
            if !on {
                if let Some(&(_, h)) = self.labeled.iter().find(|(fid, _)| *fid == id) {
                    self.dim_dyn.push(h);
                }
            }
        }
        for &(_, h) in &self.labeled {
            unsafe {
                let _ = InvalidateRect(Some(h), None, false);
            }
        }
    }

    fn select_section(&mut self, idx: usize) {
        self.page = idx.min(PAGES - 1);
        for (i, page) in self.pages.iter().enumerate() {
            for h in page {
                unsafe {
                    let _ = ShowWindow(*h, if i == self.page { SW_SHOW } else { SW_HIDE });
                }
            }
        }
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    fn current_reminder_path(&self) -> PathBuf {
        let sel = controls::combo_sel(self.item(ID_REMSOUNDCB)).max(0) as usize;
        let id = self.rem_ids.get(sel).copied().unwrap_or("chime");
        let file = controls::get_text(self.item(ID_REMFILE));
        if id == "custom" && !file.trim().is_empty() {
            PathBuf::from(file.trim())
        } else {
            audio::synth_path(id)
        }
    }

    fn current_azan_path(&self) -> Option<PathBuf> {
        let sel = controls::combo_sel(self.item(ID_AZAN)).max(0) as usize;
        match self.azan_ids.get(sel).map(String::as_str).unwrap_or("None") {
            "None" => None,
            "Custom" => {
                let f = controls::get_text(self.item(ID_AZANFILE));
                (!f.trim().is_empty()).then(|| PathBuf::from(f.trim()))
            }
            id => audio::builtin_adhan_path(id),
        }
    }

    fn apply_detected(&mut self, loc: &DetectedLocation) {
        controls::set_text(self.item(ID_LAT), &loc.lat.to_string());
        controls::set_text(self.item(ID_LNG), &loc.lng.to_string());
        match &loc.city {
            Some(city) if !city.trim().is_empty() => controls::set_text(self.item(ID_CITY), city),
            _ => {
                if controls::get_text(self.item(ID_CITY)).trim().is_empty() {
                    controls::set_text(self.item(ID_CITY), i18n::t("city.myLocation"));
                }
            }
        }
        let method = location::method_for_country(loc.country_iso.as_deref());
        controls::combo_set(self.item(ID_METHOD), index_of_method(method));
        let (lat, lng) = (self.try_lat(), self.try_lng());
        self.live(|c| {
            if let Some(v) = lat {
                c.latitude = v;
            }
            if let Some(v) = lng {
                c.longitude = v;
            }
        });
    }

    // Language is structural (strings + RTL fixed at construction): apply, then close with Retry
    // so the host reopens the dialog rebuilt in the new language.
    fn on_language_changed(&mut self) {
        if !self.ready {
            return;
        }
        let id = LANG_IDS[controls::combo_sel(self.item(ID_LANGUAGE)).clamp(0, 6) as usize];
        if id == self.host().cfg().language {
            return;
        }
        self.host().cfg().language = id.to_string();
        i18n::set(id);
        self.host().live_preview();
        self.finish(SettingsResult::Retry);
    }

    fn on_save(&mut self) {
        let mut tmp = self.host().cfg().clone();
        if !self.collect(&mut tmp) {
            return;
        }
        *self.host().cfg() = tmp;
        self.host().cfg().save();
        self.host().live_preview();
        self.finish(SettingsResult::Ok);
    }

    fn on_export(&mut self) {
        let mut tmp = self.host().cfg().clone();
        if !self.collect(&mut tmp) {
            return;
        }
        let Some(path) = self.save_file_dialog("JSON\0*.json\0\0", "PrayerTray-config.json") else {
            return;
        };
        let ok = serde_json::to_string_pretty(&tmp)
            .ok()
            .and_then(|json| std::fs::write(&path, json).ok())
            .is_some();
        if !ok {
            self.msg(i18n::t("msg.exportError"), i18n::t("msg.invalidCaption"), MB_ICONWARNING);
        }
    }

    fn on_import(&mut self) {
        let Some(path) = self.open_file_dialog("JSON\0*.json\0All files\0*.*\0\0") else {
            return;
        };
        let imported = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<AppConfig>(&t).ok());
        let Some(mut imported) = imported else {
            self.msg(i18n::t("msg.importError"), i18n::t("msg.invalidCaption"), MB_ICONWARNING);
            return;
        };
        imported.sanitize();
        let prev_lang = self.host().cfg().language.clone();
        *self.host().cfg() = imported;
        self.ready = false;
        self.load_values();
        self.ready = true;
        self.sync_enabled();
        if self.host().cfg().language != prev_lang {
            let lang = self.host().cfg().language.clone();
            i18n::set(&lang);
            self.host().live_preview();
            self.finish(SettingsResult::Retry);
            return;
        }
        self.host().live_preview();
    }

    fn finish(&mut self, r: SettingsResult) {
        if self.result.is_some() {
            return;
        }
        audio::stop();
        // A language restart (Retry) keeps the in-progress config; only a real Cancel reverts.
        if r == SettingsResult::Cancel {
            let snap = self.snapshot.clone();
            *self.host().cfg() = snap;
            let lang = self.snapshot.language.clone();
            i18n::set(&lang);
            self.host().live_preview();
        }
        self.result = Some(r);
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    fn on_detect(&mut self) {
        let btn = self.item(ID_DETECT);
        controls::enable(btn, false);
        controls::set_text(btn, "…");
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let loc = Box::new(location::detect());
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_DETECT_RESULT,
                    WPARAM(0),
                    LPARAM(Box::into_raw(loc) as isize),
                );
            }
        });
    }

    fn on_set_paste(&mut self) {
        let text = controls::get_text(self.item(ID_PASTE));
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let loc = Box::new(location::parse(&text)); // may follow short links (network)
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_PARSE_RESULT,
                    WPARAM(0),
                    LPARAM(Box::into_raw(loc) as isize),
                );
            }
        });
    }

    fn on_command(&mut self, id: i32, code: u32) {
        if code == BN_CLICKED {
            // Owner-drawn checkboxes don't auto-toggle; flip our stored state before the handlers
            // (which read controls::checked) run. Push buttons aren't in `checks`, so they're skipped.
            let h = self.item(id);
            if self.checks.contains(&h) {
                controls::toggle_check(h);
            }
            match id {
                ID_SAVE => self.on_save(),
                ID_CANCEL => self.finish(SettingsResult::Cancel),
                ID_EXPORT => self.on_export(),
                ID_IMPORT => self.on_import(),
                n if (ID_NAV_BASE..ID_NAV_BASE + PAGES as i32).contains(&n) => {
                    self.select_section((n - ID_NAV_BASE) as usize)
                }
                ID_DETECT => self.on_detect(),
                ID_OPENMAP => {
                    std::thread::spawn(|| {
                        let rough = location::ip_rough();
                        open_url(&location::maps_url(rough.map(|r| (r.lat, r.lng))));
                    });
                }
                ID_SETPASTE => self.on_set_paste(),
                ID_TESTTOAST => {
                    let use_toast = controls::checked(self.item(ID_RICH));
                    self.host().test_notify(use_toast);
                }
                ID_REMBROWSE => {
                    if let Some(p) = self.open_file_dialog("Audio\0*.wav;*.mp3\0All files\0*.*\0\0") {
                        controls::set_text(self.item(ID_REMFILE), &p.display().to_string());
                        self.sync_enabled();
                    }
                }
                ID_AZANBROWSE => {
                    if let Some(p) = self.open_file_dialog("Audio\0*.mp3;*.wav\0All files\0*.*\0\0") {
                        controls::set_text(self.item(ID_AZANFILE), &p.display().to_string());
                        self.sync_enabled();
                    }
                }
                ID_REMTEST => audio::play(&self.current_reminder_path()),
                ID_AZANTEST => {
                    if let Some(p) = self.current_azan_path() {
                        audio::play(&p);
                    }
                }
                ID_AZANSTOP => audio::stop(),
                ID_H24 => {
                    let v = controls::checked(self.item(ID_H24));
                    self.live(|c| c.use24_hour = v);
                }
                ID_HIDEFS => {
                    let v = controls::checked(self.item(ID_HIDEFS));
                    self.live(|c| c.hide_on_fullscreen = v);
                }
                ID_SHOWHIJRI => {
                    let v = controls::checked(self.item(ID_SHOWHIJRI));
                    self.live(|c| c.show_hijri_date = v);
                    self.sync_enabled();
                }
                ID_SHOWEVENTS => {
                    let v = controls::checked(self.item(ID_SHOWEVENTS));
                    self.live(|c| c.show_islamic_events = v);
                }
                ID_REMENABLE | ID_REMSOUND => self.sync_enabled(),
                ID_NETSPEED => {
                    let v = controls::checked(self.item(ID_NETSPEED));
                    self.live(|c| c.show_net_speed = v);
                    self.sync_enabled();
                }
                ID_PING => {
                    let v = controls::checked(self.item(ID_PING));
                    self.live(|c| c.show_ping = v);
                    self.sync_enabled();
                }
                ID_SYSMETERS => {
                    let v = controls::checked(self.item(ID_SYSMETERS));
                    self.live(|c| c.show_sys_meters = v);
                    self.sync_enabled();
                }
                ID_ROTATE => {
                    let v = controls::checked(self.item(ID_ROTATE));
                    self.live(|c| c.rotate_meters = v);
                }
                ID_COMPACT => {
                    let v = controls::checked(self.item(ID_COMPACT));
                    self.live(|c| c.compact_meters = v);
                }
                ID_TRACKUSAGE => {
                    let v = controls::checked(self.item(ID_TRACKUSAGE));
                    self.live(|c| c.track_data_usage = v);
                    self.sync_enabled();
                }
                ID_SHOWUSAGE => {
                    let v = controls::checked(self.item(ID_SHOWUSAGE));
                    self.live(|c| c.show_data_usage = v);
                    self.sync_enabled();
                }
                _ => {}
            }
        } else if code == CBN_SELCHANGE {
            match id {
                ID_LANGUAGE => self.on_language_changed(),
                ID_METHOD => {
                    let i = controls::combo_sel(self.item(ID_METHOD)).clamp(0, METHODS.len() as i32 - 1) as usize;
                    self.live(|c| c.method = METHODS[i].key.to_string());
                }
                ID_ASR => {
                    let hanafi = controls::combo_sel(self.item(ID_ASR)) == 1;
                    self.live(|c| c.asr = if hanafi { 2 } else { 1 });
                }
                ID_HIGHLAT => {
                    let i = controls::combo_sel(self.item(ID_HIGHLAT)).clamp(0, 3) as usize;
                    self.live(|c| c.high_lats = HIGHLAT_IDS[i].to_string());
                }
                ID_THEME => {
                    let i = controls::combo_sel(self.item(ID_THEME)).clamp(0, 5) as usize;
                    self.live(|c| c.theme = theme::NAMES[i].to_string());
                }
                ID_FONT => {
                    let f = usize::try_from(controls::combo_sel(self.item(ID_FONT)))
                        .ok()
                        .and_then(|i| self.fonts.get(i).cloned());
                    if let Some(f) = f {
                        self.live(|c| c.font_family = f);
                    }
                }
                ID_FONTSIZE => {
                    let i = controls::combo_sel(self.item(ID_FONTSIZE)).clamp(0, 5) as usize;
                    self.live(|c| c.font_scale_pct = SIZE_STEPS[i]);
                }
                ID_POSITION => {
                    let left = controls::combo_sel(self.item(ID_POSITION)) == 1;
                    self.live(|c| c.widget_anchor = if left { "Left" } else { "Right" }.into());
                }
                ID_REMSOUNDCB | ID_AZAN => self.sync_enabled(),
                _ => {} // monitor change applies on Save only
            }
        } else if code == EN_KILLFOCUS {
            match id {
                ID_LAT => {
                    if let Some(v) = self.try_lat() {
                        self.live(|c| c.latitude = v);
                    }
                }
                ID_LNG => {
                    if let Some(v) = self.try_lng() {
                        self.live(|c| c.longitude = v);
                    }
                }
                ID_OFFSET => {
                    if let Ok(v) = controls::get_text(self.item(ID_OFFSET)).trim().parse::<i32>() {
                        self.live(|c| c.widget_offset = v.clamp(0, 2000));
                    }
                }
                ID_PINGHOST => {
                    let h = controls::get_text(self.item(ID_PINGHOST));
                    let host = if h.trim().is_empty() { "1.1.1.1".to_string() } else { h.trim().to_string() };
                    self.live(|c| c.ping_host = host);
                }
                ID_HIJRIADJ => {
                    if let Some(v) = self.get_num(ID_HIJRIADJ, -2, 2) {
                        self.live(|c| c.hijri_adjust = v);
                    }
                }
                n if (ID_ADJ_BASE..ID_ADJ_BASE + 5).contains(&n) => {
                    if let Some(v) = self.get_num(n, -60, 60) {
                        let i = (n - ID_ADJ_BASE) as usize;
                        self.live(|c| set_adjust(c, i, v));
                    }
                }
                _ => {}
            }
        }
    }

    // --- dialogs ---

    fn warn(&self, body: &str) {
        self.msg(body, i18n::t("msg.invalidCaption"), MB_ICONWARNING);
    }

    fn msg(&self, body: &str, caption: &str, icon: MESSAGEBOX_STYLE) {
        let b = window::utf16z(body);
        let c = window::utf16z(caption);
        let style = MB_OK | icon | if i18n::is_rtl() { MB_RTLREADING | MB_RIGHT } else { Default::default() };
        unsafe {
            MessageBoxW(Some(self.hwnd), PCWSTR(b.as_ptr()), PCWSTR(c.as_ptr()), style);
        }
    }

    fn open_file_dialog(&self, filter: &str) -> Option<PathBuf> {
        let filter_w: Vec<u16> = filter.encode_utf16().collect();
        let mut buf = [0u16; 1024];
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: self.hwnd,
            lpstrFilter: PCWSTR(filter_w.as_ptr()),
            lpstrFile: PWSTR(buf.as_mut_ptr()),
            nMaxFile: buf.len() as u32,
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        unsafe { GetOpenFileNameW(&mut ofn).as_bool() }.then(|| {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            PathBuf::from(String::from_utf16_lossy(&buf[..len]))
        })
    }

    fn save_file_dialog(&self, filter: &str, default_name: &str) -> Option<PathBuf> {
        let filter_w: Vec<u16> = filter.encode_utf16().collect();
        let mut buf = [0u16; 1024];
        for (i, c) in default_name.encode_utf16().enumerate().take(buf.len() - 1) {
            buf[i] = c;
        }
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: self.hwnd,
            lpstrFilter: PCWSTR(filter_w.as_ptr()),
            lpstrFile: PWSTR(buf.as_mut_ptr()),
            nMaxFile: buf.len() as u32,
            lpstrDefExt: w!("json"),
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        unsafe { GetSaveFileNameW(&mut ofn).as_bool() }.then(|| {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            PathBuf::from(String::from_utf16_lossy(&buf[..len]))
        })
    }

    // --- painting ---

    fn paint(&mut self) {
        let p = theme::current();
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(self.hwnd, &mut ps);
            let pen = CreatePen(PS_SOLID, 1, controls::colorref(p.panel));
            let old_pen = SelectObject(hdc, pen.into());
            let old_brush = SelectObject(hdc, self.brushes.panel.into());
            let r = self.s(16);
            let _ = RoundRect(
                hdc,
                self.s(CARD_X),
                self.s(CARD_Y),
                self.s(CARD_X + CARD_W),
                self.s(CARD_Y + CARD_H),
                r,
                r,
            );
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(pen.into());

            let old_font = SelectObject(hdc, self.font_title.into());
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, controls::colorref(p.accent));
            let mut rc = RECT {
                left: self.s(LBL_X),
                top: self.s(CARD_Y + CARD_PAD),
                right: self.s(CARD_X + CARD_W - CARD_PAD),
                bottom: self.s(CARD_Y + CARD_PAD + TITLE_H),
            };
            let mut title: Vec<u16> = self.titles[self.page].encode_utf16().collect();
            DrawTextW(hdc, &mut title, &mut rc, DT_SINGLELINE);
            SelectObject(hdc, old_font);
            let _ = EndPaint(self.hwnd, &ps);
        }
    }
}

impl WindowHandler for Dialog {
    fn message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_ERASEBKGND => {
                unsafe {
                    let mut rc = RECT::default();
                    let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc);
                    windows::Win32::Graphics::Gdi::FillRect(
                        windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _),
                        &rc,
                        self.brushes.bg,
                    );
                }
                Some(LRESULT(1))
            }
            WM_PAINT => {
                self.paint();
                Some(LRESULT(0))
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let p = theme::current();
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                let child = HWND(lparam.0 as *mut _);
                // A read-only edit sends WM_CTLCOLORSTATIC (editable ones use WM_CTLCOLOREDIT), so any
                // tracked edit landing here is read-only -> field background + readable dim text.
                if self.edits.contains(&child) {
                    let brush = controls::ctl_colors(hdc, p.text_dim, p.bg_hover, self.brushes.field);
                    return Some(LRESULT(brush.0 as isize));
                }
                let dim = self.dim.contains(&child) || self.dim_dyn.contains(&child);
                let text = if dim { p.text_dim } else { p.text };
                let brush = controls::ctl_colors(hdc, text, p.panel, self.brushes.panel);
                Some(LRESULT(brush.0 as isize))
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                let p = theme::current();
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                let brush = controls::ctl_colors(hdc, p.text, p.bg_hover, self.brushes.field);
                Some(LRESULT(brush.0 as isize))
            }
            WM_DRAWITEM => {
                const ODT_COMBOBOX: u32 = 3; // winuser.h; not surfaced by the windows crate
                let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
                if dis.CtlType.0 == ODT_COMBOBOX {
                    controls::draw_combo(dis, self.font, i18n::is_rtl());
                    return Some(LRESULT(1));
                }
                if self.checks.contains(&dis.hwndItem) {
                    let text = controls::get_text(dis.hwndItem);
                    let on = controls::checked(dis.hwndItem);
                    controls::draw_checkbox(dis, &text, self.font, on, i18n::is_rtl());
                    return Some(LRESULT(1));
                }
                let id = dis.CtlID as i32;
                let kind = if (ID_NAV_BASE..ID_NAV_BASE + PAGES as i32).contains(&id) {
                    if (id - ID_NAV_BASE) as usize == self.page {
                        ButtonKind::NavActive
                    } else {
                        ButtonKind::NavIdle
                    }
                } else if id == ID_SAVE {
                    ButtonKind::Accent
                } else {
                    ButtonKind::Normal
                };
                let text = controls::get_text(dis.hwndItem);
                controls::draw_button(dis, &text, self.font, kind, i18n::is_rtl());
                Some(LRESULT(1))
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                self.on_command(id, code);
                Some(LRESULT(0))
            }
            WM_DETECT_RESULT => {
                let loc = *unsafe { Box::from_raw(lparam.0 as *mut Option<DetectedLocation>) };
                let btn = self.item(ID_DETECT);
                controls::enable(btn, true);
                controls::set_text(btn, i18n::t("btn.detect"));
                match loc {
                    Some(l) => self.apply_detected(&l),
                    None => self.msg(i18n::t("msg.detectFail"), i18n::t("msg.detectCaption"), MB_ICONINFORMATION),
                }
                Some(LRESULT(0))
            }
            WM_PARSE_RESULT => {
                let loc = *unsafe { Box::from_raw(lparam.0 as *mut Option<DetectedLocation>) };
                match loc {
                    Some(l) => self.apply_detected(&l),
                    None => self.msg(i18n::t("msg.pasteFail"), i18n::t("msg.pasteCaption"), MB_ICONINFORMATION),
                }
                Some(LRESULT(0))
            }
            WM_CLOSE => {
                self.finish(SettingsResult::Cancel);
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

fn index_of_method(key: &str) -> i32 {
    METHODS.iter().position(|m| m.key == key).unwrap_or(0) as i32
}

fn nearest_size(pct: i32) -> usize {
    let mut best = 0;
    for (i, s) in SIZE_STEPS.iter().enumerate() {
        if (s - pct).abs() < (SIZE_STEPS[best] - pct).abs() {
            best = i;
        }
    }
    best
}

fn set_adjust(c: &mut AppConfig, i: usize, v: i32) {
    match i {
        0 => c.fajr_adjust = v,
        1 => c.dhuhr_adjust = v,
        2 => c.asr_adjust = v,
        3 => c.maghrib_adjust = v,
        _ => c.isha_adjust = v,
    }
}

fn open_url(url: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let u = window::utf16z(url);
    unsafe {
        ShellExecuteW(None, w!("open"), PCWSTR(u.as_ptr()), None, None, SW_SHOWNORMAL);
    }
}
