//! App config + small runtime state, JSON-compatible with the C# build
//! (%APPDATA%\PrayerTray\config.json, PascalCase, same defaults and sentinels).

use crate::calc::praytimes::{method_by_key, AsrJuristic, CalcMethod, HighLatRule, Offsets};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn is_true(b: &bool) -> bool {
    *b
}

/// PopupX/PopupY unset sentinel (C# int.MinValue).
pub const POPUP_UNSET: i32 = i32::MIN;
/// TimezoneHours "use system timezone" sentinel.
pub const TZ_SYSTEM: f64 = 999.0;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppConfig {
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
    pub method: String,
    pub asr: i32,
    pub high_lats: String, // None | MidNight | OneSeventh | AngleBased
    // Per-prayer fine-tune in minutes; clamped -60..60 on save.
    pub fajr_adjust: i32,
    pub dhuhr_adjust: i32,
    pub asr_adjust: i32,
    pub maghrib_adjust: i32,
    pub isha_adjust: i32,
    pub use24_hour: bool,
    pub widget_anchor: String, // Left | Right
    pub widget_offset: i32,
    pub theme: String, // Auto | Dark | Light | Midnight | Slate | Warm
    pub monitor_device_name: Option<String>,
    pub hide_on_fullscreen: bool,
    // Net-meter fields (speed/ping/data-usage tail).
    pub show_net_speed: bool,
    pub show_ping: bool,
    pub ping_host: String,
    pub show_sys_meters: bool,
    pub compact_meters: bool,
    pub rotate_meters: bool,
    pub track_data_usage: bool,
    pub show_data_usage: bool,
    pub timezone_hours: f64,
    pub language: String, // auto | en | ar | fr | tr | ur | id
    pub show_hijri_date: bool,
    pub hijri_adjust: i32,
    pub show_islamic_events: bool,
    pub sunnah_fast_reminder: bool,
    pub friday_reminder: bool,
    pub font_family: String,
    pub font_scale_pct: i32,
    pub popup_pinned: bool,
    pub popup_x: i32,
    pub popup_y: i32,
    pub rich_toasts: bool,
    pub reminder_enabled: bool,
    pub reminder_minutes: i32,
    pub reminder_sound: bool,
    pub reminder_sound_id: String,
    pub reminder_sound_path: Option<String>,
    pub azan_mode: String, // None | <builtin id> | Custom
    pub azan_custom_path: Option<String>,
    // Rust-only, defaults on — only the opt-out is written, keeping configs C#-compatible.
    #[serde(skip_serializing_if = "is_true")]
    pub mute_when_busy: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            city: "Makkah".into(),
            latitude: 21.4225,
            longitude: 39.8262,
            method: "MWL".into(),
            asr: AsrJuristic::Standard as i32,
            high_lats: "AngleBased".into(),
            fajr_adjust: 0,
            dhuhr_adjust: 0,
            asr_adjust: 0,
            maghrib_adjust: 0,
            isha_adjust: 0,
            use24_hour: false,
            widget_anchor: "Right".into(),
            widget_offset: 12,
            theme: "Auto".into(),
            monitor_device_name: None,
            hide_on_fullscreen: true,
            show_net_speed: false,
            show_ping: false,
            ping_host: "1.1.1.1".into(),
            show_sys_meters: false,
            compact_meters: false,
            rotate_meters: true,
            track_data_usage: false,
            show_data_usage: false,
            timezone_hours: TZ_SYSTEM,
            language: "auto".into(),
            show_hijri_date: true,
            hijri_adjust: 0,
            show_islamic_events: true,
            sunnah_fast_reminder: false,
            friday_reminder: false,
            font_family: "Segoe UI".into(),
            font_scale_pct: 100,
            popup_pinned: false,
            popup_x: POPUP_UNSET,
            popup_y: POPUP_UNSET,
            rich_toasts: true,
            reminder_enabled: false,
            reminder_minutes: 10,
            reminder_sound: true,
            reminder_sound_id: "chime".into(),
            reminder_sound_path: None,
            azan_mode: "None".into(),
            azan_custom_path: None,
            mute_when_busy: true,
        }
    }
}

impl AppConfig {
    pub fn font_scale(&self) -> f32 {
        self.font_scale_pct.clamp(80, 150) as f32 / 100.0
    }

    /// Clamp all ranged fields; run on any config that bypassed the form (file load, import).
    pub fn sanitize(&mut self) {
        self.latitude = self.latitude.clamp(-90.0, 90.0);
        self.longitude = self.longitude.clamp(-180.0, 180.0);
        for a in [
            &mut self.fajr_adjust,
            &mut self.dhuhr_adjust,
            &mut self.asr_adjust,
            &mut self.maghrib_adjust,
            &mut self.isha_adjust,
        ] {
            *a = (*a).clamp(-60, 60);
        }
        self.widget_offset = self.widget_offset.clamp(0, 2000);
        self.hijri_adjust = self.hijri_adjust.clamp(-2, 2);
        self.reminder_minutes = self.reminder_minutes.clamp(1, 60);
        self.font_scale_pct = self.font_scale_pct.clamp(80, 150);
    }

    pub fn dir() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
        base.join("PrayerTray")
    }

    fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn is_first_run() -> bool {
        !Self::path().exists()
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &std::path::Path) -> Self {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(mut cfg) = serde_json::from_str::<Self>(&text) {
                cfg.sanitize();
                return cfg;
            }
        }
        Self::default() // missing or corrupt -> defaults
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = crate::util::write_atomic(&Self::path(), json.as_bytes());
        }
    }

    /// 999 = follow the system timezone (caller passes the current local offset).
    pub fn resolve_timezone(&self, system_offset_hours: f64) -> f64 {
        if self.timezone_hours == TZ_SYSTEM {
            system_offset_hours
        } else {
            self.timezone_hours
        }
    }

    pub fn calc_method(&self) -> &'static CalcMethod {
        method_by_key(&self.method)
    }

    pub fn high_lat(&self) -> HighLatRule {
        HighLatRule::from_name(&self.high_lats)
    }

    pub fn asr_juristic(&self) -> AsrJuristic {
        AsrJuristic::from_int(self.asr)
    }

    pub fn time_adjust(&self) -> Offsets {
        Offsets {
            fajr: self.fajr_adjust,
            dhuhr: self.dhuhr_adjust,
            asr: self.asr_adjust,
            maghrib: self.maghrib_adjust,
            isha: self.isha_adjust,
        }
    }
}

/// Small persisted state (not user settings) — survives restarts so reminders don't double-fire.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppState {
    pub sunnah_fast_noticed: String, // yyyy-MM-dd of the last eve-before fast notice
}

impl AppState {
    fn path() -> PathBuf {
        AppConfig::dir().join("state.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            let _ = crate::util::write_atomic(&Self::path(), json.as_bytes());
        }
    }
}
