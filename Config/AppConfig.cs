using System;
using System.IO;
using System.Text.Json;
using PrayerTray.Calc;

namespace PrayerTray.Config;

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
    public string Theme { get; set; } = "Auto";          // Auto | Dark | Light | Midnight | Slate | Warm
    public string? MonitorDeviceName { get; set; }       // null = primary monitor's taskbar
    public bool HideOnFullscreen { get; set; } = true;   // hide the pill over fullscreen apps/games
    // 999 = use system timezone for the date (handles DST); else fixed UTC offset hours.
    public double TimezoneHours { get; set; } = 999;

    public string Language { get; set; } = "auto";       // auto | en | ar
    public bool ShowHijriDate { get; set; } = true;
    public int HijriAdjust { get; set; } = 0;            // moon-sighting offset, clamped -2..2 on save
    public string FontFamily { get; set; } = "Segoe UI";
    public int FontScalePct { get; set; } = 100;         // clamped 80–150 on save

    public bool PopupPinned { get; set; } = false;       // keep the times panel open
    public int PopupX { get; set; } = int.MinValue;      // int.MinValue = unset -> anchor to the pill
    public int PopupY { get; set; } = int.MinValue;

    public bool ReminderEnabled { get; set; } = false;
    public int ReminderMinutes { get; set; } = 10;       // clamped 1–60 on save
    public bool ReminderSound { get; set; } = true;
    public string ReminderSoundId { get; set; } = "chime"; // synth bank id, or "custom"
    public string? ReminderSoundPath { get; set; }       // custom file (when id == "custom")

    public string AzanMode { get; set; } = "None";       // None | <builtin id> | Custom
    public string? AzanCustomPath { get; set; }

    public float FontScale => Math.Clamp(FontScalePct, 80, 150) / 100f;

    public AppConfig Clone() => (AppConfig)MemberwiseClone();

    /// <summary>Copy all fields from another instance in place (used to revert on Cancel).</summary>
    public void CopyFrom(AppConfig o)
    {
        City = o.City; Latitude = o.Latitude; Longitude = o.Longitude; Method = o.Method; Asr = o.Asr;
        Use24Hour = o.Use24Hour; WidgetAnchor = o.WidgetAnchor; WidgetOffset = o.WidgetOffset; Theme = o.Theme;
        MonitorDeviceName = o.MonitorDeviceName; HideOnFullscreen = o.HideOnFullscreen; TimezoneHours = o.TimezoneHours;
        Language = o.Language; ShowHijriDate = o.ShowHijriDate; HijriAdjust = o.HijriAdjust;
        FontFamily = o.FontFamily; FontScalePct = o.FontScalePct;
        PopupPinned = o.PopupPinned; PopupX = o.PopupX; PopupY = o.PopupY;
        ReminderEnabled = o.ReminderEnabled; ReminderMinutes = o.ReminderMinutes; ReminderSound = o.ReminderSound;
        ReminderSoundId = o.ReminderSoundId; ReminderSoundPath = o.ReminderSoundPath;
        AzanMode = o.AzanMode; AzanCustomPath = o.AzanCustomPath;
    }

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
