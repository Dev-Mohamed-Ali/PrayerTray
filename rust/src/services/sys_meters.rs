//! CPU load and memory pressure, sampled on the same 1 s tick as the net meters.

use crate::native::time::tick_count64;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

fn ticks(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

/// A gap this long makes the previous counters useless: diffing across it reports the average
/// over the whole pause, not the current load. Matches the net samplers' re-prime rule.
const STALE_MS: u64 = 5_000;

#[derive(Default)]
pub struct SysMeters {
    idle: u64,
    busy: u64,
    last: u64,
    primed: bool,
}

impl SysMeters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whole-percent CPU load since the last call. The first call only primes, returning 0.
    pub fn cpu_percent(&mut self) -> u32 {
        let (mut idle_ft, mut kernel_ft, mut user_ft) = Default::default();
        if unsafe { GetSystemTimes(Some(&mut idle_ft), Some(&mut kernel_ft), Some(&mut user_ft)) }
            .is_err()
        {
            return 0;
        }
        // Kernel time already includes idle, so busy is (kernel - idle) + user.
        let idle = ticks(idle_ft);
        let busy = (ticks(kernel_ft) + ticks(user_ft)).saturating_sub(idle);
        let (d_idle, d_busy) = (idle.saturating_sub(self.idle), busy.saturating_sub(self.busy));
        let now = tick_count64();
        let primed = self.primed && now.saturating_sub(self.last) <= STALE_MS;
        self.idle = idle;
        self.busy = busy;
        self.last = now;
        self.primed = true;
        let span = d_idle + d_busy;
        if !primed || span == 0 {
            return 0;
        }
        percent(d_busy, span)
    }
}

/// Physical memory in use, as Windows itself reports it in Task Manager.
pub fn memory_percent() -> u32 {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    if unsafe { GlobalMemoryStatusEx(&mut m) }.is_err() {
        return 0;
    }
    m.dwMemoryLoad.min(100)
}

fn percent(part: u64, whole: u64) -> u32 {
    if whole == 0 {
        return 0;
    }
    (((part * 100 + whole / 2) / whole) as u32).min(100)
}

pub fn format_cpu(pct: u32) -> String {
    format!("CPU {pct}%")
}

pub fn format_ram(pct: u32) -> String {
    format!("RAM {pct}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_rounds_to_nearest_and_clamps() {
        assert_eq!(percent(0, 100), 0);
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(2, 3), 67); // .666 rounds up
        assert_eq!(percent(100, 100), 100);
        assert_eq!(percent(5, 0), 0); // no elapsed span
    }

    #[test]
    fn first_cpu_sample_only_primes() {
        let mut s = SysMeters::new();
        assert_eq!(s.cpu_percent(), 0, "first call has no previous counters to diff against");
        assert!(s.primed);
    }
}
