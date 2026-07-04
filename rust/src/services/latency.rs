//! Background latency probe. sample() kicks a probe at most ~1/3 s and returns the last
//! result (ms, or -1). Port of C# Services/Latency.cs.
//!
//! Note: the C# TCP probe could source-bind to the selected adapter; std's connect_timeout
//! can't bind-before-connect, so that niche is dropped — the connect still honours per-app
//! proxy/VPN rules because it's an app-owned socket.

use crate::native::net;
use crate::native::time::tick_count64;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const PROBE_TIMEOUT_MS: u32 = 2000;
const MIN_INTERVAL_MS: u64 = 3000;

pub struct Latency {
    ms: Arc<AtomicI32>,
    in_flight: Arc<AtomicBool>,
    last_sent: u64,
    host: String,
    tcp: bool,
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
            tcp: false,
        }
    }

    pub fn set_host(&mut self, host: &str) {
        let host = if host.trim().is_empty() { "1.1.1.1" } else { host.trim() };
        if host != self.host {
            self.host = host.to_owned();
            self.ms.store(-1, Ordering::Relaxed);
        }
    }

    pub fn set_mode(&mut self, tcp: bool) {
        if tcp != self.tcp {
            self.tcp = tcp;
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
            let (ms, flag, host, tcp) =
                (self.ms.clone(), self.in_flight.clone(), self.host.clone(), self.tcp);
            std::thread::spawn(move || {
                let result = if tcp { tcp_probe(&host) } else { net::icmp_ping(&host, PROBE_TIMEOUT_MS) };
                ms.store(result, Ordering::Relaxed);
                flag.store(false, Ordering::Relaxed);
            });
        }
        self.ms.load(Ordering::Relaxed)
    }
}

fn tcp_probe(host: &str) -> i32 {
    let Ok(mut addrs) = (host, 443u16).to_socket_addrs() else { return -1 };
    let Some(addr) = addrs.find(|a| a.is_ipv4()) else { return -1 };
    let t0 = tick_count64();
    match TcpStream::connect_timeout(&addr, Duration::from_millis(PROBE_TIMEOUT_MS as u64)) {
        Ok(_) => tick_count64().saturating_sub(t0) as i32,
        Err(_) => -1,
    }
}

pub fn format(ms: i32) -> String {
    if ms < 0 {
        "— ms".into()
    } else {
        format!("{ms} ms")
    }
}
