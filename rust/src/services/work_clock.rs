//! Manual work-clock: Start/End toggle accruing worked seconds per day into work.json
//! (kept ~365 days). Same accumulator shape as data_usage, but time-driven and manually gated.

use crate::config::AppConfig;
use crate::native::time::{day_key_offset, tick_count64, today_key};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

const KEEP_DAYS: i64 = 365;
const AUTOSAVE_MS: u64 = 300_000;
const GAP_MS: u64 = 10_000;

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct Persist {
    days: BTreeMap<String, u64>, // seconds worked per yyyy-MM-dd
    active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct PersistRef<'a> {
    days: &'a BTreeMap<String, u64>,
    active: bool,
}

#[derive(Default)]
pub struct WorkClock {
    days: BTreeMap<String, u64>,
    active: bool,
    last_tick: u64,
    carry_ms: u64,
    last_save: u64,
    dirty: bool,
}

fn file_path() -> PathBuf {
    AppConfig::dir().join("work.json")
}

impl WorkClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self) {
        let p: Persist = std::fs::read_to_string(file_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        self.days = p.days;
        self.active = p.active;
        self.last_tick = 0; // resume without backfilling the app-closed gap
        self.carry_ms = 0;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn start(&mut self) {
        self.active = true;
        self.last_tick = tick_count64();
        self.carry_ms = 0;
        self.dirty = true;
        self.save();
    }

    pub fn stop(&mut self) {
        self.active = false;
        self.dirty = true;
        self.save();
    }

    pub fn toggle(&mut self) {
        if self.active {
            self.stop();
        } else {
            self.start();
        }
    }

    pub fn tick(&mut self) {
        if !self.active {
            return;
        }
        let now = tick_count64();
        self.tick_at(now, &today_key());
        if self.dirty && now.saturating_sub(self.last_save) >= AUTOSAVE_MS {
            self.save();
        }
    }

    /// Accrue `now − last_tick` into `days[day]`. Split out for deterministic tests.
    fn tick_at(&mut self, now: u64, day: &str) {
        // First tick after start/load, or a gap (sleep, app closed): prime, don't count it.
        if self.last_tick == 0 || now.saturating_sub(self.last_tick) > GAP_MS {
            self.last_tick = now;
            return;
        }
        self.carry_ms += now - self.last_tick;
        self.last_tick = now;
        let secs = self.carry_ms / 1000;
        if secs > 0 {
            self.carry_ms -= secs * 1000;
            *self.days.entry(day.to_string()).or_default() += secs;
            self.dirty = true;
        }
    }

    pub fn today(&self) -> u64 {
        self.days.get(&today_key()).copied().unwrap_or(0)
    }

    /// History newest-first (BTreeMap iterates ascending, so reverse).
    pub fn history(&self) -> Vec<(String, u64)> {
        self.days.iter().rev().map(|(k, v)| (k.clone(), *v)).collect()
    }

    pub fn reset(&mut self) {
        self.days.clear();
        self.dirty = true;
        self.save();
    }

    pub fn flush(&mut self) {
        if self.dirty {
            self.save();
        }
    }

    fn save(&mut self) {
        self.last_save = tick_count64();
        let cutoff = day_key_offset(KEEP_DAYS);
        self.days.retain(|k, _| k.as_str() >= cutoff.as_str());
        let _ = std::fs::create_dir_all(AppConfig::dir());
        let p = PersistRef { days: &self.days, active: self.active };
        if let Ok(json) = serde_json::to_string(&p) {
            if std::fs::write(file_path(), json).is_ok() {
                self.dirty = false;
            }
        }
    }
}

/// "3h 24m" — for the menu label, popup line, and dialog.
pub fn fmt_hm(secs: u64) -> String {
    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
}

/// "3:24" — compact pill segment (worst-case slot template `⏱ 88:88`).
pub fn fmt_clock(secs: u64) -> String {
    format!("{}:{:02}", secs / 3600, (secs % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_at(t: u64) -> WorkClock {
        let mut w = WorkClock::new();
        w.active = true;
        w.last_tick = t;
        w
    }

    #[test]
    fn accrues_elapsed_into_the_day() {
        let mut w = active_at(1_000);
        w.tick_at(6_000, "2026-07-05"); // +5 s
        w.tick_at(9_500, "2026-07-05"); // +3.5 s -> 8 s whole, 0.5 carried
        assert_eq!(w.today_of("2026-07-05"), 8);
        assert_eq!(w.carry_ms, 500);
    }

    #[test]
    fn gap_does_not_count() {
        let mut w = active_at(1_000);
        w.tick_at(20_000, "2026-07-05"); // 19 s gap > 10 s -> primed, no count
        assert_eq!(w.today_of("2026-07-05"), 0);
        w.tick_at(23_000, "2026-07-05"); // +3 s counts
        assert_eq!(w.today_of("2026-07-05"), 3);
    }

    #[test]
    fn first_tick_primes() {
        let mut w = WorkClock::new();
        w.active = true; // last_tick == 0
        w.tick_at(5_000, "2026-07-05");
        assert_eq!(w.today_of("2026-07-05"), 0);
        w.tick_at(7_000, "2026-07-05");
        assert_eq!(w.today_of("2026-07-05"), 2);
    }

    #[test]
    fn splits_across_midnight() {
        let mut w = active_at(1_000);
        w.tick_at(4_000, "2026-07-05"); // +3 s day 1
        w.tick_at(6_000, "2026-07-06"); // +2 s day 2
        assert_eq!(w.today_of("2026-07-05"), 3);
        assert_eq!(w.today_of("2026-07-06"), 2);
    }

    #[test]
    fn history_is_newest_first() {
        let mut w = WorkClock::new();
        w.days.insert("2026-07-03".into(), 10);
        w.days.insert("2026-07-05".into(), 30);
        w.days.insert("2026-07-04".into(), 20);
        let h = w.history();
        assert_eq!(h[0].0, "2026-07-05");
        assert_eq!(h[2].0, "2026-07-03");
    }

    #[test]
    fn persist_round_trip_keeps_active_and_days() {
        let mut days = BTreeMap::new();
        days.insert("2026-07-05".to_string(), 3661u64);
        let json = serde_json::to_string(&PersistRef { days: &days, active: true }).unwrap();
        assert!(json.contains("\"Days\""));
        assert!(json.contains("\"Active\""));
        let back: Persist = serde_json::from_str(&json).unwrap();
        assert!(back.active);
        assert_eq!(back.days["2026-07-05"], 3661);
    }

    #[test]
    fn formatters() {
        assert_eq!(fmt_hm(0), "0h 0m");
        assert_eq!(fmt_hm(59), "0h 0m");
        assert_eq!(fmt_hm(3600), "1h 0m");
        assert_eq!(fmt_hm(3661), "1h 1m");
        assert_eq!(fmt_hm(12 * 3600 + 34 * 60), "12h 34m");
        assert_eq!(fmt_hm(25 * 3600), "25h 0m");
        assert_eq!(fmt_clock(0), "0:00");
        assert_eq!(fmt_clock(3661), "1:01");
        assert_eq!(fmt_clock(3 * 3600 + 24 * 60), "3:24");
    }

    // Test-only accessor mirroring today() but for an explicit key.
    impl WorkClock {
        fn today_of(&self, day: &str) -> u64 {
            self.days.get(day).copied().unwrap_or(0)
        }
    }
}
