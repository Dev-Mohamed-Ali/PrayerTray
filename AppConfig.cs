using System;
using System.IO;
using System.Text.Json;

namespace PrayerTray;

public class AppConfig
{
    public string City { get; set; } = "Makkah";
    public double Latitude { get; set; } = 21.4225;
    public double Longitude { get; set; } = 39.8262;
    public string Method { get; set; } = "MWL";
    public int Asr { get; set; } = (int)AsrJuristic.Standard;
    public bool Use24Hour { get; set; } = false;
    public string WidgetAnchor { get; set; } = "Right"; // Left | Right
    public int WidgetOffset { get; set; } = 12;          // px gap from that edge (or from the tray)
    // 999 = use system timezone for the date (handles DST); else fixed UTC offset hours.
    public double TimezoneHours { get; set; } = 999;

    static string Path => System.IO.Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "PrayerTray", "config.json");

    public static bool IsFirstRun => !File.Exists(Path);

    static readonly JsonSerializerOptions JsonOpts = new() { WriteIndented = true };

    public static AppConfig Load()
    {
        try
        {
            if (File.Exists(Path))
                return JsonSerializer.Deserialize<AppConfig>(File.ReadAllText(Path)) ?? new AppConfig();
        }
        catch { /* corrupt config -> defaults */ }
        return new AppConfig();
    }

    public void Save()
    {
        Directory.CreateDirectory(System.IO.Path.GetDirectoryName(Path)!);
        File.WriteAllText(Path, JsonSerializer.Serialize(this, JsonOpts));
    }

    public double ResolveTimezone(DateTime date) =>
        TimezoneHours == 999 ? TimeZoneInfo.Local.GetUtcOffset(date).TotalHours : TimezoneHours;

    public CalcMethod CalcMethod =>
        CalcMethod.All.TryGetValue(Method, out var m) ? m : CalcMethod.All["MWL"];
}
