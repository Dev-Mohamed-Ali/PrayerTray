//! Manual update check against the GitHub latest release, port of Services/UpdateChecker.cs.

use super::http;

pub const RELEASES_URL: &str = "https://github.com/Dev-Mohamed-Ali/PrayerTray/releases/latest";
const LATEST_API: &str = "https://api.github.com/repos/Dev-Mohamed-Ali/PrayerTray/releases/latest";

const WANTED_ASSET: &str = "PrayerTray-win-x64.exe";

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub latest: (u32, u32, u32),
    pub url: String,
    pub asset_url: Option<String>,
}

pub fn current() -> (u32, u32, u32) {
    parse_version(env!("CARGO_PKG_VERSION")).unwrap_or((1, 0, 0))
}

/// Dev builds carry 0.0.0 (CI injects the real tag); never self-update those.
pub fn is_dev_build() -> bool {
    current() <= (1, 0, 0)
}

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim_start_matches(['v', 'V']);
    let mut it = s.split('.').map(|p| p.parse::<u32>());
    let major = it.next()?.ok()?;
    let minor = it.next().and_then(|r| r.ok()).unwrap_or(0);
    let build = it.next().and_then(|r| r.ok()).unwrap_or(0);
    Some((major, minor, build))
}

/// Latest release, or None on any network/parse failure. Blocking — call off the UI thread.
pub fn fetch_latest() -> Option<UpdateInfo> {
    let body = http::get_string(LATEST_API, Some("Accept: application/vnd.github+json"))?;
    let root: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = root.get("tag_name")?.as_str()?;
    let latest = parse_version(tag)?;
    let url = root.get("html_url").and_then(|u| u.as_str()).unwrap_or(RELEASES_URL).to_string();

    let asset_url = root.get("assets").and_then(|a| a.as_array()).and_then(|assets| {
        assets.iter().find_map(|a| {
            (a.get("name")?.as_str()? == WANTED_ASSET)
                .then(|| a.get("browser_download_url")?.as_str().map(str::to_string))
                .flatten()
        })
    });
    Some(UpdateInfo { latest, url, asset_url })
}

pub fn is_newer(info: &UpdateInfo) -> bool {
    !is_dev_build() && info.latest > current()
}

/// Blocking download — call off the UI thread.
pub fn download(url: &str, dest: &std::path::Path) -> bool {
    http::download(url, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parse_and_compare() {
        assert_eq!(parse_version("v1.14.0"), Some((1, 14, 0)));
        assert_eq!(parse_version("2.0"), Some((2, 0, 0)));
        assert!(parse_version("abc").is_none());
        // 3-part normalize semantics: 1.13.0 < 1.14.0 regardless of a 4th part.
        assert!((1, 13, 0) < (1, 14, 0));
    }
}
