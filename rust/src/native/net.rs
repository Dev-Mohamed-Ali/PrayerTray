//! Per-adapter byte counters (GetIfTable2), NIC enumeration for the settings picker,
//! and a one-shot ICMP echo. Replaces the C# System.Net.NetworkInformation surface.

use std::ffi::c_void;
use std::net::ToSocketAddrs;
use windows::core::GUID;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY,
    MIB_IF_TABLE2,
};

// IF_TYPE values (iftypes.h); the windows crate doesn't surface these as constants.
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_TYPE_TUNNEL: u32 = 131;
const IF_OPER_STATUS_UP: i32 = 1;

/// One network adapter's octet counters and identity, as needed for metering.
pub struct IfRow {
    pub guid: String, // braced, e.g. "{XXXXXXXX-...}" — matches .NET NetworkInterface.Id
    pub alias: String,
    pub if_type: u32,
    pub up: bool,
    pub filter: bool, // WFP callout pseudo-interface -> would double-count
    pub rx: u64,
    pub tx: u64,
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
            // Bit 1 of the InterfaceAndOperStatusFlags bitfield is FilterInterface.
            let filter = (r.InterfaceAndOperStatusFlags._bitfield & 0x02) != 0;
            out.push(IfRow {
                guid: guid_braces(&r.InterfaceGuid),
                alias: wsz(&r.Alias),
                if_type: r.Type,
                up: r.OperStatus.0 == IF_OPER_STATUS_UP,
                filter,
                rx: r.InOctets,
                tx: r.OutOctets,
            });
        }
        FreeMibTable(table as *const c_void);
    }
    out
}

/// (friendly name, guid) for the settings NIC combo. Only currently-up real adapters: drops
/// loopback, the WFP/QoS/NDIS filter pseudo-interfaces GetIfTable2 lists, and the Teredo/6to4/
/// IP-HTTPS transition tunnels — none of which .NET's GetAllNetworkInterfaces ever showed.
pub fn adapters() -> Vec<(String, String)> {
    snapshot()
        .into_iter()
        .filter(|r| {
            r.up && !r.filter
                && r.if_type != IF_TYPE_SOFTWARE_LOOPBACK
                && r.if_type != IF_TYPE_TUNNEL
        })
        .map(|r| (r.alias, r.guid))
        .collect()
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
