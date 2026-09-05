//! Live NIC throughput. Sample ~1/s; returns down/up bytes-per-second since the last sample.
//! Port of C# Services/NetSpeed.cs.

use crate::native::net::{self, IfRow};
use crate::native::time::tick_count64;
use std::collections::HashMap;

/// Real-NIC rx/tx deltas against `baseline` (updated in place). New/reappeared adapters and
/// counter resets prime the baseline without contributing, so a flapping NIC can't inject its
/// since-boot totals as one giant delta. Shared with the data-usage accumulator.
pub fn delta(baseline: &mut HashMap<String, (u64, u64)>, rows: &[IfRow]) -> (u64, u64) {
    let (mut dr, mut dt) = (0u64, 0u64);
    for r in net::metered(rows) {
        if let Some(&(brx, btx)) = baseline.get(&r.guid) {
            if r.rx >= brx && r.tx >= btx {
                dr += r.rx - brx;
                dt += r.tx - btx;
            }
        }
        baseline.insert(r.guid.clone(), (r.rx, r.tx));
    }
    (dr, dt)
}

pub struct NetSpeed {
    baseline: HashMap<String, (u64, u64)>,
    last_tick: u64,
}

impl Default for NetSpeed {
    fn default() -> Self {
        Self::new()
    }
}

impl NetSpeed {
    pub fn new() -> Self {
        Self { baseline: HashMap::new(), last_tick: 0 }
    }

    /// Down/up bytes-per-second since the last call. First sample or a long stall
    /// (sleep/resume) primes only, returning (0, 0).
    pub fn sample(&mut self, rows: &[IfRow]) -> (u64, u64) {
        let now = tick_count64();
        let secs = (now.saturating_sub(self.last_tick)) as f64 / 1000.0;
        let first = self.last_tick == 0;
        self.last_tick = now;
        let (dr, dt) = delta(&mut self.baseline, rows);
        if first || secs <= 0.0 || secs > 10.0 {
            return (0, 0);
        }
        ((dr as f64 / secs) as u64, (dt as f64 / secs) as u64)
    }
}

pub fn format_parts(down: u64, up: u64) -> (String, String) {
    (format!("↓ {}", rate(down)), format!("↑ {}", rate(up)))
}

fn rate(bps: u64) -> String {
    if bps < 1024 {
        return format!("{bps} B/s");
    }
    let v = bps as f64 / 1024.0;
    if v < 1024.0 {
        return format!("{} KB/s", num(v));
    }
    let v = v / 1024.0;
    if v < 1024.0 {
        return format!("{} MB/s", num(v));
    }
    format!("{} GB/s", num(v / 1024.0))
}

/// One decimal under 10, none above.
fn num(v: f64) -> String {
    if v < 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.0}")
    }
}
