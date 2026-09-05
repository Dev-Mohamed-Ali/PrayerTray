//! Daily traffic accumulator persisted to usage.json (kept ~90 days). Tick ~1/s.
//! Port of C# Services/DataUsage.cs; JSON-compatible with the v1.x file.

use crate::config::AppConfig;
use crate::native::net::IfRow;
use crate::native::time::{day_key_offset, tick_count64, today_key};
use crate::services::net_speed;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

const KEEP_DAYS: i64 = 90;
const AUTOSAVE_MS: u64 = 300_000;

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Day {
    pub rx: u64,
    pub tx: u64,
}

pub struct DataUsage {
    days: BTreeMap<String, Day>,
    baseline: HashMap<String, (u64, u64)>,
    last_tick: u64,
    last_save: u64,
    dirty: bool,
}

fn file_path() -> PathBuf {
    AppConfig::dir().join("usage.json")
}

impl Default for DataUsage {
    fn default() -> Self {
        Self::new()
    }
}

impl DataUsage {
    pub fn new() -> Self {
        Self {
            days: BTreeMap::new(),
            baseline: HashMap::new(),
            last_tick: 0,
            last_save: 0,
            dirty: false,
        }
    }

    pub fn load(&mut self) {
        self.days = std::fs::read_to_string(file_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
    }

    pub fn tick(&mut self, rows: &[IfRow]) {
        let now = tick_count64();
        // A gap (sleep, tracking toggled off) means the baselines are stale; re-prime
        // rather than attribute the whole untracked window to this one tick.
        if self.last_tick > 0 && now.saturating_sub(self.last_tick) > 10_000 {
            self.baseline.clear();
        }
        self.last_tick = now;

        let (dr, dt) = net_speed::delta(&mut self.baseline, rows);
        if dr > 0 || dt > 0 {
            let day = today_key();
            let e = self.days.entry(day).or_default();
            e.rx += dr;
            e.tx += dt;
            self.dirty = true;
        }

        if self.dirty && now.saturating_sub(self.last_save) >= AUTOSAVE_MS {
            self.save();
        }
    }

    pub fn today(&self) -> (u64, u64) {
        self.days.get(&today_key()).map(|d| (d.rx, d.tx)).unwrap_or((0, 0))
    }

    /// History newest-first, including today (BTreeMap iterates ascending, so reverse).
    pub fn history(&self) -> Vec<(String, u64, u64)> {
        self.days.iter().rev().map(|(k, d)| (k.clone(), d.rx, d.tx)).collect()
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
        self.last_save = tick_count64(); // even on failure, retry at the normal cadence
        let cutoff = day_key_offset(KEEP_DAYS);
        self.days.retain(|k, _| k.as_str() >= cutoff.as_str());
        let _ = std::fs::create_dir_all(AppConfig::dir());
        if let Ok(json) = serde_json::to_string(&self.days) {
            if std::fs::write(file_path(), json).is_ok() {
                self.dirty = false;
            }
        }
    }
}

/// "512 B" / "3.25 KB" / "1.2 GB" — 3 significant figures.
pub fn size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let v = bytes as f64 / 1024.0;
    if v < 1024.0 {
        return format!("{} KB", num(v));
    }
    let v = v / 1024.0;
    if v < 1024.0 {
        return format!("{} MB", num(v));
    }
    let v = v / 1024.0;
    if v < 1024.0 {
        return format!("{} GB", num(v));
    }
    format!("{} TB", num(v / 1024.0))
}

fn num(v: f64) -> String {
    if v < 10.0 {
        format!("{v:.2}")
    } else if v < 100.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.0}")
    }
}
