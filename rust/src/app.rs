//! App orchestrator, port of App/AppHost.cs: hidden top-level window owning the tray icon,
//! timers, prayer computation, and the notification engine. The window is NOT message-only —
//! WM_POWERBROADCAST/WM_TIMECHANGE/WM_SETTINGCHANGE and the TaskbarCreated broadcast are
//! only delivered to real top-level windows.

use crate::calc::praytimes::{self, KEYS};
use crate::calc::events;
use crate::config::{AppConfig, AppState};
use crate::datetime::Date;
use crate::i18n;
use crate::native::{displays, net, quiet, startup, time};
use crate::services::update::UpdateInfo;
use crate::services::{audio, toast, update};
use crate::services::data_usage::{self, DataUsage};
use crate::services::latency::{self, Latency};
use crate::services::net_speed::{self, NetSpeed};
use crate::services::sys_meters::{self, SysMeters};
use crate::services::location::DetectedLocation;
use crate::ui::icon::{TrayIcon, WM_TRAY};
use crate::ui::popup::{Popup, Row, WM_POPUP_MOVED, WM_POPUP_PIN};
use crate::ui::settings::{self, SettingsHost, SettingsResult};
use crate::ui::theme;
use crate::ui::widget::{Widget, WM_WIDGET_CLICK, WM_WIDGET_MENU, WM_WIDGET_MOVED};
use crate::ui::window::{self, WindowHandler};
use std::collections::HashSet;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, KillTimer, PostMessageW, PostQuitMessage,
    RegisterWindowMessageW, SetForegroundWindow, SetTimer, TrackPopupMenuEx,
    MF_CHECKED, MF_SEPARATOR, MF_STRING, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, TPMPARAMS,
    TPM_LAYOUTRTL, TPM_RIGHTBUTTON, TPM_VERTICAL, WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_POWERBROADCAST,
    WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMECHANGE, WM_TIMER, WINDOW_EX_STYLE, WS_POPUP,
};

const ORDER: [&str; 6] = ["fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha"];

/// Cap on holding a muted azan when no later prayer bounds it (Isha).
const MUTED_AZAN_MAX_HOLD: i64 = 3 * 3600;

const TIMER_POS: usize = 1; // 1 s: reposition/seconds countdown
const TIMER_DATA: usize = 2; // 15 s: render + notification checks

/// Seconds each meter holds the shared tail slot when rotation is on.
const ROTATE_SECS: u32 = 4;

/// The countdown starts warming this long before the next prayer, and is fully urgent at it.
const URGENT_SECS: i64 = 15 * 60;

const CMD_SHOW_TIMES: usize = 1001;
const CMD_REFRESH: usize = 1002;
const CMD_STARTUP: usize = 1003;
const CMD_SETTINGS: usize = 1004;
const CMD_STOP_SOUND: usize = 1005;
const CMD_CHECK_UPDATES: usize = 1006;
const CMD_DATA_USAGE: usize = 1007;
const CMD_LOCK: usize = 1009;
const CMD_EXIT: usize = 1008;

/// Update-check result; lparam = Box<Option<UpdateInfo>> raw.
const WM_UPDATE_RESULT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 6;
/// Download finished; wparam = ok, lparam = Box<UpdateInfo> raw.
const WM_UPDATE_DOWNLOADED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 7;
/// First-run detection finished; lparam = Box<Option<DetectedLocation>> raw.
const WM_FIRSTRUN_LOC: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 8;

fn tick_count() -> u32 {
    unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
}

// Single-instance mutex, held by main; released right before spawning an updated exe
// (the child's mutex check must not see our guard).
static INSTANCE_MUTEX: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

pub fn set_instance_mutex(handle: isize) {
    INSTANCE_MUTEX.store(handle, std::sync::atomic::Ordering::Relaxed);
}

fn release_single_instance() {
    let h = INSTANCE_MUTEX.swap(0, std::sync::atomic::Ordering::Relaxed);
    if h != 0 {
        unsafe {
            use windows::Win32::Foundation::{CloseHandle, HANDLE};
            let handle = HANDLE(h as *mut core::ffi::c_void);
            let _ = windows::Win32::System::Threading::ReleaseMutex(handle);
            let _ = CloseHandle(handle);
        }
    }
}

/// Absolute local time in seconds (Rata-Die day * 86400 + seconds of day).
fn abs_now() -> i64 {
    let (d, min, sec) = time::now_local();
    d.to_rd() * 86400 + min as i64 * 60 + sec as i64
}

fn abs_of(date: Date, minutes: u16) -> i64 {
    date.to_rd() * 86400 + minutes as i64 * 60
}

/// An azan that was swallowed because the user was busy; surfaces once they're free.
struct MutedAzan {
    label: String,
    time_s: String,
    expires: i64,
}

pub struct App {
    pub cfg: AppConfig,
    state: AppState,
    hwnd: HWND,
    tray: Option<TrayIcon>,
    widget: Option<Box<Widget>>,
    popup: Option<Box<Popup>>,
    popup_hidden_at: u32,
    times: [u16; 6],
    times_date: Date,
    fired: HashSet<String>,
    fired_date: Date,
    muted_azan: Option<MutedAzan>,
    next_at: Option<i64>,
    taskbar_created_msg: u32,
    update_busy: bool,
    settings_open: bool,
    net_speed: NetSpeed,
    latency: Latency,
    data_usage: DataUsage,
    sys: SysMeters,
    usage_open: bool,
    rot: u32,
}

fn head_mode(cfg: &AppConfig) -> u8 {
    crate::config::HEAD_LAYOUTS.iter().position(|h| *h == cfg.head_layout).unwrap_or(1) as u8
}

impl App {
    pub fn run() {
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let cfg = AppConfig::load();
        i18n::set(&cfg.language);
        startup::heal_path();
        toast::init();
        theme::apply(&cfg.theme, &cfg.font_family, cfg.font_scale());

        let app = Box::new(Self {
            cfg,
            state: AppState::load(),
            hwnd: HWND::default(),
            tray: None,
            widget: None,
            popup: None,
            popup_hidden_at: 0,
            times: [0; 6],
            times_date: Date::new(1, 1, 1),
            fired: HashSet::new(),
            fired_date: Date::new(1, 1, 1),
            muted_azan: None,
            next_at: None,
            taskbar_created_msg: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            update_busy: false,
            settings_open: false,
            net_speed: NetSpeed::new(),
            latency: Latency::new(),
            data_usage: DataUsage::new(),
            sys: SysMeters::new(),
            usage_open: false,
            rot: 0,
        });
        let app = Box::leak(app); // lives for the process; freed by the OS at exit

        let hwnd = window::create(
            w!("PrayerTrayApp"),
            w!("PrayerTray"),
            WINDOW_EX_STYLE::default(),
            WS_POPUP, // top-level, never shown
            0,
            0,
            0,
            0,
            None,
            app as *mut Self,
        )
        .expect("app window");
        app.hwnd = hwnd;
        app.tray = Some(TrayIcon::new(hwnd));
        app.build_widget();
        let mut popup = Popup::new(hwnd);
        popup.init_pin(app.cfg.popup_pinned, app.cfg.popup_x, app.cfg.popup_y);
        app.popup = Some(popup);

        app.recompute();
        app.data_usage.load();
        app.apply_net_config();
        app.data_tick();
        if app.cfg.popup_pinned {
            app.show_popup();
        }
        unsafe {
            SetTimer(Some(hwnd), TIMER_POS, 1_000, None);
            SetTimer(Some(hwnd), TIMER_DATA, 15_000, None);
        }

        // First run: detect off-thread, then open Settings prefilled (port of FirstRunDetect).
        if AppConfig::is_first_run() {
            let target = hwnd.0 as isize;
            std::thread::spawn(move || {
                let loc = Box::new(crate::services::location::detect());
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(target as *mut _)),
                        WM_FIRSTRUN_LOC,
                        WPARAM(0),
                        LPARAM(Box::into_raw(loc) as isize),
                    );
                }
            });
        }

        // Message loop.
        use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, MSG};
        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn today(&self) -> Date {
        time::now_local().0
    }

    fn build_widget(&mut self) {
        self.widget = None; // drop the old window/hooks first
        let mut w = Widget::new(self.hwnd, self.cfg.monitor_device_name.clone());
        w.anchor_right = !self.cfg.widget_anchor.eq_ignore_ascii_case("Left");
        w.offset = self.cfg.widget_offset;
        w.hide_on_fullscreen = self.cfg.hide_on_fullscreen;
        w.locked = self.cfg.lock_widget;
        w.head = head_mode(&self.cfg);
        self.widget = Some(w);
    }

    fn recompute(&mut self) {
        let today = self.today();
        let tz = self.cfg.resolve_timezone(time::utc_offset_hours(today));
        self.times = praytimes::compute(
            today,
            self.cfg.latitude,
            self.cfg.longitude,
            tz,
            self.cfg.calc_method(),
            self.cfg.asr_juristic(),
            Some(&self.cfg.time_adjust()),
            self.cfg.high_lat(),
        );
        self.times_date = today;
    }

    fn ensure_today(&mut self) {
        if self.times_date != self.today() {
            self.recompute();
        }
    }

    /// Recompute and re-arm after a clock/timezone change, resume, or manual "Refresh now".
    fn refresh_now(&mut self) {
        self.recompute();
        self.fired.clear();
        self.fired_date = self.today();
        self.data_tick();
    }

    fn data_tick(&mut self) {
        self.render_tick();
        self.check_notifications();
    }

    fn pos_tick(&mut self) {
        if let Some(w) = &mut self.widget {
            w.tick();
        }
        self.rot = self.rot.wrapping_add(1);
        // One adapter snapshot feeds both the usage accumulator and the live meters.
        let tracking = self.cfg.track_data_usage;
        let meters = self.has_pill_meters();
        let rows = if tracking || meters { net::snapshot() } else { Vec::new() };
        if tracking {
            self.data_usage.tick(&rows);
        }
        if meters {
            self.update_net_segments(&rows);
        } else if let Some(w) = &mut self.widget {
            w.set_net(Vec::new(), Vec::new()); // clears the tail after meters off (idempotent)
        }
        if let Some(at) = self.next_at {
            let s = at - abs_now();
            if s > -60 && s <= 60 {
                self.render_tick();
            }
        }
    }

    fn has_pill_meters(&self) -> bool {
        self.cfg.show_net_speed
            || self.cfg.show_ping
            || self.cfg.show_sys_meters
            || self.cfg.show_vpn
            || (self.cfg.track_data_usage && self.cfg.show_data_usage)
    }

    /// Push the net-config down to the probe and clear the tail when no meter is shown.
    /// Port of AppHost.ApplyWidgetConfig's net section.
    fn apply_net_config(&mut self) {
        self.latency.set_host(&self.cfg.ping_host);
        if !self.has_pill_meters() {
            if let Some(w) = &mut self.widget {
                w.set_net(Vec::new(), Vec::new());
            }
        }
    }

    /// Build the tail segments with their worst-case templates.
    fn update_net_segments(&mut self, rows: &[net::IfRow]) {
        let wide = self.cfg.wide_meters;
        let stack = self.cfg.stack_pairs;
        let order = self.cfg.pill_order.clone();
        let mut segs: Vec<(String, String)> = Vec::with_capacity(6);
        let mut vpn_at: Option<usize> = None;
        for id in &order {
            match id.as_str() {
                "speed" if self.cfg.show_net_speed => {
                    let (down, up) = self.net_speed.sample(rows);
                    let (d, u) = net_speed::format_parts(down, up);
                    // Stacked, the pair costs one reading's width instead of two.
                    let (dt, ut) = if wide { ("↓ 88.8 MB/s", "↑ 88.8 MB/s") } else { ("↓ 8.8 MB/s", "↑ 8.8 MB/s") };
                    if stack {
                        segs.push((format!("{u}\n{d}"), format!("{ut}\n{dt}")));
                    } else {
                        segs.push((u, ut.into()));
                        segs.push((d, dt.into()));
                    }
                }
                "ping" if self.cfg.show_ping => {
                    let ms = self.latency.sample();
                    segs.push((latency::format(ms), if wide { "888 ms" } else { "88 ms" }.into()));
                }
                "sys" if self.cfg.show_sys_meters => {
                    let cpu = sys_meters::format_cpu(self.sys.cpu_percent());
                    let ram = sys_meters::format_ram(sys_meters::memory_percent());
                    match self.cfg.sys_metric.as_str() {
                        "cpu" => segs.push((cpu, "CPU 100%".into())),
                        "ram" => segs.push((ram, "RAM 100%".into())),
                        _ if stack => {
                            segs.push((format!("{cpu}\n{ram}"), "CPU 100%\nRAM 100%".into()));
                        }
                        _ => {
                            segs.push((cpu, "CPU 100%".into()));
                            segs.push((ram, "RAM 100%".into()));
                        }
                    }
                }
                "usage" if self.cfg.track_data_usage && self.cfg.show_data_usage => {
                    let (dr, dt) = self.data_usage.today();
                    let (mr, mt) = self.data_usage.cycle(self.cfg.usage_cycle_day);
                    let (day, month) = (data_usage::size(dr + dt), data_usage::size(mr + mt));
                    let one = if wide { "Σ 8.88 GB" } else { "Σ 888 MB" };
                    segs.push(match self.cfg.usage_period.as_str() {
                        "month" => (format!("Σ {month}"), one.into()),
                        // Both periods only tell apart by their prefix, so they carry one.
                        "both" if stack => (format!("Σd {day}\nΣm {month}"), format!("{one}d\n{one}m")),
                        "both" => (format!("Σd {day} · Σm {month}"), format!("{one}d · {one}m")),
                        _ => (format!("Σ {day}"), one.into()),
                    });
                }
                "vpn" if self.cfg.show_vpn && net::default_route_is_tunnel(rows) => {
                    vpn_at = Some(segs.len());
                    segs.push(("VPN".into(), "VPN".into()));
                }
                _ => {}
            }
        }
        // The widget measures these to size the shared slot; picking a winner here would mean
        // guessing rendered width from a string, which is what the v2.2.1 slot work removed.
        let mut pool = Vec::new();
        if self.cfg.rotate_meters {
            // The indicator keeps its place rather than spending a turn, so it sits out the
            // rotation and rejoins at the end.
            let vpn = vpn_at.map(|i| segs.remove(i));
            if segs.len() > 1 {
                pool = segs.iter().map(|s| s.1.clone()).collect();
                segs = vec![Self::rotated(&segs, self.rot)];
            }
            segs.extend(vpn);
        }
        if let Some(w) = &mut self.widget {
            w.set_net(segs, pool);
        }
    }

    /// Whichever meter currently holds the shared slot.
    fn rotated(segs: &[(String, String)], rot: u32) -> (String, String) {
        segs[(rot / ROTATE_SECS) as usize % segs.len()].clone()
    }

    fn show_data_usage(&mut self) {
        if self.usage_open {
            return;
        }
        self.usage_open = true;
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_POS);
            let _ = KillTimer(Some(self.hwnd), TIMER_DATA);
        }
        // Raw pointer, not &mut: the dialog's nested loop can re-enter the app (a tray click
        // opens the popup, which reads data_usage) — holding a &mut across it would alias.
        let usage: *mut DataUsage = &mut self.data_usage;
        crate::ui::usage::show(self.hwnd, usage);
        unsafe {
            SetTimer(Some(self.hwnd), TIMER_POS, 1_000, None);
            SetTimer(Some(self.hwnd), TIMER_DATA, 15_000, None);
        }
        self.data_tick();
        self.usage_open = false;
    }

    fn time_of(&self, key: &str) -> Option<u16> {
        KEYS.iter().position(|k| *k == key).map(|i| self.times[i])
    }

    /// Formats minutes-since-midnight per the C# PrayerPopup.Format.
    pub fn format_time(minutes: u16, use24: bool) -> String {
        let (h, m) = (minutes / 60, minutes % 60);
        if use24 {
            format!("{:02}:{:02}", h, m)
        } else {
            let h12 = match h % 12 {
                0 => 12,
                x => x,
            };
            format!("{}:{:02} {}", h12, m, i18n::am_pm(h as u32))
        }
    }

    fn format_countdown(secs: i64) -> String {
        if secs < 60 {
            format!("{}{}", secs.max(0), i18n::t("unit.s"))
        } else if secs < 3600 {
            format!("{}{}", secs / 60, i18n::t("unit.m"))
        } else {
            format!("{}:{:02}", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// (key, target abs-seconds, countdown-string) — "now" during the prayer's own minute,
    /// else the plain countdown. Port of CurrentOrNext.
    fn current_or_next(&mut self) -> (Option<&'static str>, Option<i64>, String) {
        self.ensure_today();
        let now = abs_now();
        let today = self.today();
        for key in ORDER {
            if key == "sunrise" {
                continue;
            }
            let Some(ts) = self.time_of(key) else { continue };
            let at = abs_of(today, ts);
            if now < at {
                return (Some(key_static(key)), Some(at), Self::format_countdown(at - now));
            }
            if now < at + 60 {
                return (Some(key_static(key)), Some(at), "now".into());
            }
        }
        // All done today -> tomorrow's fajr.
        let tomorrow = today.add_days(1);
        let tz = self.cfg.resolve_timezone(time::utc_offset_hours(tomorrow));
        let t = praytimes::compute(
            tomorrow,
            self.cfg.latitude,
            self.cfg.longitude,
            tz,
            self.cfg.calc_method(),
            self.cfg.asr_juristic(),
            Some(&self.cfg.time_adjust()),
            self.cfg.high_lat(),
        );
        let fajr_at = abs_of(tomorrow, t[0]);
        (Some("fajr"), Some(fajr_at), Self::format_countdown(fajr_at - now))
    }

    /// Map the internal "now" sentinel to localized display text.
    fn shown_countdown(countdown: &str) -> String {
        if countdown == "now" {
            i18n::t("countdown.now").to_string()
        } else {
            countdown.to_string()
        }
    }

    /// Display-only refresh (no notifications).
    fn render_tick(&mut self) {
        let use24 = self.cfg.use24_hour;
        let (key, at, countdown) = self.current_or_next();
        self.next_at = at;
        let label = key.map(i18n::prayer).unwrap_or("?");
        // The countdown runs to the next prayer, which is also when the current window shuts.
        let urgency = at
            .map(|t| 1.0 - ((t - abs_now()).clamp(0, URGENT_SECS) as f32 / URGENT_SECS as f32))
            .unwrap_or(0.0);
        let time_str = key
            .and_then(|k| self.time_of(k))
            .map(|ts| Self::format_time(ts, use24))
            .unwrap_or_default();

        let tip = if countdown == "now" {
            format!("{} {} {}", i18n::t("tray.now"), label, time_str)
        } else {
            format!(
                "{} {} {} ({} {})",
                i18n::t("tray.next"),
                label,
                time_str,
                i18n::t("tray.in"),
                countdown
            )
        };
        if let Some(tray) = &mut self.tray {
            tray.set_tooltip(&tip);
        }
        let shown = Self::shown_countdown(&countdown);
        if let Some(w) = &mut self.widget {
            w.set_data(label, &time_str, &shown, urgency);
        }
        if self.popup.as_ref().map(|p| p.visible()).unwrap_or(false) {
            self.show_popup();
        }
    }

    /// Prefer a rich Action Center toast; fall back to a tray balloon.
    fn notify(&mut self, title: &str, body: &str) {
        self.notify_ex(title, body, false);
    }

    /// `silent` suppresses the balloon's system ding; toasts are already silent by XML.
    fn notify_ex(&mut self, title: &str, body: &str, silent: bool) {
        if self.cfg.rich_toasts && toast::show(title, body) {
            return;
        }
        if let Some(tray) = &mut self.tray {
            tray.balloon(title, body, silent);
        }
    }

    /// The file the configured azan would play, or None when nothing is set up.
    fn azan_path(&self) -> Option<std::path::PathBuf> {
        let mode = if self.cfg.azan_mode == "Builtin" {
            audio::BUILTIN_ADHANS[0].0 // legacy config -> first builtin
        } else {
            self.cfg.azan_mode.as_str()
        };
        match mode {
            "None" | "" => None,
            "Custom" => self
                .cfg
                .azan_custom_path
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .map(std::path::PathBuf::from),
            id => audio::builtin_adhan_path(id),
        }
    }

    fn play_azan(&self) {
        if let Some(p) = self.azan_path() {
            audio::play(&p);
        }
    }

    /// None = free to make noise.
    fn busy(&self) -> Option<quiet::Busy> {
        self.cfg.mute_when_busy.then(quiet::state).flatten()
    }

    fn next_prayer_after(&self, date: Date, after: i64) -> Option<i64> {
        ORDER
            .iter()
            .filter(|k| **k != "sunrise")
            .filter_map(|k| self.time_of(k))
            .map(|ts| abs_of(date, ts))
            .filter(|t| *t > after)
            .min()
    }

    fn fired_key(&self, date: Date, tail: &str) -> String {
        format!("{:04}{:02}{:02}:{}", date.year, date.month, date.day, tail)
    }

    /// Edge-triggered, day-aware: each reminder/azan fires once, within ~2 min of its minute.
    fn check_notifications(&mut self) {
        let today = self.today();
        let now = abs_now();
        if self.fired_date != today {
            self.fired.clear();
            self.fired_date = today;
        }

        let in_window = |target: i64| {
            let s = now - target;
            (0..120).contains(&s)
        };

        for key in ORDER {
            if key == "sunrise" {
                continue;
            }
            let Some(ts) = self.time_of(key) else { continue };
            let at = abs_of(today, ts);
            let label = i18n::prayer(key);

            if self.cfg.reminder_enabled && in_window(at - self.cfg.reminder_minutes as i64 * 60) {
                let id = self.fired_key(today, &format!("{key}:rem"));
                if self.fired.insert(id) {
                    let mins = self.cfg.reminder_minutes.to_string();
                    let time_s = Self::format_time(ts, self.cfg.use24_hour);
                    let body = i18n::f("balloon.reminderBody", &[label, &mins, &time_s]);
                    let busy = self.busy();
                    self.notify_ex(i18n::t("balloon.reminderTitle"), &body, busy.is_some());
                    if self.cfg.reminder_sound && busy.is_none() {
                        audio::play_reminder(&self.cfg);
                    }
                }
            }

            if in_window(at) {
                let id = self.fired_key(today, key);
                if self.fired.insert(id) {
                    let time_s = Self::format_time(ts, self.cfg.use24_hour);
                    let body = i18n::f("balloon.timeBody", &[label, &time_s]);
                    let busy = self.busy();
                    self.notify_ex(i18n::t("balloon.timeTitle"), &body, busy.is_some());
                    self.muted_azan = None; // this prayer supersedes any older muted one
                    if self.azan_path().is_some() {
                        if busy.is_some() {
                            self.muted_azan = Some(MutedAzan {
                                label: label.to_string(),
                                time_s,
                                expires: self
                                    .next_prayer_after(today, at)
                                    .unwrap_or(at + MUTED_AZAN_MAX_HOLD),
                            });
                        } else {
                            self.play_azan();
                        }
                    }
                }
            }
        }

        // Catch-up line once the user is free; never a late replay of the adhan itself.
        if let Some(m) = self.muted_azan.take() {
            if now < m.expires {
                if self.busy().is_some() {
                    self.muted_azan = Some(m);
                } else {
                    let body = i18n::f("balloon.mutedBody", &[&m.label, &m.time_s]);
                    self.notify(i18n::t("balloon.mutedTitle"), &body);
                }
            }
        }

        // Eve-before nudge for tomorrow's Sunnah fast (persisted latch, fires once from Maghrib on).
        if self.cfg.sunnah_fast_reminder {
            if let Some(mts) = self.time_of("maghrib") {
                let date_s = format!("{:04}-{:02}-{:02}", today.year, today.month, today.day);
                if now >= abs_of(today, mts) && self.state.sunnah_fast_noticed != date_s {
                    if let Some(reason) = self.sunnah_fast_reason(today.add_days(1)) {
                        self.state.sunnah_fast_noticed = date_s;
                        self.state.save();
                        let name = Self::fast_reason_name(reason);
                        let body = i18n::f("balloon.fastBody", &[&name]);
                        self.notify(i18n::t("balloon.fastTitle"), &body);
                    }
                }
            }
        }

        // Friday nudges: Surah Al-Kahf at Fajr, Jumu'ah heads-up before Dhuhr.
        const JUMUAH_LEAD_MIN: i64 = 30;
        if self.cfg.friday_reminder && today.weekday() == 5 {
            if let Some(fts) = self.time_of("fajr") {
                if in_window(abs_of(today, fts)) {
                    let id = self.fired_key(today, "kahf");
                    if self.fired.insert(id) {
                        self.notify(i18n::t("balloon.jumuahTitle"), i18n::t("balloon.kahfBody"));
                    }
                }
            }
            if let Some(dts) = self.time_of("dhuhr") {
                if in_window(abs_of(today, dts) - JUMUAH_LEAD_MIN * 60) {
                    let id = self.fired_key(today, "jumuah");
                    if self.fired.insert(id) {
                        self.notify(i18n::t("balloon.jumuahTitle"), i18n::t("balloon.jumuahBody"));
                    }
                }
            }
        }
    }

    /// Why `day` is a Sunnah fasting day (or None). Fixed days win; skips forbidden days.
    fn sunnah_fast_reason(&self, day: Date) -> Option<&'static str> {
        let ev = events::for_date(day, self.cfg.hijri_adjust);
        match ev {
            Some("eidFitr") | Some("eidAdha") | Some("tashreeq") => return None, // fasting forbidden
            Some(e @ ("arafah" | "ashura" | "whiteDays")) => return Some(e),
            _ => {}
        }
        match day.weekday() {
            1 => Some("monday"),
            4 => Some("thursday"),
            _ => None,
        }
    }

    fn fast_reason_name(r: &str) -> String {
        match r {
            "monday" => i18n::t("fast.monday").to_string(),
            "thursday" => i18n::t("fast.thursday").to_string(),
            _ => i18n::event(r).to_string(),
        }
    }

    fn show_menu(&mut self) {
        // A modal dialog runs its own message loop that still dispatches tray clicks; suppress the
        // menu so it can't toggle/teardown state the open dialog is displaying.
        if self.settings_open || self.usage_open {
            return;
        }
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let add = |flags, id: usize, text: &str| {
                let wide = window::utf16z(text);
                let _ = AppendMenuW(menu, flags, id, windows::core::PCWSTR(wide.as_ptr()));
            };
            add(MF_STRING, CMD_SHOW_TIMES, i18n::t("menu.showTimes"));
            add(MF_STRING, CMD_REFRESH, i18n::t("menu.refresh"));
            add(MF_SEPARATOR, 0, "");
            add(
                if startup::is_enabled() { MF_STRING | MF_CHECKED } else { MF_STRING },
                CMD_STARTUP,
                i18n::t("menu.startup"),
            );
            add(
                if self.cfg.lock_widget { MF_STRING | MF_CHECKED } else { MF_STRING },
                CMD_LOCK,
                i18n::t("menu.lockWidget"),
            );
            add(MF_STRING, CMD_SETTINGS, i18n::t("menu.settings"));
            add(MF_STRING, CMD_STOP_SOUND, i18n::t("menu.stopSound"));
            add(MF_STRING, CMD_CHECK_UPDATES, i18n::t("menu.checkUpdates"));
            add(MF_STRING, CMD_DATA_USAGE, i18n::t("menu.dataUsage"));
            add(MF_SEPARATOR, 0, "");
            add(MF_STRING, CMD_EXIT, i18n::t("menu.exit"));

            let mut pt = Default::default();
            let _ = GetCursorPos(&mut pt);
            // Required for the menu to dismiss when clicking elsewhere (classic tray-menu quirk).
            let _ = SetForegroundWindow(self.hwnd);
            // Exclude the taskbar strip: menus clip to the monitor (not work area), and the
            // taskbar z-band would hide any rows placed under it.
            let point_rect = RECT { left: pt.x, top: pt.y, right: pt.x, bottom: pt.y };
            let exclude = displays::at_point(pt)
                .map(|m| {
                    let (b, w) = (m.bounds, m.work);
                    if w.bottom < b.bottom {
                        RECT { left: b.left, top: w.bottom, right: b.right, bottom: b.bottom }
                    } else if w.top > b.top {
                        RECT { left: b.left, top: b.top, right: b.right, bottom: w.top }
                    } else if w.left > b.left {
                        RECT { left: b.left, top: b.top, right: w.left, bottom: b.bottom }
                    } else if w.right < b.right {
                        RECT { left: w.right, top: b.top, right: b.right, bottom: b.bottom }
                    } else {
                        point_rect
                    }
                })
                .unwrap_or(point_rect);
            let params = TPMPARAMS { cbSize: std::mem::size_of::<TPMPARAMS>() as u32, rcExclude: exclude };
            let rtl = if i18n::is_rtl() { TPM_LAYOUTRTL } else { Default::default() };
            let _ = TrackPopupMenuEx(
                menu,
                (TPM_RIGHTBUTTON | TPM_VERTICAL | rtl).0,
                pt.x,
                pt.y,
                self.hwnd,
                Some(&params),
            );
            let _ = DestroyMenu(menu);
        }
    }

    fn toggle_popup(&mut self) {
        let visible = self.popup.as_ref().map(|p| p.visible()).unwrap_or(false);
        if visible {
            if let Some(p) = &self.popup {
                p.hide();
            }
            self.popup_hidden_at = tick_count();
        } else if tick_count().wrapping_sub(self.popup_hidden_at) > 250 {
            self.show_popup();
        }
    }

    fn show_popup(&mut self) {
        self.ensure_today();
        let (next_key, _, countdown) = self.current_or_next();
        let use24 = self.cfg.use24_hour;
        let today = self.today();

        let mut rows = Vec::with_capacity(6);
        for (i, key) in KEYS.iter().enumerate() {
            rows.push(Row {
                label: i18n::prayer(key).to_string(),
                time: Self::format_time(self.times[i], use24),
                is_next: Some(*key) == next_key,
                is_sunrise: *key == "sunrise",
            });
        }

        let hijri = if self.cfg.show_hijri_date {
            i18n::format_hijri(today, self.cfg.hijri_adjust)
        } else {
            String::new()
        };
        let event = self.todays_event();
        let fast = self.todays_fast();
        let usage = if self.cfg.track_data_usage {
            let (rx, tx) = self.data_usage.today();
            i18n::f("usage.today", &[&data_usage::size(rx), &data_usage::size(tx)])
        } else {
            String::new()
        };
        let shown = Self::shown_countdown(&countdown);
        let (rect, anchor_right) = self
            .widget
            .as_ref()
            .map(|w| (w.screen_rect(), w.anchor_right))
            .unwrap_or_default();
        let city = self.cfg.city.clone();
        if let Some(p) = &mut self.popup {
            p.show_times(&city, today, rows, &shown, rect, anchor_right, &hijri, &event, &fast, &usage);
        }
    }

    /// Popup line: today's special day, else a countdown to the next major event (within ~6 weeks).
    fn todays_event(&self) -> String {
        if !self.cfg.show_islamic_events {
            return String::new();
        }
        let today = self.today();
        if let Some(ev) = events::for_date(today, self.cfg.hijri_adjust) {
            return i18n::event(ev).to_string();
        }
        if let Some((k, days)) = events::next_major(today, self.cfg.hijri_adjust) {
            if days <= 45 {
                return if days == 1 {
                    i18n::f("event.tomorrow", &[i18n::event(k)])
                } else {
                    i18n::f("event.inDays", &[i18n::event(k), &days.to_string()])
                };
            }
        }
        String::new()
    }

    /// Popup fast line: fast-day itself all day; else the eve notice from Maghrib onward.
    fn todays_fast(&self) -> String {
        if !self.cfg.sunnah_fast_reminder {
            return String::new();
        }
        let today = self.today();
        if let Some(r) = self.sunnah_fast_reason(today) {
            return i18n::f("fast.today", &[&Self::fast_reason_name(r)]);
        }
        if let Some(mts) = self.time_of("maghrib") {
            if abs_now() >= abs_of(today, mts) {
                if let Some(r) = self.sunnah_fast_reason(today.add_days(1)) {
                    return i18n::f("fast.tomorrow", &[&Self::fast_reason_name(r)]);
                }
            }
        }
        String::new()
    }

    fn command(&mut self, id: usize) {
        match id {
            CMD_SHOW_TIMES => self.toggle_popup(),
            CMD_REFRESH => self.refresh_now(),
            CMD_STARTUP => {
                if !startup::set_enabled(!startup::is_enabled()) {
                    let title = i18n::t("app.name").to_string();
                    self.notify(&title, i18n::t("msg.startupError"));
                }
            }
            CMD_LOCK => {
                self.cfg.lock_widget = !self.cfg.lock_widget;
                self.cfg.save();
                if let Some(w) = &mut self.widget {
                    w.locked = self.cfg.lock_widget;
                }
            }
            CMD_SETTINGS => self.run_settings(None),
            CMD_STOP_SOUND => audio::stop(),
            CMD_CHECK_UPDATES => self.check_updates(),
            CMD_DATA_USAGE => self.show_data_usage(),
            CMD_EXIT => self.exit(),
            _ => {}
        }
    }

    /// Modal settings with reopen-loop: a language change closes with Retry so the dialog
    /// rebuilds in the new language/RTL. The opening snapshot is carried across reopens so
    /// Cancel still reverts to the true original. Port of AppHost.RunSettings.
    fn run_settings(&mut self, mut prefill: Option<DetectedLocation>) {
        if self.settings_open {
            return;
        }
        self.settings_open = true;
        if let Some(w) = &mut self.widget {
            w.pause();
        }
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_POS);
            let _ = KillTimer(Some(self.hwnd), TIMER_DATA);
        }
        let orig = self.cfg.clone();
        loop {
            let taken = prefill.take();
            let result = settings::run(self, &orig, taken.as_ref());
            match result {
                SettingsResult::Retry => continue,
                SettingsResult::Ok => {
                    // Monitor change is the only thing live preview defers.
                    let current = self.widget.as_ref().and_then(|w| w.device_name()).unwrap_or("").to_string();
                    let wanted = self.cfg.monitor_device_name.clone().unwrap_or_default();
                    if !current.eq_ignore_ascii_case(&wanted) {
                        self.build_widget();
                    }
                    break;
                }
                SettingsResult::Cancel => break,
            }
        }
        unsafe {
            SetTimer(Some(self.hwnd), TIMER_POS, 1_000, None);
            SetTimer(Some(self.hwnd), TIMER_DATA, 15_000, None);
        }
        if let Some(w) = &mut self.widget {
            w.resume();
        }
        self.data_tick();
        self.settings_open = false;
    }

    fn check_updates(&mut self) {
        if self.update_busy {
            return;
        }
        self.update_busy = true;
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let info = Box::new(update::fetch_latest());
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_UPDATE_RESULT,
                    WPARAM(0),
                    LPARAM(Box::into_raw(info) as isize),
                );
            }
        });
    }

    fn msg_yes_no(&self, title: &str, body: &str) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDYES, MB_ICONINFORMATION, MB_RIGHT, MB_RTLREADING, MB_YESNO,
        };
        let t = window::utf16z(title);
        let b = window::utf16z(body);
        let style = MB_YESNO
            | MB_ICONINFORMATION
            | if i18n::is_rtl() { MB_RTLREADING | MB_RIGHT } else { Default::default() };
        unsafe {
            MessageBoxW(
                Some(self.hwnd),
                windows::core::PCWSTR(b.as_ptr()),
                windows::core::PCWSTR(t.as_ptr()),
                style,
            ) == IDYES
        }
    }

    fn open_url(url: &str) {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let u = window::utf16z(url);
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                windows::core::PCWSTR(u.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }

    fn on_update_result(&mut self, info: Option<UpdateInfo>) {
        self.update_busy = false;
        let title = i18n::t("app.name").to_string();
        let Some(info) = info else {
            self.notify(&title, i18n::t("update.error"));
            return;
        };
        if !update::is_newer(&info) {
            let (ma, mi, b) = update::current();
            let body = i18n::f("update.none", &[&format!("v{ma}.{mi}.{b}")]);
            self.notify(&title, &body);
            return;
        }
        let (ma, mi, b) = info.latest;
        let v = format!("v{ma}.{mi}.{b}");
        if !self.msg_yes_no(i18n::t("update.availableTitle"), &i18n::f("update.availableBody", &[&v])) {
            return;
        }
        let Some(asset) = info.asset_url.clone() else {
            Self::open_url(&info.url);
            return;
        };
        self.notify(&title, i18n::t("update.downloading"));
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let dest = std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|d| d.join("PrayerTray-new.exe")));
            let ok = dest.as_deref().map(|d| update::download(&asset, d)).unwrap_or(false);
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_UPDATE_DOWNLOADED,
                    WPARAM(ok as usize),
                    LPARAM(Box::into_raw(Box::new(info)) as isize),
                );
            }
        });
    }

    /// Swap the downloaded exe in and restart; any failure restores the original.
    fn on_update_downloaded(&mut self, ok: bool, info: UpdateInfo) {
        let title = i18n::t("app.name").to_string();
        let Some(exe) = std::env::current_exe().ok() else { return };
        let dest = exe.parent().map(|d| d.join("PrayerTray-new.exe")).unwrap_or_default();
        if !ok {
            Self::open_url(&info.url);
            return;
        }
        if !self.msg_yes_no(i18n::t("update.availableTitle"), i18n::t("update.restartAsk")) {
            let _ = std::fs::remove_file(&dest); // postponed -> discard; re-downloaded next check
            return;
        }
        let old = exe.with_extension("exe.old");
        let swap = (|| -> std::io::Result<()> {
            if old.exists() {
                std::fs::remove_file(&old)?;
            }
            std::fs::rename(&exe, &old)?;
            std::fs::rename(&dest, &exe)?;
            Ok(())
        })();
        if swap.is_err() {
            let _ = std::fs::remove_file(&dest);
            self.notify(&title, i18n::t("update.failed"));
            return;
        }
        // Spawn the new exe; undo the swap if that fails so the running process matches disk.
        if std::process::Command::new(&exe).spawn().is_ok() {
            release_single_instance();
            self.exit();
        } else {
            let _ = std::fs::rename(&exe, &dest);
            let _ = std::fs::rename(&old, &exe);
            let _ = std::fs::remove_file(&dest);
            self.notify(&title, i18n::t("update.failed"));
        }
    }

    fn exit(&mut self) {
        audio::stop();
        self.data_usage.flush();
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_POS);
            let _ = KillTimer(Some(self.hwnd), TIMER_DATA);
        }
        if let Some(tray) = &mut self.tray {
            tray.remove();
        }
        unsafe { PostQuitMessage(0) };
    }
}

fn key_static(key: &str) -> &'static str {
    ORDER.iter().find(|k| **k == key).copied().unwrap_or("fajr")
}

impl SettingsHost for App {
    fn cfg(&mut self) -> &mut AppConfig {
        &mut self.cfg
    }

    /// Re-apply the (mutated-in-place) config without raising windows. Port of AppHost.LivePreview.
    fn live_preview(&mut self) {
        theme::apply(&self.cfg.theme, &self.cfg.font_family, self.cfg.font_scale());
        if let Some(w) = &mut self.widget {
            w.anchor_right = !self.cfg.widget_anchor.eq_ignore_ascii_case("Left");
            w.offset = self.cfg.widget_offset;
            w.hide_on_fullscreen = self.cfg.hide_on_fullscreen;
            w.locked = self.cfg.lock_widget;
            w.head = head_mode(&self.cfg);
        }
        self.apply_net_config();
        self.recompute();
        self.render_tick(); // display only — never fire azan/balloon from a settings change
        if let Some(w) = &mut self.widget {
            w.preview_reposition();
        }
    }

    fn test_notify(&mut self, use_toast: bool) {
        let title = i18n::t("app.name");
        let body = i18n::t("toast.test");
        if use_toast && toast::show(title, body) {
            return;
        }
        if let Some(tray) = &mut self.tray {
            tray.balloon(title, body, false);
        }
    }
}

impl WindowHandler for App {
    fn message(&mut self, _hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_TIMER => {
                match wparam.0 {
                    TIMER_POS => self.pos_tick(),
                    TIMER_DATA => self.data_tick(),
                    _ => {}
                }
                Some(LRESULT(0))
            }
            WM_TRAY => {
                match lparam.0 as u32 {
                    WM_LBUTTONUP => self.toggle_popup(),
                    WM_RBUTTONUP => self.show_menu(),
                    _ => {}
                }
                Some(LRESULT(0))
            }
            WM_COMMAND => {
                self.command(wparam.0 & 0xFFFF);
                Some(LRESULT(0))
            }
            WM_TIMECHANGE => {
                self.refresh_now();
                Some(LRESULT(0))
            }
            WM_SETTINGCHANGE => {
                // Follow Windows' light/dark flip only when the user left the theme on "Auto".
                if lparam.0 != 0 {
                    let s = unsafe { windows::core::PCWSTR(lparam.0 as *const u16).to_string() }.unwrap_or_default();
                    if s == "ImmersiveColorSet" && self.cfg.theme.eq_ignore_ascii_case("Auto") {
                        theme::apply(&self.cfg.theme, &self.cfg.font_family, self.cfg.font_scale());
                        if let Some(w) = &mut self.widget {
                            w.rerender();
                        }
                        self.data_tick();
                    }
                }
                None
            }
            WM_POWERBROADCAST => {
                if wparam.0 as u32 == PBT_APMRESUMESUSPEND || wparam.0 as u32 == PBT_APMRESUMEAUTOMATIC {
                    self.refresh_now();
                }
                None // let DefWindowProc return TRUE for the broadcast
            }
            m if m == self.taskbar_created_msg && m != 0 => {
                if let Some(tray) = &mut self.tray {
                    tray.readd();
                }
                self.build_widget(); // Explorer restarted -> old owner window is gone
                self.data_tick();
                Some(LRESULT(0))
            }
            WM_WIDGET_CLICK => {
                self.toggle_popup();
                Some(LRESULT(0))
            }
            WM_WIDGET_MENU => {
                self.show_menu();
                Some(LRESULT(0))
            }
            WM_POPUP_PIN => {
                self.cfg.popup_pinned = wparam.0 != 0;
                self.cfg.save();
                Some(LRESULT(0))
            }
            WM_FIRSTRUN_LOC => {
                let loc = *unsafe { Box::from_raw(lparam.0 as *mut Option<DetectedLocation>) };
                self.run_settings(loc);
                Some(LRESULT(0))
            }
            WM_UPDATE_RESULT => {
                let info = *unsafe { Box::from_raw(lparam.0 as *mut Option<UpdateInfo>) };
                self.on_update_result(info);
                Some(LRESULT(0))
            }
            WM_UPDATE_DOWNLOADED => {
                let info = *unsafe { Box::from_raw(lparam.0 as *mut UpdateInfo) };
                self.on_update_downloaded(wparam.0 != 0, info);
                Some(LRESULT(0))
            }
            WM_WIDGET_MOVED => {
                self.cfg.widget_offset = (wparam.0 as i32).clamp(0, 2000);
                // Settings mutates cfg in place for live preview and only writes on Save, so
                // saving here would make Cancel unable to undo the rest of the dialog.
                if !self.settings_open {
                    self.cfg.save();
                }
                Some(LRESULT(0))
            }
            WM_POPUP_MOVED => {
                let packed = lparam.0 as i64;
                self.cfg.popup_x = (packed & 0xFFFF_FFFF) as i32;
                self.cfg.popup_y = (packed >> 32) as i32;
                self.cfg.save();
                Some(LRESULT(0))
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}
