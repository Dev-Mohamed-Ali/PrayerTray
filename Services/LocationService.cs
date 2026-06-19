using System;
using System.Collections.Generic;
using System.Globalization;
using System.Net.Http;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Windows.Devices.Geolocation;

namespace PrayerTray.Services;

public record DetectedLocation(double Lat, double Lng, string? City, string? CountryIso, string Source);

/// <summary>Detects the user's location: Windows location service first (accurate), IP-geo as fallback.</summary>
public static class LocationService
{
    static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(5) };

    public static async Task<DetectedLocation?> DetectAsync(CancellationToken ct = default)
        => await TryWindowsLocation() ?? await TryIpGeo(ct);

    static async Task<DetectedLocation?> TryWindowsLocation()
    {
        try
        {
            if (await Geolocator.RequestAccessAsync() != GeolocationAccessStatus.Allowed) return null;
            var geo = new Geolocator { DesiredAccuracyInMeters = 3000 };
            var pos = await geo.GetGeopositionAsync(TimeSpan.FromMinutes(10), TimeSpan.FromSeconds(10));
            var p = pos.Coordinate.Point.Position;
            return new DetectedLocation(p.Latitude, p.Longitude, null, null, "Windows location");
        }
        catch { return null; }
    }

    /// <summary>IP-geo only — rough, used to pre-center the map view (never saved as-is).</summary>
    public static Task<DetectedLocation?> IpRoughAsync(CancellationToken ct = default) => TryIpGeo(ct);

    static async Task<DetectedLocation?> TryIpGeo(CancellationToken ct)
    {
        return await Query("https://ipapi.co/json/", "latitude", "longitude", "city", "country_code", ct)
            ?? await Query("https://freeipapi.com/api/json", "latitude", "longitude", "cityName", "countryCode", ct);
    }

    /// <summary>Google Maps URL, centered on the given point if supplied.</summary>
    public static string MapsUrl(double? lat, double? lng) =>
        lat is double a && lng is double o
            ? string.Format(CultureInfo.InvariantCulture, "https://www.google.com/maps/@{0},{1},12z", a, o)
            : "https://www.google.com/maps";

    static readonly Regex BareCoords = new(@"^\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*$", RegexOptions.Compiled);
    static readonly Regex UrlCoords = new(@"(?:@|[?&](?:q|query|ll|center)=)(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)", RegexOptions.Compiled);

    /// <summary>Parse coords from a pasted "lat, lng" pair or a Google Maps URL (resolving short links).</summary>
    public static async Task<DetectedLocation?> ParseAsync(string input, CancellationToken ct = default)
    {
        input = input?.Trim() ?? "";
        if (input.Length == 0) return null;

        if (Match(BareCoords, input) is { } bare) return bare;
        if (Match(UrlCoords, input) is { } direct) return direct;

        // Short share links (maps.app.goo.gl / goo.gl) carry no coords — follow the redirect, then re-scan.
        if (input.Contains("goo.gl", StringComparison.OrdinalIgnoreCase))
        {
            try
            {
                using var resp = await Http.GetAsync(input, ct);
                string resolved = resp.RequestMessage?.RequestUri?.ToString() ?? "";
                if (Match(UrlCoords, resolved) is { } viaUrl) return viaUrl;
                if (Match(UrlCoords, await resp.Content.ReadAsStringAsync(ct)) is { } viaBody) return viaBody;
            }
            catch { /* fall through */ }
        }
        return null;
    }

    static DetectedLocation? Match(Regex rx, string text)
    {
        var m = rx.Match(text);
        if (!m.Success) return null;
        if (!double.TryParse(m.Groups[1].Value, NumberStyles.Float, CultureInfo.InvariantCulture, out var lat) ||
            !double.TryParse(m.Groups[2].Value, NumberStyles.Float, CultureInfo.InvariantCulture, out var lng) ||
            lat < -90 || lat > 90 || lng < -180 || lng > 180)
            return null;
        return new DetectedLocation(lat, lng, null, null, "Map link");
    }

    static async Task<DetectedLocation?> Query(string url, string latK, string lngK, string cityK, string ctryK, CancellationToken ct)
    {
        try
        {
            using var doc = JsonDocument.Parse(await Http.GetStringAsync(url, ct));
            var root = doc.RootElement;
            if (!TryDouble(root, latK, out var lat) || !TryDouble(root, lngK, out var lng)) return null;
            string? city = root.TryGetProperty(cityK, out var c) ? c.GetString() : null;
            string? iso = root.TryGetProperty(ctryK, out var i) ? i.GetString() : null;
            return new DetectedLocation(lat, lng, city, iso, "Approximate (IP)");
        }
        catch { return null; }
    }

    // Some endpoints send numbers as JSON numbers, others as strings.
    static bool TryDouble(JsonElement root, string key, out double value)
    {
        value = 0;
        if (!root.TryGetProperty(key, out var e)) return false;
        if (e.ValueKind == JsonValueKind.Number) { value = e.GetDouble(); return true; }
        return e.ValueKind == JsonValueKind.String
            && double.TryParse(e.GetString(), NumberStyles.Float, CultureInfo.InvariantCulture, out value);
    }

    static readonly Dictionary<string, string> CountryMethod = new(StringComparer.OrdinalIgnoreCase)
    {
        ["EG"] = "Egypt",
        ["SA"] = "Makkah", ["AE"] = "Makkah", ["QA"] = "Makkah", ["KW"] = "Makkah",
        ["BH"] = "Makkah", ["OM"] = "Makkah", ["YE"] = "Makkah",
        ["US"] = "ISNA", ["CA"] = "ISNA",
        ["PK"] = "Karachi", ["IN"] = "Karachi", ["BD"] = "Karachi", ["AF"] = "Karachi",
    };

    /// <summary>Conventional calc method for a country; falls back to the Windows region, then MWL.</summary>
    public static string MethodForCountry(string? iso)
    {
        iso ??= TryRegionIso();
        return iso != null && CountryMethod.TryGetValue(iso, out var m) ? m : "MWL";
    }

    static string? TryRegionIso()
    {
        try { return RegionInfo.CurrentRegion.TwoLetterISORegionName; }
        catch { return null; }
    }
}
