//! Config compatibility with the C# build: PascalCase names, sentinels, defaults, clamps.

use prayertray::config::{AppConfig, POPUP_UNSET, TZ_SYSTEM};

/// A config.json as written by the C# v1.14 build (System.Text.Json, WriteIndented).
const CSHARP_CONFIG: &str = r#"{
  "City": "Cairo",
  "Latitude": 30.0444,
  "Longitude": 31.2357,
  "Method": "Egypt",
  "Asr": 2,
  "HighLats": "MidNight",
  "FajrAdjust": 3,
  "DhuhrAdjust": 0,
  "AsrAdjust": -2,
  "MaghribAdjust": 0,
  "IshaAdjust": 90,
  "Use24Hour": true,
  "WidgetAnchor": "Left",
  "WidgetOffset": 40,
  "Theme": "Midnight",
  "MonitorDeviceName": "\\\\.\\DISPLAY2",
  "HideOnFullscreen": false,
  "ShowNetSpeed": true,
  "ShowPing": true,
  "PingHost": "8.8.8.8",
  "CompactMeters": true,
  "TrackDataUsage": true,
  "ShowDataUsage": false,
  "TimezoneHours": 2,
  "Language": "ar",
  "ShowHijriDate": true,
  "HijriAdjust": -1,
  "ShowIslamicEvents": true,
  "SunnahFastReminder": true,
  "FridayReminder": true,
  "FontFamily": "Cairo",
  "FontScalePct": 120,
  "PopupPinned": false,
  "PopupX": -2147483648,
  "PopupY": -2147483648,
  "RichToasts": true,
  "ReminderEnabled": true,
  "ReminderMinutes": 15,
  "ReminderSound": true,
  "ReminderSoundId": "chime",
  "ReminderSoundPath": null,
  "AzanMode": "makkah",
  "AzanCustomPath": null
}"#;

#[test]
fn reads_csharp_config_and_sanitizes() {
    let mut cfg: AppConfig = serde_json::from_str(CSHARP_CONFIG).unwrap();
    cfg.sanitize();
    assert_eq!(cfg.city, "Cairo");
    assert_eq!(cfg.method, "Egypt");
    assert_eq!(cfg.asr, 2);
    assert_eq!(cfg.isha_adjust, 60, "sanitize clamps 90 -> 60");
    assert!(cfg.use24_hour);
    assert_eq!(cfg.monitor_device_name.as_deref(), Some("\\\\.\\DISPLAY2"));
    assert_eq!(cfg.timezone_hours, 2.0);
    assert_eq!(cfg.language, "ar");
    assert_eq!(cfg.hijri_adjust, -1);
    assert_eq!(cfg.font_scale_pct, 120);
    assert_eq!(cfg.popup_x, POPUP_UNSET);
    assert_eq!(cfg.reminder_sound_path, None);
    assert_eq!(cfg.azan_mode, "makkah");
}

/// serde_json keeps int/float tokens distinct (2 vs 2.0); C# reads either. Compare as f64.
fn canon(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                *v = serde_json::json!(f);
            }
        }
        serde_json::Value::Object(m) => m.values_mut().for_each(canon),
        serde_json::Value::Array(a) => a.iter_mut().for_each(canon),
        _ => {}
    }
}

/// The contract is that no C# field is ever dropped or renamed on the way back out. Rust-only
/// settings added since the port are allowed to appear alongside them, so this is a subset check
/// rather than an equality one.
#[test]
fn roundtrip_preserves_every_csharp_field() {
    let cfg: AppConfig = serde_json::from_str(CSHARP_CONFIG).unwrap();
    let out = serde_json::to_string_pretty(&cfg).unwrap();
    let mut a: serde_json::Value = serde_json::from_str(CSHARP_CONFIG).unwrap();
    let mut b: serde_json::Value = serde_json::from_str(&out).unwrap();
    canon(&mut a);
    canon(&mut b);
    let (before, after) = (a.as_object().unwrap(), b.as_object().unwrap());
    for (key, value) in before {
        assert_eq!(after.get(key), Some(value), "C# field {key} must survive a round-trip");
    }
}

/// v1.x/v2.0 configs still carry the removed iqamah keys; they must not break loading.
#[test]
fn removed_iqamah_fields_are_ignored() {
    let cfg: AppConfig =
        serde_json::from_str(r#"{"City":"Cairo","FajrIqamah":20,"IshaIqamah":10}"#).unwrap();
    assert_eq!(cfg.city, "Cairo");
}

#[test]
fn missing_fields_take_csharp_defaults() {
    let cfg: AppConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(cfg.city, "Makkah");
    assert_eq!(cfg.latitude, 21.4225);
    assert_eq!(cfg.method, "MWL");
    assert_eq!(cfg.high_lats, "AngleBased");
    assert_eq!(cfg.widget_anchor, "Right");
    assert_eq!(cfg.widget_offset, 12);
    assert_eq!(cfg.theme, "Auto");
    assert_eq!(cfg.ping_host, "1.1.1.1");
    assert_eq!(cfg.timezone_hours, TZ_SYSTEM);
    assert_eq!(cfg.language, "auto");
    assert!(cfg.show_hijri_date && cfg.show_islamic_events && cfg.rich_toasts && cfg.reminder_sound);
    assert_eq!(cfg.font_family, "Segoe UI");
    assert_eq!(cfg.font_scale_pct, 100);
    assert_eq!(cfg.popup_x, POPUP_UNSET);
    assert_eq!(cfg.reminder_minutes, 10);
    assert_eq!(cfg.reminder_sound_id, "chime");
    assert_eq!(cfg.azan_mode, "None");
    assert!(cfg.hide_on_fullscreen);
}

#[test]
fn corrupt_config_falls_back_to_defaults() {
    let dir = std::env::temp_dir().join("prayertray-test-cfg");
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("corrupt.json");
    std::fs::write(&p, "{not json").unwrap();
    let cfg = AppConfig::load_from(&p);
    assert_eq!(cfg.city, "Makkah");
}
