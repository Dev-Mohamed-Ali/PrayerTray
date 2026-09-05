//! Net-metering pure logic: delta accounting, rate/size formatters, usage.json compat.

#![cfg(windows)]

use prayertray::native::net::IfRow;
use prayertray::services::data_usage::{size, Day};
use prayertray::services::latency;
use prayertray::services::net_speed::{delta, format_parts};
use std::collections::{BTreeMap, HashMap};

/// A row with no HardwareInterface flag — exercises the interface-type fallback.
fn row(guid: &str, if_type: u32, up: bool, filter: bool, rx: u64, tx: u64) -> IfRow {
    IfRow { guid: guid.into(), alias: "nic".into(), index: 0, if_type, up, filter, hardware: false, rx, tx }
}

/// A row NDIS reports as real hardware.
fn hw(guid: &str, if_type: u32, up: bool, rx: u64, tx: u64) -> IfRow {
    IfRow { guid: guid.into(), alias: "nic".into(), index: 0, if_type, up, filter: false, hardware: true, rx, tx }
}

const ETH: u32 = 6;
const WIFI: u32 = 71;
const LOOPBACK: u32 = 24;
const VIRTUAL: u32 = 53; // WireGuard / tun2socks adapters (nekoray, sing-box, Clash)
const TUNNEL: u32 = 131;

#[test]
fn delta_primes_on_first_pass() {
    let mut base = HashMap::new();
    let rows = [row("{a}", ETH, true, false, 1000, 500)];
    // First pass sees no baseline -> contributes nothing, just primes.
    assert_eq!(delta(&mut base, &rows), (0, 0));
    // Second pass with grown counters yields the difference.
    let rows = [row("{a}", ETH, true, false, 1300, 700)];
    assert_eq!(delta(&mut base, &rows), (300, 200));
}

#[test]
fn delta_counter_reset_primes_without_spiking() {
    let mut base = HashMap::new();
    delta(&mut base, &[row("{a}", ETH, true, false, 5000, 5000)]);
    // Counter reset (reboot/adapter reset): new value below baseline -> no giant delta.
    assert_eq!(delta(&mut base, &[row("{a}", ETH, true, false, 10, 10)]), (0, 0));
    // Subsequent growth from the re-primed baseline is normal.
    assert_eq!(delta(&mut base, &[row("{a}", ETH, true, false, 60, 40)]), (50, 30));
}

#[test]
fn delta_counts_physical_nics_only() {
    let mut base = HashMap::new();
    let rows = [
        row("{lo}", LOOPBACK, true, false, 100, 100),
        row("{tun}", TUNNEL, true, false, 100, 100),
        row("{vpn}", VIRTUAL, true, false, 100, 100),
        row("{flt}", ETH, true, true, 100, 100),
        row("{down}", ETH, false, false, 100, 100),
        row("{eth}", ETH, true, false, 100, 100),
        row("{wifi}", WIFI, true, false, 100, 100),
    ];
    delta(&mut base, &rows); // prime the eligible ones
    let rows2 = [
        row("{lo}", LOOPBACK, true, false, 200, 200),
        row("{tun}", TUNNEL, true, false, 200, 200),
        row("{vpn}", VIRTUAL, true, false, 200, 200),
        row("{flt}", ETH, true, true, 200, 200),
        row("{down}", ETH, false, false, 200, 200),
        row("{eth}", ETH, true, false, 250, 230),
        row("{wifi}", WIFI, true, false, 140, 120),
    ];
    // Only the ethernet and Wi-Fi rows count: 150+40 rx, 130+20 tx.
    assert_eq!(delta(&mut base, &rows2), (190, 150));
}

/// The bug this filter exists for: a tun2socks VPN carries the same payload as the NIC
/// underneath it, so counting both reports roughly double the real transfer.
#[test]
fn delta_does_not_double_count_a_vpn_tunnel() {
    let mut base = HashMap::new();
    let prime = [
        row("{eth}", ETH, true, false, 0, 0),
        row("{vpn}", VIRTUAL, true, false, 0, 0),
    ];
    delta(&mut base, &prime);
    // 1 MB downloaded: the physical NIC and the TUN both report it.
    let after = [
        row("{eth}", ETH, true, false, 1_000_000, 20_000),
        row("{vpn}", VIRTUAL, true, false, 1_000_000, 20_000),
    ];
    assert_eq!(delta(&mut base, &after), (1_000_000, 20_000));
}

/// A Wi-Fi Direct / Mobile Hotspot virtual adapter reports plain ethernet, so only the
/// HardwareInterface flag keeps it out of the total.
#[test]
fn delta_prefers_the_hardware_flag_over_interface_type() {
    let mut base = HashMap::new();
    let prime = [
        hw("{wifi}", WIFI, true, 0, 0),
        row("{direct}", ETH, true, false, 0, 0), // "Local Area Connection* 6"
        row("{vpn}", VIRTUAL, true, false, 0, 0),
    ];
    delta(&mut base, &prime);
    let after = [
        hw("{wifi}", WIFI, true, 900, 100),
        row("{direct}", ETH, true, false, 900, 100),
        row("{vpn}", VIRTUAL, true, false, 900, 100),
    ];
    // Only the hardware row counts once any row reports the flag.
    assert_eq!(delta(&mut base, &after), (900, 100));
}

/// A hardware row that is down or a filter pseudo-interface still never counts.
#[test]
fn delta_skips_down_and_filter_rows_even_with_the_hardware_flag() {
    let mut base = HashMap::new();
    let mut down = hw("{down}", ETH, false, 0, 0);
    let mut flt = hw("{flt}", ETH, true, 0, 0);
    flt.filter = true;
    let mut prime_down = hw("{down}", ETH, false, 0, 0);
    prime_down.filter = false;
    let mut prime_flt = hw("{flt}", ETH, true, 0, 0);
    prime_flt.filter = true;
    delta(&mut base, &[hw("{eth}", ETH, true, 0, 0), prime_down, prime_flt]);
    down.rx = 500;
    flt.rx = 500;
    let after = [hw("{eth}", ETH, true, 700, 300), down, flt];
    assert_eq!(delta(&mut base, &after), (700, 300));
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
fn metered_rows_are_real_nics_that_are_up() {
    let rows = prayertray::native::net::snapshot();
    for r in prayertray::native::net::metered(&rows) {
        assert!(r.up && !r.filter, "{}: metered rows must be up and non-filter", r.alias);
        assert!(
            r.hardware || r.is_physical(),
            "{}: metered rows must be hardware, or physical-typed on the fallback path",
            r.alias
        );
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

fn at(mut r: IfRow, index: u32) -> IfRow {
    r.index = index;
    r
}

/// The whole point of the indicator: the default route belongs to the TUN, not the NIC.
#[test]
fn tunnel_route_is_detected_when_the_vpn_owns_the_default_route() {
    let rows = [at(hw("{eth}", ETH, true, 0, 0), 18), at(row("{vpn}", VIRTUAL, true, false, 0, 0), 7)];
    assert!(prayertray::native::net::is_tunnel_route(&rows, 7));
    assert!(!prayertray::native::net::is_tunnel_route(&rows, 18), "the real NIC is not a tunnel");
}

/// A down or filter-pseudo interface must never be read as an active tunnel.
#[test]
fn tunnel_route_ignores_unusable_rows() {
    let mut down = row("{vpn}", VIRTUAL, false, false, 0, 0);
    down.index = 7;
    let rows = [at(hw("{eth}", ETH, true, 0, 0), 18), down];
    assert!(!prayertray::native::net::is_tunnel_route(&rows, 7));
}

/// On a machine where nothing reports the hardware flag the check has no basis, so it must
/// stay silent rather than call every adapter a tunnel.
#[test]
fn tunnel_route_says_nothing_without_the_hardware_flag() {
    let rows = [at(row("{a}", ETH, true, false, 0, 0), 18), at(row("{b}", VIRTUAL, true, false, 0, 0), 7)];
    assert!(!prayertray::native::net::is_tunnel_route(&rows, 7));
}
