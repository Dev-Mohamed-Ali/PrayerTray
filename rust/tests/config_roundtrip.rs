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

/// No C# field may be dropped or renamed; Rust-only settings may sit alongside them.
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

/// reminder_sound_id lands in a filename (audio::synth_path), and on_import accepts arbitrary
/// JSON into it, so sanitize() must reject anything that is not a known tone.
#[test]
fn imported_reminder_sound_id_cannot_escape_the_temp_directory() {
    let hostile = r#"{"ReminderSoundId":"../../../Users/Public/x"}"#;
    let cfg: AppConfig = serde_json::from_str(hostile).unwrap();
    let mut cfg = cfg;
    cfg.sanitize();
    assert_eq!(cfg.reminder_sound_id, "chime");

    // Every id the settings combo can produce must survive untouched.
    for id in prayertray::config::REMINDER_SOUND_IDS {
        let mut c = AppConfig { reminder_sound_id: id.into(), ..AppConfig::default() };
        c.sanitize();
        assert_eq!(c.reminder_sound_id, id);
    }
}

/// The pill order drives which segments render and in what order; an imported or hand-edited
/// file may repeat, omit or invent ids, and every segment must still appear exactly once.
#[test]
fn pill_order_is_repaired_to_every_segment_exactly_once() {
    let cases = [
        r#"{"PillOrder":[]}"#,
        r#"{"PillOrder":["ping","ping","ping"]}"#,
        r#"{"PillOrder":["nonsense","usage","../etc"]}"#,
        r#"{}"#,
    ];
    for json in cases {
        let mut cfg: AppConfig = serde_json::from_str(json).unwrap();
        cfg.sanitize();
        let mut sorted = cfg.pill_order.clone();
        sorted.sort();
        let mut want: Vec<String> = prayertray::config::PILL_SEGMENTS.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(sorted, want, "{json}");
    }
}

/// A partial order keeps the caller's sequence and gains the rest behind it.
#[test]
fn pill_order_keeps_the_positions_it_was_given() {
    let mut cfg: AppConfig = serde_json::from_str(r#"{"PillOrder":["usage","vpn"]}"#).unwrap();
    cfg.sanitize();
    assert_eq!(&cfg.pill_order[..2], &["usage".to_string(), "vpn".to_string()]);
}

#[test]
fn unknown_head_layout_and_period_fall_back() {
    let mut cfg: AppConfig =
        serde_json::from_str(r#"{"HeadLayout":"sideways","UsagePeriod":"decade"}"#).unwrap();
    cfg.sanitize();
    assert_eq!(cfg.head_layout, "stacked");
    assert_eq!(cfg.usage_period, "today");

    for id in prayertray::config::HEAD_LAYOUTS {
        let mut c = AppConfig { head_layout: id.into(), ..AppConfig::default() };
        c.sanitize();
        assert_eq!(c.head_layout, id);
    }
}
