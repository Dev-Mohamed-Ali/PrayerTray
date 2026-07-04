//! Net-metering pure logic: delta accounting, rate/size formatters, usage.json compat.

#![cfg(windows)]

use prayertray::native::net::IfRow;
use prayertray::services::data_usage::{size, Day};
use prayertray::services::latency;
use prayertray::services::net_speed::{delta, format_parts};
use std::collections::{BTreeMap, HashMap};

fn row(guid: &str, if_type: u32, up: bool, filter: bool, rx: u64, tx: u64) -> IfRow {
    IfRow { guid: guid.into(), alias: "nic".into(), if_type, up, filter, rx, tx }
}

// Ordinary ethernet adapter type.
const ETH: u32 = 6;
const LOOPBACK: u32 = 24;
const TUNNEL: u32 = 131;

#[test]
fn delta_primes_on_first_pass() {
    let mut base = HashMap::new();
    let rows = [row("{a}", ETH, true, false, 1000, 500)];
    // First pass sees no baseline -> contributes nothing, just primes.
    assert_eq!(delta(&mut base, None, &rows), (0, 0));
    // Second pass with grown counters yields the difference.
    let rows = [row("{a}", ETH, true, false, 1300, 700)];
    assert_eq!(delta(&mut base, None, &rows), (300, 200));
}

#[test]
fn delta_counter_reset_primes_without_spiking() {
    let mut base = HashMap::new();
    delta(&mut base, None, &[row("{a}", ETH, true, false, 5000, 5000)]);
    // Counter reset (reboot/adapter reset): new value below baseline -> no giant delta.
    assert_eq!(delta(&mut base, None, &[row("{a}", ETH, true, false, 10, 10)]), (0, 0));
    // Subsequent growth from the re-primed baseline is normal.
    assert_eq!(delta(&mut base, None, &[row("{a}", ETH, true, false, 60, 40)]), (50, 30));
}

#[test]
fn delta_excludes_loopback_tunnel_filter_and_down_when_all() {
    let mut base = HashMap::new();
    let rows = [
        row("{lo}", LOOPBACK, true, false, 100, 100),
        row("{tun}", TUNNEL, true, false, 100, 100),
        row("{flt}", ETH, true, true, 100, 100),
        row("{down}", ETH, false, false, 100, 100),
        row("{ok}", ETH, true, false, 100, 100),
    ];
    delta(&mut base, None, &rows); // prime the eligible ones
    let rows2 = [
        row("{lo}", LOOPBACK, true, false, 200, 200),
        row("{tun}", TUNNEL, true, false, 200, 200),
        row("{flt}", ETH, true, true, 200, 200),
        row("{down}", ETH, false, false, 200, 200),
        row("{ok}", ETH, true, false, 250, 230),
    ];
    // Only {ok} counts.
    assert_eq!(delta(&mut base, None, &rows2), (150, 130));
}

#[test]
fn delta_explicit_pick_includes_tunnel_and_is_case_insensitive() {
    let mut base = HashMap::new();
    let rows = [row("{TUN-1}", TUNNEL, true, false, 100, 100)];
    delta(&mut base, Some("{tun-1}"), &rows); // prime; guid match ignores case
    let rows2 = [row("{TUN-1}", TUNNEL, true, false, 180, 140)];
    assert_eq!(delta(&mut base, Some("{tun-1}"), &rows2), (80, 40));
}

#[test]
fn rate_formatting_boundaries() {
    assert_eq!(format_parts(0, 1023), ("↓ 0 B/s".into(), "↑ 1023 B/s".into()));
    // 1024 B/s -> 1.0 KB/s (one decimal under 10).
    assert_eq!(format_parts(1024, 0).0, "↓ 1.0 KB/s");
    // 10 KB/s -> no decimal.
    assert_eq!(format_parts(10 * 1024, 0).0, "↓ 10 KB/s");
    // 1.5 MB/s.
    assert_eq!(format_parts(3 * 1024 * 1024 / 2, 0).0, "↓ 1.5 MB/s");
}

#[test]
fn size_formatting_three_sig_figs() {
    assert_eq!(size(0), "0 B");
    assert_eq!(size(1023), "1023 B");
    assert_eq!(size(1024), "1.00 KB");
    assert_eq!(size(1024 * 1024), "1.00 MB");
    // 12.5 MB -> one decimal (>=10, <100).
    assert_eq!(size(25 * 1024 * 1024 / 2), "12.5 MB");
    // 512 MB -> no decimal (>=100).
    assert_eq!(size(512 * 1024 * 1024), "512 MB");
}

#[test]
fn latency_format() {
    assert_eq!(latency::format(-1), "— ms");
    assert_eq!(latency::format(0), "0 ms");
    assert_eq!(latency::format(42), "42 ms");
}

#[test]
fn snapshot_returns_rows_with_braced_guids() {
    // Live GetIfTable2 on the host: there is always at least a loopback interface.
    let rows = prayertray::native::net::snapshot();
    assert!(!rows.is_empty(), "GetIfTable2 returned no interfaces");
    assert!(
        rows.iter().all(|r| r.guid.starts_with('{') && r.guid.ends_with('}')),
        "every interface must have a braced GUID"
    );
}

#[test]
fn adapters_are_a_subset_of_snapshot_guids() {
    let snap: std::collections::HashSet<String> =
        prayertray::native::net::snapshot().into_iter().map(|r| r.guid).collect();
    for (_, guid) in prayertray::native::net::adapters() {
        assert!(snap.contains(&guid), "adapter guid must come from the snapshot");
    }
}

#[test]
#[ignore = "does real network I/O; run explicitly"]
fn icmp_ping_localhost_is_fast() {
    let ms = prayertray::native::net::icmp_ping("127.0.0.1", 1000);
    assert!(ms >= 0, "loopback ping should succeed");
}

#[test]
fn usage_json_is_csharp_compatible() {
    // Shape written by the C# Dictionary<string, Day(long Rx, long Tx)>.
    let csharp = r#"{"2026-07-03":{"Rx":1234,"Tx":5678},"2026-07-04":{"Rx":10,"Tx":20}}"#;
    let map: BTreeMap<String, Day> = serde_json::from_str(csharp).unwrap();
    assert_eq!(map["2026-07-03"].rx, 1234);
    assert_eq!(map["2026-07-03"].tx, 5678);
    // Round-trips back to the same field names.
    let out = serde_json::to_string(&map).unwrap();
    assert!(out.contains(r#""Rx":10"#) && out.contains(r#""Tx":20"#));
}
