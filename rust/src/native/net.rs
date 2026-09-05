//! Per-adapter byte counters (GetIfTable2) and a one-shot ICMP echo.
//! Replaces the C# System.Net.NetworkInformation surface.

use std::ffi::c_void;
use std::net::ToSocketAddrs;
use windows::core::GUID;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY,
    MIB_IF_TABLE2,
};

// IF_TYPE values (iftypes.h); the windows crate doesn't surface these as constants.
const IF_TYPE_ETHERNET: u32 = 6;
const IF_TYPE_IEEE80211: u32 = 71;
const IF_TYPE_WWANPP: u32 = 243;
const IF_TYPE_WWANPP2: u32 = 244;
const IF_OPER_STATUS_UP: i32 = 1;

/// One network adapter's octet counters and identity, as needed for metering.
pub struct IfRow {
    pub guid: String, // braced, e.g. "{XXXXXXXX-...}" — matches .NET NetworkInterface.Id
    pub alias: String,
    pub if_type: u32,
    pub up: bool,
    pub filter: bool, // WFP callout pseudo-interface -> would double-count
    pub hardware: bool, // NDIS HardwareInterface: backed by a real miniport
    pub rx: u64,
    pub tx: u64,
}

impl IfRow {
    /// Ethernet, Wi-Fi, or mobile broadband by interface type. Coarser than the hardware
    /// flag — Wi-Fi Direct and Hyper-V virtual adapters also report type 6.
    pub fn is_physical(&self) -> bool {
        matches!(
            self.if_type,
            IF_TYPE_ETHERNET | IF_TYPE_IEEE80211 | IF_TYPE_WWANPP | IF_TYPE_WWANPP2
        )
    }

    fn usable(&self) -> bool {
        self.up && !self.filter
    }
}

/// The adapters whose counters may be summed. A VPN TUN, a Wi-Fi Direct virtual, or a
/// Hyper-V vSwitch carries the same bytes as the NIC beneath it, so counting both reports
/// roughly double the real transfer; NDIS's HardwareInterface flag is the only field that
/// separates them (`if_type` does not — several of them report plain ethernet).
///
/// If nothing reports the flag, fall back to the interface-type allowlist so the meters
/// still show something rather than a permanent zero.
pub fn metered(rows: &[IfRow]) -> impl Iterator<Item = &IfRow> {
    let any_hw = rows.iter().any(|r| r.usable() && r.hardware);
    rows.iter()
        .filter(move |r| r.usable() && if any_hw { r.hardware } else { r.is_physical() })
}

fn guid_braces(g: &GUID) -> String {
    let d4 = g.data4;
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1, g.data2, g.data3, d4[0], d4[1], d4[2], d4[3], d4[4], d4[5], d4[6], d4[7]
    )
}

fn wsz(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Snapshot every adapter's counters. Filtering (loopback/tunnel/down) is left to the caller.
pub fn snapshot() -> Vec<IfRow> {
    let mut out = Vec::new();
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return out;
        }
        let n = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
        for r in rows {
            // InterfaceAndOperStatusFlags bitfield: bit 0 HardwareInterface, bit 1 FilterInterface.
            let flags = r.InterfaceAndOperStatusFlags._bitfield;
            out.push(IfRow {
                guid: guid_braces(&r.InterfaceGuid),
                alias: wsz(&r.Alias),
                if_type: r.Type,
                up: r.OperStatus.0 == IF_OPER_STATUS_UP,
                filter: (flags & 0x02) != 0,
                hardware: (flags & 0x01) != 0,
                rx: r.InOctets,
                tx: r.OutOctets,
            });
        }
        FreeMibTable(table as *const c_void);
    }
    out
}

/// Round-trip time in ms to `host` via kernel ICMP, or -1 on any failure. Blocks up to `timeout_ms`.
pub fn icmp_ping(host: &str, timeout_ms: u32) -> i32 {
    let Some(v4) = resolve_v4(host) else { return -1 };
    let dest = u32::from_le_bytes(v4); // IPADDR is network-order bytes packed into a u32
    unsafe {
        let Ok(handle) = IcmpCreateFile() else { return -1 };
        if handle == HANDLE(usize::MAX as *mut c_void) || handle.is_invalid() {
            return -1;
        }
        let req = [0x61u8; 32];
        let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + req.len() + 8];
        let count = IcmpSendEcho(
            handle,
            dest,
            req.as_ptr() as *const c_void,
            req.len() as u16,
            None,
            reply.as_mut_ptr() as *mut c_void,
            reply.len() as u32,
            timeout_ms,
        );
        let ms = if count > 0 {
            // reply is a Vec<u8> (align 1); read the fields unaligned rather than form a
            // misaligned &ICMP_ECHO_REPLY.
            let p = reply.as_ptr() as *const ICMP_ECHO_REPLY;
            let status = std::ptr::read_unaligned(std::ptr::addr_of!((*p).Status));
            if status == 0 {
                std::ptr::read_unaligned(std::ptr::addr_of!((*p).RoundTripTime)) as i32
            } else {
                -1
            }
        } else {
            -1
        };
        let _ = IcmpCloseHandle(handle);
        ms
    }
}

fn resolve_v4(host: &str) -> Option<[u8; 4]> {
    // Accept a bare host; append a dummy port so ToSocketAddrs resolves it.
    (host, 0u16)
        .to_socket_addrs()
        .ok()?
        .find_map(|a| match a {
            std::net::SocketAddr::V4(v4) => Some(v4.ip().octets()),
            _ => None,
        })
}
