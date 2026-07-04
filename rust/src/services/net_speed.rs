//! Live NIC throughput. Sample ~1/s; returns down/up bytes-per-second since the last sample.
//! Port of C# Services/NetSpeed.cs.

use crate::native::net::IfRow;
use crate::native::time::tick_count64;
use std::collections::HashMap;

const IF_TYPE_LOOPBACK: u32 = 24;
const IF_TYPE_TUNNEL: u32 = 131;

/// Per-adapter rx/tx deltas against `baseline` (updated in place). New/reappeared adapters and
/// counter resets prime the baseline without contributing, so a flapping NIC can't inject its
/// since-boot totals as one giant delta. Shared with the data-usage accumulator.
pub fn delta(
    baseline: &mut HashMap<String, (u64, u64)>,
    iface: Option<&str>,
    rows: &[IfRow],
) -> (u64, u64) {
    let (mut dr, mut dt) = (0u64, 0u64);
    for r in rows {
        match iface {
            Some(id) => {
                if !r.guid.eq_ignore_ascii_case(id) {
                    continue; // explicit pick wins, even for tunnel adapters
                }
            }
            None => {
                if r.if_type == IF_TYPE_LOOPBACK || r.if_type == IF_TYPE_TUNNEL || r.filter {
                    continue;
                }
            }
        }
        if !r.up {
            continue;
        }
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
    iface: Option<String>,
}

impl Default for NetSpeed {
    fn default() -> Self {
        Self::new()
    }
}

impl NetSpeed {
    pub fn new() -> Self {
        Self { baseline: HashMap::new(), last_tick: 0, iface: None }
    }

    /// Restrict counters to one adapter (None = all). VPN TUN adapters otherwise double-count.
    pub fn set_interface(&mut self, id: Option<&str>) {
        if id != self.iface.as_deref() {
            self.iface = id.map(str::to_owned);
            self.baseline.clear();
        }
    }

    /// Down/up bytes-per-second since the last call. First sample or a long stall
    /// (sleep/resume) primes only, returning (0, 0).
    pub fn sample(&mut self, rows: &[IfRow]) -> (u64, u64) {
        let now = tick_count64();
        let secs = (now.saturating_sub(self.last_tick)) as f64 / 1000.0;
        let first = self.last_tick == 0;
        self.last_tick = now;
        let (dr, dt) = delta(&mut self.baseline, self.iface.as_deref(), rows);
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
