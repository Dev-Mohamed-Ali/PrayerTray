//! Borderless popup listing today's times, port of UI/PrayerPopup.cs.
//! Pinnable (draggable, remembers position); auto-hides on deactivate when unpinned.

use crate::i18n;
use crate::native::displays;
use crate::ui::gdip::{self, Bitmap, Font, Graphics, Pen, SolidBrush, StringFormat};
use crate::ui::theme::{self, argb};
use crate::ui::window::{self, WindowHandler};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetWindowRect, IsWindowVisible, MoveWindow, PostMessageW,
    SendMessageW, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNA, WM_ACTIVATE, WM_APP, WM_ERASEBKGND,
    WM_EXITSIZEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NCLBUTTONDOWN, WM_PAINT,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

/// Posted to the app window when the user toggles the pin (wparam = pinned as 0/1).
pub const WM_POPUP_PIN: u32 = WM_APP + 4;
/// Posted to the app window when the pinned popup is dragged (x low 32, y high 32 of lparam).
pub const WM_POPUP_MOVED: u32 = WM_APP + 5;

const HTCAPTION: usize = 2;
const WA_INACTIVE: usize = 0;

pub struct Row {
    pub label: String,
    pub time: String,
    pub is_next: bool,
    pub is_sunrise: bool,
}

pub struct Popup {
    pub hwnd: HWND,
    app_hwnd: HWND,
    city: String,
    date: String,
    hijri: String,
    event: String,
    fast: String,
    usage: String,
    work: String,
    chip_override: String,
    countdown: String,
    next_label: String,
    rows: Vec<Row>,
    widget_rect: RECT,
    anchor_right: bool,
    pinned: bool,
    saved_x: i32,
    saved_y: i32,
    positioning: bool,
    pin_box: RECT,
    width: i32,
    height: i32,
}

fn scaled(v: i32) -> i32 {
    (v as f32 * theme::font_scale()).round() as i32
}

impl Popup {
    pub fn new(app_hwnd: HWND) -> Box<Self> {
        gdip::init();
        let mut p = Box::new(Self {
            hwnd: HWND::default(),
            app_hwnd,
            city: String::new(),
            date: String::new(),
            hijri: String::new(),
            event: String::new(),
            fast: String::new(),
            usage: String::new(),
            work: String::new(),
            chip_override: String::new(),
            countdown: String::new(),
            next_label: String::new(),
            rows: Vec::new(),
            widget_rect: RECT::default(),
            anchor_right: true,
            pinned: false,
            saved_x: i32::MIN,
            saved_y: i32::MIN,
            positioning: false,
            pin_box: RECT::default(),
            width: 268,
            height: 200,
        });
        let hwnd = window::create(
            w!("PrayerTrayPopup"),
            w!(""),
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            WS_POPUP, // hidden until ShowTimes
            0,
            0,
            p.width,
            p.height,
            None,
            p.as_mut() as *mut Self,
        )
        .expect("popup window");
        p.hwnd = hwnd;
        unsafe {
            let pref = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const core::ffi::c_void,
                4,
            );
        }
        p
    }

    /// Seed pin state + last position from config.
    pub fn init_pin(&mut self, pinned: bool, x: i32, y: i32) {
        self.pinned = pinned;
        self.saved_x = x;
        self.saved_y = y;
    }

    pub fn visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn header_h(&self) -> i32 {
        scaled(
            (if self.hijri.is_empty() { 60 } else { 74 })
                + (if self.event.is_empty() { 0 } else { 16 })
                + (if self.fast.is_empty() { 0 } else { 16 }),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_times(
        &mut self,
        city: &str,
        date: crate::datetime::Date,
        rows: Vec<Row>,
        countdown: &str,
        widget_rect: RECT,
        anchor_right: bool,
        hijri: &str,
        event: &str,
        fast: &str,
        usage: &str,
        work: &str,
        chip_override: &str,
    ) {
        self.widget_rect = widget_rect;
        self.anchor_right = anchor_right;
        self.city = city.into();
        self.date = i18n::format_popup_date(date);
        self.hijri = hijri.into();
        self.event = event.into();
        self.fast = fast.into();
        self.usage = usage.into();
        self.work = work.into();
        self.chip_override = chip_override.into();
        self.countdown = countdown.into();
        self.next_label = rows.iter().find(|r| r.is_next).map(|r| r.label.clone()).unwrap_or_default();
        self.rows = rows;

        self.width = scaled(268);
        let row_h = scaled(38);
        let footer_lines = i32::from(!self.usage.is_empty()) + i32::from(!self.work.is_empty());
        let footer = footer_lines * scaled(22);
        self.height = self.header_h() + self.rows.len() as i32 * row_h + footer + scaled(16);
        self.position();
        self.invalidate();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            let _ = SetForegroundWindow(self.hwnd); // Activate: enables deactivate-to-hide
        }
    }

    fn position(&mut self) {
        self.positioning = true;
        let (x, y) = if self.pinned && self.saved_x != i32::MIN {
            let wa = displays::all()
                .into_iter()
                .find(|m| {
                    self.saved_x >= m.bounds.left
                        && self.saved_x < m.bounds.right
                        && self.saved_y >= m.bounds.top
                        && self.saved_y < m.bounds.bottom
                })
                .or_else(displays::primary)
                .map(|m| m.work)
                .unwrap_or_default();
            (
                self.saved_x.clamp(wa.left, (wa.right - self.width).max(wa.left)),
                self.saved_y.clamp(wa.top, (wa.bottom - self.height).max(wa.top)),
            )
        } else {
            let b = displays::all()
                .into_iter()
                .find(|m| {
                    self.widget_rect.left >= m.bounds.left && self.widget_rect.left < m.bounds.right
                })
                .or_else(displays::primary)
                .map(|m| m.bounds)
                .unwrap_or_default();
            let x = if self.anchor_right {
                self.widget_rect.right - self.width
            } else {
                self.widget_rect.left
            };
            (
                x.clamp(b.left + 8, b.right - self.width - 8),
                self.widget_rect.top - 8 - self.height,
            )
        };
        unsafe {
            let _ = MoveWindow(self.hwnd, x, y, self.width, self.height, true);
        }
        self.positioning = false;
    }

    fn save_position(&mut self) {
        let mut r = RECT::default();
        unsafe {
            let _ = GetWindowRect(self.hwnd, &mut r);
        }
        self.saved_x = r.left;
        self.saved_y = r.top;
        let packed = ((r.top as i64) << 32) | (r.left as i64 & 0xFFFF_FFFF);
        unsafe {
            let _ = PostMessageW(Some(self.app_hwnd), WM_POPUP_MOVED, WPARAM(0), LPARAM(packed as isize));
        }
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn in_pin_box(&self, x: i32, y: i32) -> bool {
        x >= self.pin_box.left && x < self.pin_box.right && y >= self.pin_box.top && y < self.pin_box.bottom
    }

    fn paint(&mut self) {
        let mut ps = PAINTSTRUCT::default();
        let hdc = unsafe { BeginPaint(self.hwnd, &mut ps) };
        {
            let buffer = Bitmap::new(self.width, self.height);
            self.render(&Graphics::from_bitmap(&buffer));
            Graphics::from_hdc(hdc).draw_bitmap(&buffer, 0, 0);
        }
        unsafe {
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn render(&mut self, g: &Graphics) {
        let pal = theme::current();
        g.clear(pal.panel);

        let rtl = i18n::is_rtl();
        let fs = theme::font_scale();
        let fam = theme::family();
        let f_city = Font::new_pt(&fam, 11.0 * fs, gdip::STYLE_BOLD);
        let f_date = Font::new_pt(&fam, 8.5 * fs, gdip::STYLE_REGULAR);
        let f_row = Font::new_pt(&fam, 10.5 * fs, gdip::STYLE_REGULAR);
        let f_row_b = Font::new_pt(&fam, 10.5 * fs, gdip::STYLE_BOLD);
        let f_chip = Font::new_pt(&fam, 9.0 * fs, gdip::STYLE_BOLD);

        let pad = scaled(16) as f32;
        let w = self.width as f32;
        let header_h = self.header_h();

        let text = SolidBrush::new(pal.text);
        let dim = SolidBrush::new(pal.text_dim);
        let accent = SolidBrush::new(pal.accent);
        let accent_soft = SolidBrush::new(pal.accent_soft);

        // Countdown chip measured first so the date row reserves room for it.
        let chip = if !self.chip_override.is_empty() {
            self.chip_override.clone()
        } else if self.countdown.is_empty() {
            String::new()
        } else {
            format!("{} {} {}", self.next_label, i18n::t("popup.in"), self.countdown)
        };
        let chip_w = if chip.is_empty() {
            0.0
        } else {
            g.measure_string(&chip, &f_chip).0 + scaled(14) as f32
        };

        let hdr_align = if rtl { gdip::ALIGN_FAR } else { gdip::ALIGN_NEAR };
        let sf_hdr = StringFormat::new(hdr_align, gdip::ALIGN_NEAR);
        g.draw_string(&self.city, &f_city, &text, gdip::rectf(pad, scaled(12) as f32, w - 2.0 * pad, scaled(22) as f32), &sf_hdr);
        let date_w = w - 2.0 * pad - if chip_w > 0.0 { chip_w + scaled(6) as f32 } else { 0.0 };
        let date_x = if rtl { w - pad - date_w } else { pad };
        g.draw_string(&self.date, &f_date, &dim, gdip::rectf(date_x, scaled(34) as f32, date_w, scaled(20) as f32), &sf_hdr);
        if !self.hijri.is_empty() {
            g.draw_string(&self.hijri, &f_date, &dim, gdip::rectf(pad, scaled(52) as f32, w - 2.0 * pad, scaled(18) as f32), &sf_hdr);
        }
        if !self.event.is_empty() {
            let ey = scaled(if self.hijri.is_empty() { 52 } else { 68 }) as f32;
            g.draw_string(&self.event, &f_date, &accent, gdip::rectf(pad, ey, w - 2.0 * pad, scaled(18) as f32), &sf_hdr);
        }
        if !self.fast.is_empty() {
            let fy = 52 + if self.hijri.is_empty() { 0 } else { 16 } + if self.event.is_empty() { 0 } else { 16 };
            g.draw_string(&self.fast, &f_date, &accent, gdip::rectf(pad, scaled(fy) as f32, w - 2.0 * pad, scaled(18) as f32), &sf_hdr);
        }

        self.draw_pin(g, rtl);

        if chip_w > 0.0 {
            let chip_x = if rtl { pad + scaled(2) as f32 } else { w - pad - chip_w - scaled(2) as f32 };
            let chip_rect = gdip::rectf(chip_x, scaled(34) as f32, chip_w, scaled(22) as f32);
            g.fill_rounded(&accent_soft, chip_rect, scaled(11) as f32);
            g.draw_string(
                &chip,
                &f_chip,
                &accent,
                gdip::rectf(chip_rect.X + scaled(7) as f32, chip_rect.Y + scaled(3) as f32, chip_w, scaled(18) as f32),
                &StringFormat::new(gdip::ALIGN_NEAR, gdip::ALIGN_NEAR),
            );
        }

        let sep = Pen::new(argb(60, 60, 64), 1.0);
        let sep_y = (header_h - scaled(6)) as f32;
        g.draw_line(&sep, pad, sep_y, w - pad, sep_y);

        let sf_label = StringFormat::new(if rtl { gdip::ALIGN_FAR } else { gdip::ALIGN_NEAR }, gdip::ALIGN_CENTER);
        let sf_time = StringFormat::new(if rtl { gdip::ALIGN_NEAR } else { gdip::ALIGN_FAR }, gdip::ALIGN_CENTER);
        let row_h = scaled(38);
        let mut y = header_h;
        for r in &self.rows {
            let row_rect = gdip::rectf(
                scaled(8) as f32,
                y as f32,
                (self.width - scaled(16)) as f32,
                (row_h - scaled(4)) as f32,
            );
            if r.is_next {
                g.fill_rounded(&accent_soft, row_rect, scaled(10) as f32);
                let bar_x = if rtl {
                    row_rect.X + row_rect.Width - scaled(6) as f32
                } else {
                    row_rect.X + 2.0
                };
                g.fill_rounded(
                    &accent,
                    gdip::rectf(bar_x, row_rect.Y + scaled(7) as f32, 4.0, row_rect.Height - scaled(14) as f32),
                    2.0,
                );
            }

            let fg = if r.is_next {
                &accent
            } else if r.is_sunrise {
                &dim
            } else {
                &text
            };
            let font = if r.is_next { &f_row_b } else { &f_row };
            let rect = gdip::rectf(
                pad + scaled(6) as f32,
                y as f32,
                w - 2.0 * pad - scaled(6) as f32,
                (row_h - scaled(4)) as f32,
            );
            g.draw_string(&r.label, font, fg, rect, &sf_label);
            g.draw_string(&r.time, font, fg, rect, &sf_time);
            y += row_h;
        }

        for line in [&self.usage, &self.work] {
            if line.is_empty() {
                continue;
            }
            g.draw_string(
                line,
                &f_date,
                &dim,
                gdip::rectf(pad, y as f32 + scaled(4) as f32, w - 2.0 * pad, scaled(18) as f32),
                &sf_hdr,
            );
            y += scaled(22);
        }
    }

    /// Thumbtack in the top corner; filled accent when pinned, hollow otherwise.
    fn draw_pin(&mut self, g: &Graphics, rtl: bool) {
        let s = scaled(18);
        let pad = scaled(16);
        let x = if rtl { pad } else { self.width - pad - s };
        self.pin_box = RECT { left: x, top: scaled(11), right: x + s, bottom: scaled(11) + s };
        let pal = theme::current();
        let c = if self.pinned { pal.accent } else { pal.text_dim };
        let sf = s as f32;
        let hx = x as f32 + sf * 0.5;
        let hy = self.pin_box.top as f32 + sf * 0.34;
        let hr = sf * 0.30;
        let pen = Pen::new(c, (sf * 0.10).max(1.4));
        g.draw_line(&pen, hx, hy + hr, hx, (self.pin_box.bottom - scaled(2)) as f32); // needle
        if self.pinned {
            let b = SolidBrush::new(c);
            g.fill_ellipse(&b, hx - hr, hy - hr, hr * 2.0, hr * 2.0);
        } else {
            g.draw_ellipse(&pen, hx - hr, hy - hr, hr * 2.0, hr * 2.0); // head
        }
    }
}

impl WindowHandler for Popup {
    fn message(&mut self, _hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_ERASEBKGND => Some(LRESULT(1)),
            WM_PAINT => {
                self.paint();
                Some(LRESULT(0))
            }
            WM_ACTIVATE => {
                if (wparam.0 & 0xFFFF) == WA_INACTIVE && !self.pinned {
                    self.hide();
                }
                None
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (lparam.0 as i32 & 0xFFFF, (lparam.0 as i32 >> 16) & 0xFFFF);
                if !self.in_pin_box(x, y) && y < self.header_h() {
                    // Drag from the header.
                    unsafe {
                        let _ = ReleaseCapture();
                        SendMessageW(self.hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION)), Some(LPARAM(0)));
                    }
                }
                None
            }
            WM_LBUTTONUP => {
                let (x, y) = (lparam.0 as i32 & 0xFFFF, (lparam.0 as i32 >> 16) & 0xFFFF);
                if self.in_pin_box(x, y) {
                    self.pinned = !self.pinned;
                    unsafe {
                        let _ = PostMessageW(
                            Some(self.app_hwnd),
                            WM_POPUP_PIN,
                            WPARAM(self.pinned as usize),
                            LPARAM(0),
                        );
                    }
                    if self.pinned {
                        self.save_position();
                    }
                    self.invalidate();
                }
                None
            }
            WM_EXITSIZEMOVE => {
                if self.pinned && !self.positioning {
                    self.save_position();
                }
                None
            }
            _ => None,
        }
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}
