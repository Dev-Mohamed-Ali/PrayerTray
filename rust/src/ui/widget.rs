//! The next-prayer taskbar pill, port of UI/TaskbarWidget.cs: a top-level always-on-top
//! overlay owned by the taskbar window so it rides the taskbar's z-band. Optional net-meter
//! tail segments (speed/ping/usage) render in fixed grow-only slots to keep the width stable.

use crate::i18n;
use crate::native::{displays, taskbar};
use crate::ui::gdip::{self, Bitmap, Font, Graphics, SolidBrush, StringFormat};
use crate::ui::theme;
use crate::ui::window::{self, WindowHandler};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, EndPaint, InvalidateRect, SetWindowRgn, PAINTSTRUCT,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetCursorPos, IsWindowVisible, MoveWindow, PostMessageW,
    ShowWindow, EVENT_OBJECT_REORDER, EVENT_SYSTEM_FOREGROUND, SW_HIDE, SW_SHOWNA, WINEVENT_OUTOFCONTEXT,
    WM_APP, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT,
    WM_RBUTTONUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

const WM_MOUSELEAVE: u32 = 0x02A3;

/// Posted to the app window on pill left-click (toggle popup).
pub const WM_WIDGET_CLICK: u32 = WM_APP + 2;
/// Posted to the app window on pill right-click; cursor pos packed in lparam.
pub const WM_WIDGET_MENU: u32 = WM_APP + 3;
/// Posted to the widget window by the win-event hook (raise/visibility check).
const WM_RAISE: u32 = WM_APP + 10;

const MA_NOACTIVATE: u32 = 3;

// Hook callbacks carry no context; a single widget exists at a time.
static HOOK_TARGET: AtomicIsize = AtomicIsize::new(0);

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let target = HOOK_TARGET.load(Ordering::Relaxed);
    if target != 0 {
        let _ = unsafe { PostMessageW(Some(HWND(target as *mut _)), WM_RAISE, WPARAM(0), LPARAM(0)) };
    }
}

pub struct Widget {
    pub hwnd: HWND,
    app_hwnd: HWND,
    device: Option<String>,
    pub anchor_right: bool,
    pub offset: i32,
    pub hide_on_fullscreen: bool,
    name: String,
    time: String,
    count: String,
    segs: Vec<(String, String)>, // (live text, worst-case template) per tail segment
    seg_slots: Vec<i32>,         // grow-only slot width per segment
    net_font_scale: f32,         // slot basis: cleared when this or the family changes
    net_family: String,
    hover: bool,
    tracking: bool,
    paused: bool,
    suppressed: bool,
    w: i32,
    h: i32,
    scale: f32,
    last_rect: RECT,
    buffer: Option<Bitmap>,
    measure_bmp: Bitmap,
    fg_hook: HWINEVENTHOOK,
    reorder_hook: HWINEVENTHOOK,
    last_raise: u32,
}

fn tick_count() -> u32 {
    unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
}

impl Widget {
    pub fn new(app_hwnd: HWND, device: Option<String>) -> Box<Self> {
        gdip::init();
        let tb = taskbar::taskbar_for_device(device.as_deref());
        let scale = tb.map(taskbar::scale_of).unwrap_or(1.0);

        let mut widget = Box::new(Self {
            hwnd: HWND::default(),
            app_hwnd,
            device,
            anchor_right: true,
            offset: 12,
            hide_on_fullscreen: true,
            name: "—".into(),
            time: String::new(),
            count: "…".into(),
            segs: Vec::new(),
            seg_slots: Vec::new(),
            net_font_scale: 0.0,
            net_family: String::new(),
            hover: false,
            tracking: false,
            paused: false,
            suppressed: false,
            w: 160,
            h: 32,
            scale: if scale > 0.0 { scale } else { 1.0 },
            last_rect: RECT::default(),
            buffer: None,
            measure_bmp: Bitmap::new(1, 1),
            fg_hook: HWINEVENTHOOK::default(),
            reorder_hook: HWINEVENTHOOK::default(),
            last_raise: 0,
        });

        let hwnd = window::create(
            w!("PrayerTrayWidget"),
            w!(""),
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            widget.w,
            widget.h,
            tb, // owner -> rides in the taskbar's z-band
            widget.as_mut() as *mut Self,
        )
        .expect("widget window");
        widget.hwnd = hwnd;
        HOOK_TARGET.store(hwnd.0 as isize, Ordering::Relaxed);

        // Re-raise the instant the taskbar comes forward. Foreground hook is global (infrequent);
        // the chatty reorder hook is scoped to the taskbar's thread — never system-wide.
        unsafe {
            widget.fg_hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            if let Some(tb) = tb {
                let tid = taskbar::thread_of(tb);
                if tid != 0 {
                    widget.reorder_hook = SetWinEventHook(
                        EVENT_OBJECT_REORDER,
                        EVENT_OBJECT_REORDER,
                        None,
                        Some(win_event_proc),
                        0,
                        tid,
                        WINEVENT_OUTOFCONTEXT,
                    );
                }
            }
        }

        widget.render_buffer();
        widget.sync_position();
        widget
    }

    pub fn device_name(&self) -> Option<&str> {
        self.device.as_deref()
    }

    fn s(&self, v: f32) -> i32 {
        (v * self.scale).round() as i32
    }

    fn main_font(&self) -> Font {
        Font::new(&theme::family(), 13.0 * self.scale * theme::font_scale(), gdip::STYLE_REGULAR)
    }

    fn count_font(&self) -> Font {
        Font::new(&theme::family(), 13.0 * self.scale * theme::font_scale(), gdip::STYLE_BOLD)
    }

    fn measure(&self, s: &str, font: &Font) -> f32 {
        Graphics::from_bitmap(&self.measure_bmp).measure_string(s, font).0
    }

    pub fn set_data(&mut self, name: &str, time: &str, countdown: &str) {
        self.name = name.into();
        self.time = time.into();
        self.count = countdown.into();
        self.resize_to_content();
        self.render_buffer();
        self.invalidate();
    }

    /// Live throughput/ping/usage segments, refreshed every second; fixed per-segment slots
    /// keep the pill width static. Each entry is (live text, worst-case template).
    pub fn set_net(&mut self, segs: Vec<(String, String)>) {
        if segs.len() == self.segs.len() {
            let mut same = true;
            let mut tmpl_changed = false;
            #[allow(clippy::needless_range_loop)] // parallel index into self.segs and segs
            for i in 0..self.segs.len() {
                if self.segs[i] != segs[i] {
                    same = false;
                }
                if self.segs[i].1 != segs[i].1 {
                    tmpl_changed = true;
                }
            }
            if same {
                return;
            }
            if tmpl_changed {
                self.seg_slots.iter_mut().for_each(|s| *s = 0); // compact toggle -> re-seed
            }
        } else {
            self.seg_slots = vec![0; segs.len()]; // segment set changed -> fresh slots
        }
        self.segs = segs;
        self.resize_to_content();
        self.render_buffer();
        self.invalidate();
    }

    fn left_text(&self) -> String {
        format!("{}  {}", self.name, self.time).trim().to_string()
    }

    // Between-segment separator: [gap] · [gap], same rhythm as the main "·".
    fn seg_gap(&self) -> i32 {
        self.s(6.0) * 3
    }

    /// Total tail width from the per-segment slots (grow-only; reset on font/DPI basis change).
    fn net_width(&mut self, f_main: &Font) -> i32 {
        if self.segs.is_empty() {
            return 0;
        }
        if theme::font_scale() != self.net_font_scale || theme::family() != self.net_family {
            self.net_font_scale = theme::font_scale();
            self.net_family = theme::family();
            self.seg_slots.iter_mut().for_each(|s| *s = 0);
        }
        let mut total = 0;
        for i in 0..self.segs.len() {
            if self.seg_slots[i] == 0 {
                let tmpl = self.segs[i].1.clone();
                self.seg_slots[i] = self.measure(&tmpl, f_main).ceil() as i32;
            }
            let text = self.segs[i].0.clone();
            let w = self.measure(&text, f_main).ceil() as i32;
            if w > self.seg_slots[i] {
                self.seg_slots[i] = w;
            }
            total += self.seg_slots[i] + if i > 0 { self.seg_gap() } else { 0 };
        }
        total
    }

    fn resize_to_content(&mut self) {
        let f_main = self.main_font();
        let f_count = self.count_font();
        let w_left = self.measure(&self.left_text(), &f_main).ceil() as i32;
        let w_count = self.measure(&self.count, &f_count).ceil() as i32;
        let w_net = self.net_width(&f_main);
        // [pad][dot][gap] left [gap] · [gap] count [ [gap] · [gap] net ] [pad]
        let tail = self.s(8.0)
            + self.s(6.0)
            + self.s(8.0)
            + w_count
            + if w_net > 0 { self.s(8.0) + self.s(6.0) + self.s(8.0) + w_net } else { 0 };
        self.w = (self.s(12.0) + self.s(8.0) + self.s(8.0) + w_left + tail + self.s(12.0))
            .max(self.s(110.0));
    }

    /// 1 s safety tick; hooks handle the instant cases.
    pub fn tick(&mut self) {
        self.update_visibility();
    }

    fn update_visibility(&mut self) {
        if self.hwnd.is_invalid() || self.paused {
            return;
        }
        if self.hide_on_fullscreen {
            if let Some(m) = displays::by_device(self.device.as_deref()) {
                if taskbar::is_fullscreen_app_on(&m.bounds) {
                    if !self.suppressed {
                        self.suppressed = true;
                        self.hide();
                    }
                    return;
                }
            }
        }
        if self.suppressed {
            self.suppressed = false;
            self.show();
        }
        self.sync_position();
    }

    pub fn screen_rect(&self) -> RECT {
        taskbar::window_rect(self.hwnd).unwrap_or_default()
    }

    pub fn visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Stop fighting modal dialogs: a topmost re-raise dismisses an open combo dropdown.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.update_visibility();
    }

    pub fn sync_position(&mut self) {
        if !self.paused {
            self.position_core(true);
        }
    }

    /// Settings live preview: bypasses the pause guard and never re-raises.
    pub fn preview_reposition(&mut self) {
        self.position_core(false);
    }

    fn position_core(&mut self, raise: bool) {
        let Some(screen) = displays::by_device(self.device.as_deref()) else { return };
        let tb = taskbar::taskbar_for_device(self.device.as_deref());
        let on_its_bar = tb
            .and_then(displays::from_window)
            .map(|m| m.device.eq_ignore_ascii_case(&screen.device))
            .unwrap_or(false);

        let prev_scale = self.scale;
        let scale_src = if on_its_bar { tb.unwrap() } else { self.hwnd };
        self.scale = taskbar::scale_of(scale_src);
        if self.scale <= 0.0 {
            self.scale = 1.0;
        }
        if self.scale != prev_scale {
            self.seg_slots.iter_mut().for_each(|s| *s = 0); // slots are DPI-dependent too
            self.resize_to_content();
            self.render_buffer();
            self.invalidate();
        }

        let (strip, right_edge, left_edge) = if on_its_bar {
            let Some(r) = taskbar::window_rect(tb.unwrap()) else { return };
            if r.right <= r.left {
                return;
            }
            let tray_left = taskbar::tray_notify_left(tb.unwrap());
            let right = if tray_left > 0 { tray_left } else { r.right };
            (r, right, r.left)
        } else {
            // No taskbar on this monitor -> float at its bottom.
            let b = screen.bounds;
            let h = self.s(40.0);
            (RECT { left: b.left, top: b.bottom - h, right: b.right, bottom: b.bottom }, b.right, b.left)
        };

        let strip_h = strip.bottom - strip.top;
        self.h = self.s(32.0).min(strip_h - self.s(4.0));
        let y = strip.top + (strip_h - self.h) / 2;
        let x = if self.anchor_right {
            right_edge - self.w - self.s(self.offset as f32)
        } else {
            left_edge + self.s(self.offset as f32)
        };

        let target = RECT { left: x, top: y, right: x + self.w, bottom: y + self.h };
        if target == self.last_rect {
            if raise {
                taskbar::raise_topmost(self.hwnd);
            }
            return;
        }
        self.last_rect = target;
        unsafe {
            let _ = MoveWindow(self.hwnd, x, y, self.w, self.h, true);
        }
        if raise {
            taskbar::raise_topmost(self.hwnd);
        }
        self.update_region();
    }

    fn update_region(&self) {
        let r = self.s(8.0);
        unsafe {
            let rgn = CreateRoundRectRgn(0, 0, self.w + 1, self.h + 1, r * 2, r * 2);
            let _ = SetWindowRgn(self.hwnd, Some(rgn), true); // window takes ownership
        }
    }

    fn invalidate(&self) {
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
        }
    }

    pub fn rerender(&mut self) {
        self.resize_to_content();
        self.render_buffer();
        self.invalidate();
    }

    fn render_buffer(&mut self) {
        let pal = theme::current();
        let bmp = Bitmap::new(self.w.max(1), self.h.max(1));
        {
            let g = Graphics::from_bitmap(&bmp);
            g.clear(if self.hover { pal.bg_hover } else { pal.bg });

            let cy = (self.h / 2) as f32;
            let dot_d = self.s(8.0) as f32;
            let f_main = self.main_font();
            let f_count = self.count_font();
            let left = self.left_text();
            let wf = self.w as f32;
            let hf = self.h as f32;
            let w_net = self.net_width(&f_main);

            let accent = SolidBrush::new(pal.accent);
            let text = SolidBrush::new(pal.text);
            let dim = SolidBrush::new(pal.text_dim);
            let good = SolidBrush::new(pal.good);

            let rect = |x: f32, w: f32| gdip::rectf(x, 0.0, w, hf);

            if i18n::is_rtl() {
                // Mirror: dot at the right, content flows right-to-left.
                g.fill_ellipse(&accent, wf - self.s(12.0) as f32 - dot_d, cy - dot_d / 2.0, dot_d, dot_d);
                let far = StringFormat::new(gdip::ALIGN_FAR, gdip::ALIGN_CENTER);
                let mut xr = wf - self.s(12.0) as f32 - dot_d - self.s(8.0) as f32;
                g.draw_string(&left, &f_main, &text, rect(0.0, xr), &far);
                xr -= self.measure(&left, &f_main) + self.s(8.0) as f32;
                g.draw_string("·", &f_main, &dim, rect(0.0, xr), &far);
                xr -= (self.s(6.0) + self.s(8.0)) as f32;
                g.draw_string(&self.count, &f_count, &good, rect(0.0, xr), &far);
                if !self.segs.is_empty() {
                    // Mirrored tail: block at the left pad, segments left-aligned, "·" at its end.
                    let near = StringFormat::new(gdip::ALIGN_NEAR, gdip::ALIGN_CENTER);
                    let mut xl = self.s(12.0) as f32;
                    for i in (0..self.segs.len()).rev() {
                        g.draw_string(&self.segs[i].0, &f_main, &dim, rect(xl, wf - xl), &near);
                        if i > 0 {
                            let sx = xl + self.seg_slots[i] as f32 + self.s(6.0) as f32;
                            g.draw_string("·", &f_main, &dim, rect(sx, self.s(6.0) as f32), &near);
                        }
                        xl += (self.seg_slots[i] + self.seg_gap()) as f32;
                    }
                    let ex = self.s(12.0) as f32 + w_net as f32 + self.s(8.0) as f32;
                    g.draw_string("·", &f_main, &dim, rect(ex, self.s(6.0) as f32), &near);
                }
            } else {
                g.fill_ellipse(&accent, self.s(12.0) as f32, cy - dot_d / 2.0, dot_d, dot_d);
                let sf = StringFormat::new(gdip::ALIGN_NEAR, gdip::ALIGN_CENTER);
                let mut x = (self.s(12.0) + self.s(8.0)) as f32 + dot_d;
                g.draw_string(&left, &f_main, &text, rect(x, wf), &sf);
                x += self.measure(&left, &f_main) + self.s(8.0) as f32;
                g.draw_string("·", &f_main, &dim, rect(x, self.s(6.0) as f32), &sf);
                x += (self.s(6.0) + self.s(8.0)) as f32;
                g.draw_string(&self.count, &f_count, &good, rect(x, wf), &sf);
                if !self.segs.is_empty() {
                    // "·" at the block's static start; segments right-aligned in slots (units anchored).
                    let dot_x = wf - (self.s(12.0) + self.s(8.0) + self.s(6.0)) as f32 - w_net as f32;
                    g.draw_string("·", &f_main, &dim, rect(dot_x, self.s(6.0) as f32), &sf);
                    let far_sf = StringFormat::new(gdip::ALIGN_FAR, gdip::ALIGN_CENTER);
                    let mut xr = wf - self.s(12.0) as f32;
                    for i in (0..self.segs.len()).rev() {
                        g.draw_string(&self.segs[i].0, &f_main, &dim, rect(0.0, xr), &far_sf);
                        if i > 0 {
                            let sx = xr - self.seg_slots[i] as f32 - self.s(12.0) as f32;
                            g.draw_string("·", &f_main, &dim, rect(sx, self.s(6.0) as f32), &sf);
                        }
                        xr -= (self.seg_slots[i] + self.seg_gap()) as f32;
                    }
                }
            }
        }
        self.buffer = Some(bmp);
    }

    fn paint(&mut self) {
        let mut ps = PAINTSTRUCT::default();
        let hdc = unsafe { BeginPaint(self.hwnd, &mut ps) };
        if let Some(buf) = &self.buffer {
            let g = Graphics::from_hdc(hdc);
            g.draw_bitmap(buf, 0, 0);
        }
        unsafe {
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn track_leave(&self) {
        let mut tme = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: self.hwnd,
            dwHoverTime: 0,
        };
        unsafe {
            let _ = TrackMouseEvent(&mut tme);
        }
    }
}

impl WindowHandler for Widget {
    fn message(&mut self, _hwnd: HWND, msg: u32, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_ERASEBKGND => Some(LRESULT(1)),
            WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_PAINT => {
                self.paint();
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE => {
                if !self.tracking {
                    self.track_leave();
                    self.tracking = true;
                }
                if !self.hover {
                    self.hover = true;
                    self.render_buffer();
                    self.invalidate();
                }
                None
            }
            WM_MOUSELEAVE => {
                self.tracking = false;
                if self.hover {
                    self.hover = false;
                    self.render_buffer();
                    self.invalidate();
                }
                None
            }
            WM_LBUTTONUP => {
                unsafe {
                    let _ = PostMessageW(Some(self.app_hwnd), WM_WIDGET_CLICK, WPARAM(0), LPARAM(0));
                }
                None
            }
            WM_RBUTTONUP => {
                let mut pt = Default::default();
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                    let packed = ((pt.y as isize) << 32) | (pt.x as isize & 0xFFFF_FFFF);
                    let _ = PostMessageW(Some(self.app_hwnd), WM_WIDGET_MENU, WPARAM(0), LPARAM(packed));
                }
                None
            }
            WM_RAISE => {
                if !self.paused {
                    let now = tick_count();
                    if now.wrapping_sub(self.last_raise) >= 16 {
                        self.last_raise = now;
                        self.update_visibility();
                    }
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

impl Drop for Widget {
    fn drop(&mut self) {
        HOOK_TARGET.store(0, Ordering::Relaxed);
        unsafe {
            if !self.fg_hook.is_invalid() {
                let _ = UnhookWinEvent(self.fg_hook);
            }
            if !self.reorder_hook.is_invalid() {
                let _ = UnhookWinEvent(self.reorder_hook);
            }
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}
