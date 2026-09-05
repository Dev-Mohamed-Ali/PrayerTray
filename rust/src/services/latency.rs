//! Background latency probe. sample() kicks an ICMP echo at most ~1/3 s and returns the
//! last result (ms, or -1). Port of C# Services/Latency.cs.

use crate::native::net;
use crate::native::time::tick_count64;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

const PROBE_TIMEOUT_MS: u32 = 2000;
const MIN_INTERVAL_MS: u64 = 3000;

pub struct Latency {
    ms: Arc<AtomicI32>,
    in_flight: Arc<AtomicBool>,
    last_sent: u64,
    host: String,
}

impl Default for Latency {
    fn default() -> Self {
        Self::new()
    }
}

impl Latency {
    pub fn new() -> Self {
        Self {
            ms: Arc::new(AtomicI32::new(-1)),
            in_flight: Arc::new(AtomicBool::new(false)),
            last_sent: 0,
            host: "1.1.1.1".into(),
        }
    }

    pub fn set_host(&mut self, host: &str) {
        let host = if host.trim().is_empty() { "1.1.1.1" } else { host.trim() };
        if host != self.host {
            self.host = host.to_owned();
            self.ms.store(-1, Ordering::Relaxed);
        }
    }

    /// Kick a probe if idle and enough time has passed; returns the last cached result.
    pub fn sample(&mut self) -> i32 {
        let now = tick_count64();
        if !self.in_flight.load(Ordering::Relaxed) && now.saturating_sub(self.last_sent) >= MIN_INTERVAL_MS
        {
            self.last_sent = now;
            self.in_flight.store(true, Ordering::Relaxed);
            let (ms, flag, host) =
                (self.ms.clone(), self.in_flight.clone(), self.host.clone());
            std::thread::spawn(move || {
                ms.store(net::icmp_ping(&host, PROBE_TIMEOUT_MS), Ordering::Relaxed);
                flag.store(false, Ordering::Relaxed);
            });
        }
        self.ms.load(Ordering::Relaxed)
    }
}

pub fn format(ms: i32) -> String {
    if ms < 0 {
        "— ms".into()
    } else {
        format!("{ms} ms")
    }
}
