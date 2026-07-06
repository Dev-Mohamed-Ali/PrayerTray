//! Themed native Win32 child controls (combo/edit/checkbox/owner-drawn buttons)
//! plus the WM_CTLCOLOR* brush set for the settings dialog.

use crate::ui::theme;
use crate::ui::window;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, DrawFrameControl, DrawTextW, EnumFontFamiliesExW,
    FillRect, GetDC, InvalidateRect, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_QUALITY, DFC_BUTTON, DFCS_BUTTONCHECK,
    DFCS_CHECKED, DFCS_INACTIVE, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_RIGHT, DT_SINGLELINE,
    DT_VCENTER, FF_DONTCARE, FW_BOLD, FW_NORMAL, HBRUSH, HDC, HFONT, LOGFONTW, OUT_DEFAULT_PRECIS,
    TEXTMETRICW, TRANSPARENT,
};
use windows::Win32::UI::Controls::{
    SetWindowTheme, DRAWITEMSTRUCT, ODS_COMBOBOXEDIT, ODS_DISABLED, ODS_SELECTED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, SendMessageW,
    SetWindowLongPtrW, SetWindowTextW, BS_OWNERDRAW, CBS_DROPDOWNLIST, CBS_HASSTRINGS,
    CBS_OWNERDRAWFIXED, CB_ADDSTRING, CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN, CB_SETCURSEL,
    ES_AUTOHSCROLL, GWLP_USERDATA, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETFONT, WS_BORDER,
    WS_CHILD, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

const EM_SETCUEBANNER: u32 = 0x1501;
const EM_SETREADONLY: u32 = 0x00CF;
const SS_LEFTNOWORDWRAP: u32 = 0x0000_000C;

/// 0xAARRGGBB (theme) -> 0x00BBGGRR (GDI).
pub fn colorref(argb: u32) -> COLORREF {
    COLORREF(((argb & 0xFF) << 16) | (argb & 0xFF00) | ((argb >> 16) & 0xFF))
}

/// Black or white, whichever reads on the accent (port of SettingsForm.OnAccent).
pub fn on_accent(accent: u32) -> u32 {
    let (r, g, b) = ((accent >> 16) & 0xFF, (accent >> 8) & 0xFF, accent & 0xFF);
    if (r * 299 + g * 587 + b * 114) / 1000 > 140 {
        0xFF00_0000
    } else {
        0xFFFF_FFFF
    }
}

fn solid(argb: u32) -> HBRUSH {
    unsafe { CreateSolidBrush(colorref(argb)) }
}

/// Cached WM_CTLCOLOR* brushes for the current palette.
pub struct Brushes {
    pub bg: HBRUSH,
    pub panel: HBRUSH,
    pub field: HBRUSH,
}

impl Brushes {
    pub fn new() -> Self {
        let p = theme::current();
        Self {
            bg: solid(p.bg),
            panel: solid(p.panel),
            field: solid(p.bg_hover),
        }
    }
}

impl Default for Brushes {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Brushes {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.bg.into());
            let _ = DeleteObject(self.panel.into());
            let _ = DeleteObject(self.field.into());
        }
    }
}

pub fn ui_font(family: &str, pt: f32, bold: bool, dpi: u32) -> HFONT {
    let name = window::utf16z(family);
    let height = -((pt * dpi as f32 / 72.0).round() as i32);
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            if bold { FW_BOLD.0 as i32 } else { FW_NORMAL.0 as i32 },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            FF_DONTCARE.0 as u32,
            PCWSTR(name.as_ptr()),
        )
    }
}

fn create(parent: HWND, class: PCWSTR, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
    let t = window::utf16z(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(t.as_ptr()),
            WINDOW_STYLE(style) | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as usize as *mut core::ffi::c_void)),
            None,
            None,
        )
        .unwrap_or_default()
    }
}

pub fn label(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
    create(parent, w!("STATIC"), text, SS_LEFTNOWORDWRAP, x, y, w, h, 0)
}

pub fn edit(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
    create(
        parent,
        w!("EDIT"),
        text,
        WS_BORDER.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32,
        x,
        y,
        w,
        h,
        id,
    )
}

pub fn combo(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
    let c = create(
        parent,
        w!("COMBOBOX"),
        "",
        WS_TABSTOP.0 | WS_VSCROLL.0 | (CBS_DROPDOWNLIST | CBS_HASSTRINGS | CBS_OWNERDRAWFIXED) as u32,
        x,
        y,
        w,
        h,
        id,
    );
    if theme::current().is_dark {
        unsafe {
            let _ = SetWindowTheme(c, w!("DarkMode_CFD"), PCWSTR::null());
        }
    }
    c
}

/// Owner-drawn (BS_OWNERDRAW) so disabled text uses a readable dim color instead of system grey.
/// Check state lives in the window's GWLP_USERDATA (owner-draw buttons don't auto-toggle or track
/// BM_*CHECK), read/written via `checked`/`set_checked` and flipped on click by `toggle_check`.
pub fn checkbox(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
    create(parent, w!("BUTTON"), text, WS_TABSTOP.0 | BS_OWNERDRAW as u32, x, y, w, h, id)
}

/// Owner-drawn push button; paint it from WM_DRAWITEM via `draw_button`.
pub fn button(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
    create(parent, w!("BUTTON"), text, WS_TABSTOP.0 | BS_OWNERDRAW as u32, x, y, w, h, id)
}

pub fn set_font(h: HWND, f: HFONT) {
    unsafe {
        SendMessageW(h, WM_SETFONT, Some(WPARAM(f.0 as usize)), Some(LPARAM(1)));
    }
}

pub fn combo_add(h: HWND, s: &str) {
    let t = window::utf16z(s);
    unsafe {
        SendMessageW(h, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(t.as_ptr() as isize)));
    }
}

/// -1 clears the selection.
pub fn combo_set(h: HWND, i: i32) {
    unsafe {
        SendMessageW(h, CB_SETCURSEL, Some(WPARAM(i as isize as usize)), Some(LPARAM(0)));
    }
}

pub fn combo_sel(h: HWND) -> i32 {
    unsafe { SendMessageW(h, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0 as i32 }
}

pub fn set_text(h: HWND, s: &str) {
    let t = window::utf16z(s);
    unsafe {
        let _ = SetWindowTextW(h, PCWSTR(t.as_ptr()));
    }
}

pub fn get_text(h: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn set_readonly(h: HWND, v: bool) {
    unsafe {
        SendMessageW(h, EM_SETREADONLY, Some(WPARAM(v as usize)), Some(LPARAM(0)));
    }
}

/// Owner-drawn checkbox state, stored in GWLP_USERDATA (see `checkbox`).
pub fn checked(h: HWND) -> bool {
    unsafe { GetWindowLongPtrW(h, GWLP_USERDATA) != 0 }
}

pub fn set_checked(h: HWND, v: bool) {
    unsafe {
        SetWindowLongPtrW(h, GWLP_USERDATA, v as isize);
        let _ = InvalidateRect(Some(h), None, false);
    }
}

pub fn toggle_check(h: HWND) {
    set_checked(h, !checked(h));
}

pub fn enable(h: HWND, v: bool) {
    unsafe {
        let _ = EnableWindow(h, v);
    }
}

pub fn cue_banner(h: HWND, text: &str) {
    let t = window::utf16z(text);
    unsafe {
        SendMessageW(h, EM_SETCUEBANNER, Some(WPARAM(1)), Some(LPARAM(t.as_ptr() as isize)));
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonKind {
    Accent,
    Normal,
    NavActive,
    NavIdle,
}

/// WM_DRAWITEM painter for `button` controls; nav kinds are start-aligned.
pub fn draw_button(dis: &DRAWITEMSTRUCT, text: &str, font: HFONT, kind: ButtonKind, rtl: bool) {
    let p = theme::current();
    let (bg, fg) = match kind {
        ButtonKind::Accent | ButtonKind::NavActive => (p.accent, on_accent(p.accent)),
        ButtonKind::NavIdle => (p.panel, p.text),
        ButtonKind::Normal => (p.bg_hover, p.text),
    };
    let fg = if (dis.itemState.0 & ODS_DISABLED.0) != 0 { p.text_dim } else { fg };
    unsafe {
        let brush = solid(bg);
        FillRect(dis.hDC, &dis.rcItem, brush);
        let _ = DeleteObject(brush.into());
        SetBkMode(dis.hDC, TRANSPARENT);
        SetTextColor(dis.hDC, colorref(fg));
        let old = SelectObject(dis.hDC, font.into());
        let nav = matches!(kind, ButtonKind::NavActive | ButtonKind::NavIdle);
        let mut rc = dis.rcItem;
        let flags = if nav {
            rc.left += 10;
            rc.right -= 10;
            DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | if rtl { DT_RIGHT } else { DT_LEFT }
        } else {
            DT_VCENTER | DT_SINGLELINE | DT_CENTER
        };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(dis.hDC, &mut wide, &mut rc, flags);
        SelectObject(dis.hDC, old);
    }
}

/// Fill a WM_CTLCOLOR* HDC and return the brush to hand back as the LRESULT.
pub fn ctl_colors(hdc: HDC, text: u32, bg: u32, brush: HBRUSH) -> HBRUSH {
    unsafe {
        SetTextColor(hdc, colorref(text));
        windows::Win32::Graphics::Gdi::SetBkColor(hdc, colorref(bg));
    }
    brush
}

fn combo_item_text(h: HWND, idx: u32) -> String {
    unsafe {
        let len = SendMessageW(h, CB_GETLBTEXTLEN, Some(WPARAM(idx as usize)), None).0;
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = SendMessageW(h, CB_GETLBTEXT, Some(WPARAM(idx as usize)), Some(LPARAM(buf.as_mut_ptr() as isize))).0;
        if n <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

/// WM_DRAWITEM painter for owner-drawn combos (selected/edit field + dropdown list items).
/// Disabled selected text dims to `text_dim` instead of the theme's system grey.
pub fn draw_combo(dis: &DRAWITEMSTRUCT, font: HFONT, rtl: bool) {
    let p = theme::current();
    let disabled = (dis.itemState.0 & ODS_DISABLED.0) != 0;
    let selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;
    let is_edit = (dis.itemState.0 & ODS_COMBOBOXEDIT.0) != 0;
    unsafe {
        let bg = if selected && !is_edit { p.accent_soft } else { p.bg_hover };
        let brush = solid(bg);
        FillRect(dis.hDC, &dis.rcItem, brush);
        let _ = DeleteObject(brush.into());
        if dis.itemID == u32::MAX {
            return; // empty combo, no selection to draw
        }
        let text = combo_item_text(dis.hwndItem, dis.itemID);
        SetBkMode(dis.hDC, TRANSPARENT);
        SetTextColor(dis.hDC, colorref(if disabled { p.text_dim } else { p.text }));
        let old = SelectObject(dis.hDC, font.into());
        let pad = (dis.rcItem.bottom - dis.rcItem.top) / 3;
        let mut rc = dis.rcItem;
        rc.left += pad;
        rc.right -= pad;
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let flags = DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | if rtl { DT_RIGHT } else { DT_LEFT };
        DrawTextW(dis.hDC, &mut wide, &mut rc, flags);
        SelectObject(dis.hDC, old);
    }
}

/// WM_DRAWITEM painter for owner-drawn checkboxes: classic glyph (DrawFrameControl) + label whose
/// color follows enabled/disabled, so disabled text stays readable (`text_dim`) not system grey.
pub fn draw_checkbox(dis: &DRAWITEMSTRUCT, text: &str, font: HFONT, is_checked: bool, rtl: bool) {
    let p = theme::current();
    let disabled = (dis.itemState.0 & ODS_DISABLED.0) != 0;
    let rc = dis.rcItem;
    let row_h = rc.bottom - rc.top;
    let box_sz = (row_h * 13 / 20).max(11);
    let gap = (row_h / 3).max(4);
    let gy = rc.top + (row_h - box_sz) / 2;
    unsafe {
        let brush = solid(p.panel);
        FillRect(dis.hDC, &rc, brush);
        let _ = DeleteObject(brush.into());

        let (gx, tl, tr) = if rtl {
            (rc.right - box_sz, rc.left, rc.right - box_sz - gap)
        } else {
            (rc.left, rc.left + box_sz + gap, rc.right)
        };
        let mut gr = RECT { left: gx, top: gy, right: gx + box_sz, bottom: gy + box_sz };
        let mut state = DFCS_BUTTONCHECK;
        if is_checked {
            state |= DFCS_CHECKED;
        }
        if disabled {
            state |= DFCS_INACTIVE;
        }
        let _ = DrawFrameControl(dis.hDC, &mut gr, DFC_BUTTON, state);

        SetBkMode(dis.hDC, TRANSPARENT);
        SetTextColor(dis.hDC, colorref(if disabled { p.text_dim } else { p.text }));
        let old = SelectObject(dis.hDC, font.into());
        let mut tr = RECT { left: tl, top: rc.top, right: tr, bottom: rc.bottom };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let flags = DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | if rtl { DT_RIGHT } else { DT_LEFT };
        DrawTextW(dis.hDC, &mut wide, &mut tr, flags);
        SelectObject(dis.hDC, old);
    }
}

/// Installed font family names, sorted, no vertical (@-prefixed) faces.
pub fn font_families() -> Vec<String> {
    unsafe extern "system" fn cb(lf: *const LOGFONTW, _tm: *const TEXTMETRICW, _ty: u32, lp: LPARAM) -> i32 {
        let list = unsafe { &mut *(lp.0 as *mut Vec<String>) };
        let face = unsafe { &(*lf).lfFaceName };
        let len = face.iter().position(|&c| c == 0).unwrap_or(face.len());
        let name = String::from_utf16_lossy(&face[..len]);
        if !name.starts_with('@') && !list.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            list.push(name);
        }
        1
    }
    let mut list: Vec<String> = Vec::new();
    unsafe {
        let hdc = GetDC(None);
        let lf = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };
        EnumFontFamiliesExW(hdc, &lf, Some(cb), LPARAM(&mut list as *mut _ as isize), 0);
        ReleaseDC(None, hdc);
    }
    list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    list
}
