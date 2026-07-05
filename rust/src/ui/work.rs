//! Work-hours dialog: today's total on top, a SysListView32 of per-day worked time newest-first,
//! Reset (confirm) and Close. Same modal/theming pattern as ui/usage.rs.

use crate::i18n;
use crate::native::displays;
use crate::services::work_clock::{self, WorkClock};
use crate::ui::controls::{self, ButtonKind};
use crate::ui::theme;
use crate::ui::window::{self, WindowHandler};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::{DeleteObject, HFONT};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, SetWindowTheme, ICC_LISTVIEW_CLASSES, INITCOMMONCONTROLSEX, LVCFMT_LEFT,
    LVCFMT_RIGHT, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVIF_TEXT, LVITEMW,
};
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW,
    IsDialogMessageW, MessageBoxW, MoveWindow, PostQuitMessage, SendMessageW, SetForegroundWindow,
    ShowWindow, TranslateMessage, HMENU, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_RIGHT,
    MB_RTLREADING, MB_YESNO, MSG, SW_SHOW, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC,
    WM_DRAWITEM, WM_ERASEBKGND, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_DLGMODALFRAME,
    WS_EX_LAYOUTRTL, WS_EX_TOPMOST, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

const ID_CLOSE: i32 = 2; // = IDCANCEL so ESC closes
const ID_RESET: i32 = 10;
const ID_LIST: i32 = 20;

const LVM_FIRST: u32 = 0x1000;
const LVM_DELETEALLITEMS: u32 = LVM_FIRST + 9;
const LVM_INSERTCOLUMNW: u32 = LVM_FIRST + 97;
const LVM_INSERTITEMW: u32 = LVM_FIRST + 77;
const LVM_SETITEMW: u32 = LVM_FIRST + 76;
const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = LVM_FIRST + 54;
const LVM_SETBKCOLOR: u32 = LVM_FIRST + 1;
const LVM_SETTEXTCOLOR: u32 = LVM_FIRST + 36;
const LVM_SETTEXTBKCOLOR: u32 = LVM_FIRST + 38;
const LVS_REPORT: u32 = 0x0001;
const LVS_SINGLESEL: u32 = 0x0004;
const LVS_EX_FULLROWSELECT: isize = 0x0000_0020;
const LVS_EX_DOUBLEBUFFER: isize = 0x0001_0000;

// 96-dpi base metrics.
const M: i32 = 14;
const TODAY_H: i32 = 24;
const LIST_Y: i32 = M + TODAY_H + 8;
const BTN_H: i32 = 30;
const BTN_W: i32 = 96;
const CLIENT_W: i32 = 320;
const CLIENT_H: i32 = 430;

struct Dialog {
    clock: *mut WorkClock,
    hwnd: HWND,
    list: HWND,
    done: bool,
    scale: f32,
    font: HFONT,
    font_bold: HFONT,
    brushes: controls::Brushes,
}

/// Modal work-hours dialog (nested message loop). `clock` is a raw pointer (not `&mut`) so the
/// caller holds no borrow across the loop — the nested loop can re-enter the app.
pub fn show(_owner: HWND, clock: *mut WorkClock) {
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
    }

    let mut dlg = Box::new(Dialog {
        clock,
        hwnd: HWND::default(),
        list: HWND::default(),
        done: false,
        scale: 1.0,
        font: HFONT::default(),
        font_bold: HFONT::default(),
        brushes: controls::Brushes::new(),
    });

    let rtl = if i18n::is_rtl() { WS_EX_LAYOUTRTL } else { WINDOW_EX_STYLE::default() };
    let title = window::utf16z(i18n::t("work.title"));
    let hwnd = window::create(
        w_class(),
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
    .expect("work window");
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
    dlg.font_bold = controls::ui_font(&theme::family(), 10.0, true, dpi.max(96));
    dlg.place_window();
    dlg.build();
    dlg.refill();
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }

    let mut msg = MSG::default();
    unsafe {
        while !dlg.is_done() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            if IsDialogMessageW(dlg.hwnd, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn w_class() -> PCWSTR {
    windows::core::w!("PrayerTrayWork")
}

impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }
            if !self.font_bold.is_invalid() {
                let _ = DeleteObject(self.font_bold.into());
            }
        }
    }
}

impl Dialog {
    fn clock(&mut self) -> &mut WorkClock {
        unsafe { &mut *self.clock }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn s(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    fn place_window(&self) {
        let style = WS_CAPTION | WS_SYSMENU;
        let mut rc = RECT { left: 0, top: 0, right: self.s(CLIENT_W), bottom: self.s(CLIENT_H) };
        unsafe {
            let _ = AdjustWindowRectEx(&mut rc, style, false, WS_EX_DLGMODALFRAME);
        }
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let work = displays::primary()
            .map(|m| m.work)
            .unwrap_or(RECT { left: 0, top: 0, right: 1920, bottom: 1080 });
        let x = work.left + ((work.right - work.left) - w) / 2;
        let y = work.top + ((work.bottom - work.top) - h) / 2;
        unsafe {
            let _ = MoveWindow(self.hwnd, x, y.max(work.top), w, h, false);
        }
    }

    fn build(&mut self) {
        let today = controls::label(self.hwnd, "", self.s(M), self.s(M), self.s(CLIENT_W - 2 * M), self.s(TODAY_H));
        controls::set_font(today, self.font_bold);

        let list_h = CLIENT_H - LIST_Y - M - BTN_H - 10;
        let style = WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | WS_TABSTOP.0 | WS_VSCROLL.0
            | LVS_REPORT | LVS_SINGLESEL;
        let list = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("SysListView32"),
                PCWSTR::null(),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(style),
                self.s(M),
                self.s(LIST_Y),
                self.s(CLIENT_W - 2 * M),
                self.s(list_h),
                Some(self.hwnd),
                Some(HMENU(ID_LIST as usize as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default()
        };
        self.list = list;
        controls::set_font(list, self.font);

        let pal = theme::current();
        unsafe {
            SendMessageW(
                list,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                Some(WPARAM(0)),
                Some(LPARAM(LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER)),
            );
            let bg = controls::colorref(pal.bg_hover).0 as isize;
            SendMessageW(list, LVM_SETBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg)));
            SendMessageW(list, LVM_SETTEXTBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg)));
            SendMessageW(
                list,
                LVM_SETTEXTCOLOR,
                Some(WPARAM(0)),
                Some(LPARAM(controls::colorref(pal.text).0 as isize)),
            );
            if pal.is_dark {
                let _ = SetWindowTheme(list, windows::core::w!("DarkMode_Explorer"), PCWSTR::null());
            }
        }

        let cols = [
            (i18n::t("work.date").to_string(), 150, LVCFMT_LEFT),
            (i18n::t("work.worked").to_string(), 122, LVCFMT_RIGHT),
        ];
        for (i, (text, w, fmt)) in cols.iter().enumerate() {
            let mut wide = window::utf16z(text);
            let col = LVCOLUMNW {
                mask: LVCF_TEXT | LVCF_WIDTH | LVCF_FMT | LVCF_SUBITEM,
                fmt: *fmt,
                cx: self.s(*w),
                pszText: PWSTR(wide.as_mut_ptr()),
                iSubItem: i as i32,
                ..Default::default()
            };
            unsafe {
                SendMessageW(
                    list,
                    LVM_INSERTCOLUMNW,
                    Some(WPARAM(i)),
                    Some(LPARAM(&col as *const _ as isize)),
                );
            }
        }

        let by = CLIENT_H - M - BTN_H;
        controls::button(self.hwnd, i18n::t("btn.resetWork"), self.s(M), self.s(by), self.s(BTN_W), self.s(BTN_H), ID_RESET);
        controls::button(self.hwnd, i18n::t("btn.close"), self.s(CLIENT_W - M - BTN_W), self.s(by), self.s(BTN_W), self.s(BTN_H), ID_CLOSE);
        controls::set_font(self.item(ID_RESET), self.font);
        controls::set_font(self.item(ID_CLOSE), self.font);
    }

    fn item(&self, id: i32) -> HWND {
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(self.hwnd), id).unwrap_or_default()
        }
    }

    fn set_lv_text(&self, row: i32, col: i32, text: &str) {
        let mut wide = window::utf16z(text);
        let item = LVITEMW {
            mask: LVIF_TEXT,
            iItem: row,
            iSubItem: col,
            pszText: PWSTR(wide.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            let msg = if col == 0 { LVM_INSERTITEMW } else { LVM_SETITEMW };
            SendMessageW(self.list, msg, Some(WPARAM(0)), Some(LPARAM(&item as *const _ as isize)));
        }
    }

    fn refill(&mut self) {
        let today = self.clock().today();
        let today_text = i18n::f("work.today", &[&work_clock::fmt_hm(today)]);
        controls::set_text(self.item_today(), &today_text);

        unsafe {
            SendMessageW(self.list, LVM_DELETEALLITEMS, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let history = self.clock().history();
        for (row, (date, secs)) in history.iter().enumerate() {
            let r = row as i32;
            self.set_lv_text(r, 0, date);
            self.set_lv_text(r, 1, &work_clock::fmt_hm(*secs));
        }
    }

    // The today label is the first (and only STATIC) child.
    fn item_today(&self) -> HWND {
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindow(
                self.hwnd,
                windows::Win32::UI::WindowsAndMessaging::GW_CHILD,
            )
            .unwrap_or_default()
        }
    }

    fn on_reset(&mut self) {
        let body = window::utf16z(i18n::t("work.resetConfirm"));
        let caption = window::utf16z(i18n::t("work.title"));
        let style = MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2
            | if i18n::is_rtl() { MB_RTLREADING | MB_RIGHT } else { Default::default() };
        let r = unsafe {
            MessageBoxW(Some(self.hwnd), PCWSTR(body.as_ptr()), PCWSTR(caption.as_ptr()), style)
        };
        if r == IDYES {
            self.clock().reset();
            self.refill();
        }
    }

    fn finish(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        // Destroy (not just hide): a leaked top-level window keeps receiving broadcasts and would
        // dispatch into the freed Dialog via stale GWLP_USERDATA.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
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
            WM_CTLCOLORSTATIC => {
                let p = theme::current();
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                let brush = controls::ctl_colors(hdc, p.accent, p.bg, self.brushes.bg);
                Some(LRESULT(brush.0 as isize))
            }
            WM_DRAWITEM => {
                let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
                let kind = if dis.CtlID as i32 == ID_CLOSE {
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
                match id {
                    ID_RESET => self.on_reset(),
                    ID_CLOSE => self.finish(),
                    _ => {}
                }
                Some(LRESULT(0))
            }
            WM_CLOSE => {
                self.finish();
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}
