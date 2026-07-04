//! Location detection, port of Services/LocationService.cs: Windows location service first
//! (accurate), IP-geo fallback. All functions block — call off the UI thread.

use super::http;

#[derive(Clone, Debug)]
pub struct DetectedLocation {
    pub lat: f64,
    pub lng: f64,
    pub city: Option<String>,
    pub country_iso: Option<String>,
    pub source: &'static str,
}

pub fn detect() -> Option<DetectedLocation> {
    try_windows_location().or_else(try_ip_geo)
}

/// IP-geo only — rough, used to pre-center the map view (never saved as-is).
pub fn ip_rough() -> Option<DetectedLocation> {
    try_ip_geo()
}

fn try_windows_location() -> Option<DetectedLocation> {
    use windows::core::Interface;
    use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator};
    use windows::Foundation::{IReference, PropertyValue};
    (|| -> windows::core::Result<Option<DetectedLocation>> {
        if Geolocator::RequestAccessAsync()?.join()? != GeolocationAccessStatus::Allowed {
            return Ok(None);
        }
        let geo = Geolocator::new()?;
        let accuracy: IReference<u32> = PropertyValue::CreateUInt32(3000)?.cast()?;
        geo.SetDesiredAccuracyInMeters(&accuracy)?;
        let pos = geo.GetGeopositionAsync()?.join()?;
        let p = pos.Coordinate()?.Point()?.Position()?;
        Ok(Some(DetectedLocation {
            lat: p.Latitude,
            lng: p.Longitude,
            city: None,
            country_iso: None,
            source: "Windows location",
        }))
    })()
    .ok()
    .flatten()
}

fn try_ip_geo() -> Option<DetectedLocation> {
    query("https://ipapi.co/json/", "latitude", "longitude", "city", "country_code")
        .or_else(|| query("https://freeipapi.com/api/json", "latitude", "longitude", "cityName", "countryCode"))
}

/// Google Maps URL, centered on the given point if supplied.
pub fn maps_url(coords: Option<(f64, f64)>) -> String {
    match coords {
        Some((lat, lng)) => format!("https://www.google.com/maps/@{lat},{lng},12z"),
        None => "https://www.google.com/maps".into(),
    }
}

/// Parse coords from a pasted "lat, lng" pair or a Google Maps URL (resolving short links).
pub fn parse(input: &str) -> Option<DetectedLocation> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    if let Some(loc) = bare_coords(input).or_else(|| url_coords(input)) {
        return Some(loc);
    }
    // Short share links carry no coords — follow the redirect, then re-scan.
    if input.to_ascii_lowercase().contains("goo.gl") {
        if let Some((final_url, body)) = http::get_with_final_url(input) {
            return url_coords(&final_url).or_else(|| url_coords(&body));
        }
    }
    None
}

fn valid(lat: f64, lng: f64) -> Option<DetectedLocation> {
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng)).then_some(DetectedLocation {
        lat,
        lng,
        city: None,
        country_iso: None,
        source: "Map link",
    })
}

/// "lat, lng" and nothing else.
fn bare_coords(s: &str) -> Option<DetectedLocation> {
    let (a, b) = s.split_once(',')?;
    valid(a.trim().parse().ok()?, b.trim().parse().ok()?)
}

/// Coords after `@` or a `q=`/`query=`/`ll=`/`center=` query parameter.
fn url_coords(s: &str) -> Option<DetectedLocation> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = match bytes[i] {
            b'@' => Some(i + 1),
            b'?' | b'&' => {
                let rest = &s[i + 1..];
                ["q=", "query=", "ll=", "center="]
                    .iter()
                    .find_map(|p| rest.starts_with(p).then(|| i + 1 + p.len()))
            }
            _ => None,
        };
        if let Some(start) = start {
            let tail = &s[start..];
            let end = tail
                .char_indices()
                .find(|(_, c)| !c.is_ascii_digit() && *c != '.' && *c != '-' && *c != ',')
                .map(|(j, _)| j)
                .unwrap_or(tail.len());
            let mut parts = tail[..end].split(','); // may carry a third piece like "12z"'s zoom
            if let (Some(a), Some(b)) = (parts.next(), parts.next()) {
                if let (Ok(lat), Ok(lng)) = (a.parse::<f64>(), b.parse::<f64>()) {
                    if let Some(loc) = valid(lat, lng) {
                        return Some(loc);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn query(url: &str, lat_k: &str, lng_k: &str, city_k: &str, ctry_k: &str) -> Option<DetectedLocation> {
    let body = http::get_string(url, None)?;
    let root: serde_json::Value = serde_json::from_str(&body).ok()?;
    let lat = try_double(&root, lat_k)?;
    let lng = try_double(&root, lng_k)?;
    Some(DetectedLocation {
        lat,
        lng,
        city: root.get(city_k).and_then(|v| v.as_str()).map(str::to_string),
        country_iso: root.get(ctry_k).and_then(|v| v.as_str()).map(str::to_string),
        source: "Approximate (IP)",
    })
}

/// Some endpoints send numbers as JSON numbers, others as strings.
fn try_double(root: &serde_json::Value, key: &str) -> Option<f64> {
    let v = root.get(key)?;
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// Conventional calc method for a country; falls back to the Windows region, then MWL.
pub fn method_for_country(iso: Option<&str>) -> &'static str {
    let owned;
    let iso = match iso {
        Some(i) => i,
        None => match region_iso() {
            Some(r) => {
                owned = r;
                &owned
            }
            None => return "MWL",
        },
    };
    match iso.to_ascii_uppercase().as_str() {
        "EG" => "Egypt",
        "SA" | "AE" | "QA" | "KW" | "BH" | "OM" | "YE" => "Makkah",
        "US" | "CA" => "ISNA",
        "PK" | "IN" | "BD" | "AF" => "Karachi",
        _ => "MWL",
    }
}

fn region_iso() -> Option<String> {
    use windows::Win32::Globalization::GetUserDefaultGeoName;
    let mut buf = [0u16; 16];
    let n = unsafe { GetUserDefaultGeoName(&mut buf) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..(n - 1) as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_coords() {
        assert!(bare_coords("21.42, 39.83").is_some());
        assert!(bare_coords("hello").is_none());
        assert!(url_coords("https://www.google.com/maps/@30.0444,31.2357,12z").is_some());
        assert!(url_coords("https://maps.google.com/?q=21.4,39.8").is_some());
        assert!(url_coords("https://example.com/?ll=-53.16,-70.91&z=4").is_some());
        assert!(url_coords("https://example.com/nothing").is_none());
        assert!(valid(95.0, 0.0).is_none());
    }
}
