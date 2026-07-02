using System;
using System.Net.Http;
using System.Reflection;
using System.Text.Json;
using System.Threading.Tasks;

namespace PrayerTray.Services;

public record UpdateInfo(Version Latest, string Url);

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
            return new UpdateInfo(latest, url);
        }
        catch { return null; }
    }

    public static bool IsNewer(UpdateInfo info) => !IsDevBuild && Normalize(info.Latest) > Normalize(Current);

    // Version.Parse("1.13.0") leaves Revision = -1 while the assembly has 4 parts; compare 3 parts.
    static Version Normalize(Version v) => new(v.Major, v.Minor, Math.Max(0, v.Build));
}
