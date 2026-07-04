using System;
using System.IO;
using System.Net.Http;
using System.Reflection;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace PrayerTray.Services;

public record UpdateInfo(Version Latest, string Url, string? AssetUrl);

/// <summary>Manual check against the GitHub latest release. Network is touched only on demand.</summary>
public static class UpdateChecker
{
    const string LatestApi = "https://api.github.com/repos/Dev-Mohamed-Ali/PrayerTray/releases/latest";
    public const string ReleasesUrl = "https://github.com/Dev-Mohamed-Ali/PrayerTray/releases/latest";

    static readonly HttpClient Http = CreateClient();

    static HttpClient CreateClient()
    {
        var c = new HttpClient { Timeout = TimeSpan.FromSeconds(8) };
        c.DefaultRequestHeaders.UserAgent.ParseAdd("PrayerTray"); // GitHub API rejects UA-less requests
        c.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        return c;
    }

    public static Version Current => Assembly.GetExecutingAssembly().GetName().Version ?? new Version(1, 0, 0, 0);

    // Dev builds carry the SDK-default 1.0.0.0 (CI injects the real tag via -p:Version).
    public static bool IsDevBuild => Current <= new Version(1, 0, 0, 0);

    // The CI publishes exactly these two assets; pick the one matching this build's variant.
    const string WantedAsset =
#if MANUAL_ONLY
        "PrayerTray-needs-dotnet8-win-x64.exe";
#else
        "PrayerTray-standalone-win-x64.exe";
#endif

    /// <summary>Latest release, or null on any network/parse failure.</summary>
    public static async Task<UpdateInfo?> FetchLatestAsync()
    {
        try
        {
            using var doc = JsonDocument.Parse(await Http.GetStringAsync(LatestApi));
            var root = doc.RootElement;
            string? tag = root.TryGetProperty("tag_name", out var t) ? t.GetString() : null;
            string url = root.TryGetProperty("html_url", out var u) ? u.GetString() ?? ReleasesUrl : ReleasesUrl;
            if (tag is null || !Version.TryParse(tag.TrimStart('v', 'V'), out var latest)) return null;

            string? asset = null;
            if (root.TryGetProperty("assets", out var assets) && assets.ValueKind == JsonValueKind.Array)
                foreach (var a in assets.EnumerateArray())
                    if (a.TryGetProperty("name", out var n) && n.GetString() == WantedAsset
                        && a.TryGetProperty("browser_download_url", out var bu))
                    { asset = bu.GetString(); break; }

            return new UpdateInfo(latest, url, asset);
        }
        catch { return null; }
    }

    /// <summary>Download an asset to <paramref name="dest"/>; false on any failure (partial file deleted).</summary>
    public static async Task<bool> DownloadAsync(string url, string dest)
    {
        try
        {
            // Per-request timeout is disabled (a 77 MB asset on slow links outlives any fixed
            // HttpClient.Timeout); the token caps the whole download so a stalled connection
            // can't hang the caller forever.
            using var c = new HttpClient { Timeout = Timeout.InfiniteTimeSpan };
            c.DefaultRequestHeaders.UserAgent.ParseAdd("PrayerTray");
            using var cts = new CancellationTokenSource(TimeSpan.FromMinutes(10));
            using var resp = await c.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, cts.Token);
            if (!resp.IsSuccessStatusCode) return false;
            await using (var fs = File.Create(dest))
                await resp.Content.CopyToAsync(fs, cts.Token);
            return true;
        }
        catch
        {
            try { if (File.Exists(dest)) File.Delete(dest); } catch { }
            return false;
        }
    }

    public static bool IsNewer(UpdateInfo info) => !IsDevBuild && Normalize(info.Latest) > Normalize(Current);

    // Version.Parse("1.13.0") leaves Revision = -1 while the assembly has 4 parts; compare 3 parts.
    static Version Normalize(Version v) => new(v.Major, v.Minor, Math.Max(0, v.Build));
}
