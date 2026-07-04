//! Minimal HTTPS GET over WinHTTP (OS schannel TLS, zero extra deps).
//! Only used for the GitHub update check, geolocation fallback, and asset download.

use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Url {
    host: Vec<u16>,
    path: Vec<u16>,
    port: u16,
    secure: bool,
}

fn crack(url: &str) -> Option<Url> {
    let (rest, secure) = url
        .strip_prefix("https://")
        .map(|r| (r, true))
        .or_else(|| url.strip_prefix("http://").map(|r| (r, false)))?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host.rfind(':') {
        Some(i) => (&host[..i], host[i + 1..].parse().ok()?),
        None => (host, if secure { 443 } else { 80 }),
    };
    Some(Url { host: wide(host), path: wide(path), port, secure })
}

/// Streaming GET; calls `sink` per chunk. Returns false on any failure or non-2xx.
fn get(url: &str, extra_headers: Option<&str>, timeout_ms: i32, sink: &mut dyn FnMut(&[u8]) -> bool) -> bool {
    get_ex(url, extra_headers, timeout_ms, sink, None)
}

fn get_ex(
    url: &str,
    extra_headers: Option<&str>,
    timeout_ms: i32,
    sink: &mut dyn FnMut(&[u8]) -> bool,
    mut final_url: Option<&mut String>,
) -> bool {
    let Some(u) = crack(url) else { return false };
    unsafe {
        let session = Handle(WinHttpOpen(
            w!("PrayerTray"), // GitHub API rejects UA-less requests
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if session.0.is_null() {
            return false;
        }
        let _ = WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms);

        let conn = Handle(WinHttpConnect(session.0, PCWSTR(u.host.as_ptr()), u.port, 0));
        if conn.0.is_null() {
            return false;
        }
        let req = Handle(WinHttpOpenRequest(
            conn.0,
            w!("GET"),
            PCWSTR(u.path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null_mut(),
            if u.secure { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) },
        ));
        if req.0.is_null() {
            return false;
        }
        if let Some(h) = extra_headers {
            let hw: Vec<u16> = h.encode_utf16().collect();
            let _ = WinHttpAddRequestHeaders(req.0, &hw, WINHTTP_ADDREQ_FLAG_ADD);
        }
        if WinHttpSendRequest(req.0, None, None, 0, 0, 0).is_err()
            || WinHttpReceiveResponse(req.0, std::ptr::null_mut()).is_err()
        {
            return false;
        }

        let mut status = 0u32;
        let mut len = 4u32;
        if WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut core::ffi::c_void),
            &mut len,
            std::ptr::null_mut(),
        )
        .is_err()
            || !(200..300).contains(&status)
        {
            return false;
        }

        if let Some(out) = final_url.as_deref_mut() {
            let mut ubuf = [0u16; 2048];
            let mut ulen = (ubuf.len() * 2) as u32;
            if WinHttpQueryOption(
                req.0,
                WINHTTP_OPTION_URL,
                Some(ubuf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut ulen,
            )
            .is_ok()
            {
                *out = String::from_utf16_lossy(&ubuf[..(ulen as usize / 2).saturating_sub(1).min(ubuf.len())]);
            }
        }

        let mut buf = [0u8; 16384];
        loop {
            let mut read = 0u32;
            if WinHttpReadData(req.0, buf.as_mut_ptr() as *mut core::ffi::c_void, buf.len() as u32, &mut read)
                .is_err()
            {
                return false;
            }
            if read == 0 {
                return true;
            }
            if !sink(&buf[..read as usize]) {
                return false;
            }
        }
    }
}

/// GET returning (final URL after redirects, body) — used to resolve map short-links.
pub fn get_with_final_url(url: &str) -> Option<(String, String)> {
    // WinHTTP follows redirects by default; WINHTTP_OPTION_URL yields the final URL.
    let mut body = Vec::new();
    let mut final_url = String::new();
    let ok = get_ex(url, None, 8_000, &mut |chunk| {
        body.extend_from_slice(chunk);
        true
    }, Some(&mut final_url));
    ok.then(|| (final_url, String::from_utf8_lossy(&body).into_owned()))
}

/// GET returning the body as UTF-8 (8 s timeouts like the C# HttpClient).
pub fn get_string(url: &str, extra_headers: Option<&str>) -> Option<String> {
    let mut body = Vec::new();
    get(url, extra_headers, 8_000, &mut |chunk| {
        body.extend_from_slice(chunk);
        true
    })
    .then(|| String::from_utf8_lossy(&body).into_owned())
}

/// Download to a file; false on any failure (partial file deleted). Generous timeout for big assets.
pub fn download(url: &str, dest: &std::path::Path) -> bool {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create(dest) else { return false };
    let ok = get(url, None, 600_000, &mut |chunk| f.write_all(chunk).is_ok());
    drop(f);
    if !ok {
        let _ = std::fs::remove_file(dest);
    }
    ok
}
