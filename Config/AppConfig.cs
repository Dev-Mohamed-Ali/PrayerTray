using System;
using System.Collections.Generic;
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
    public string HighLats { get; set; } = "AngleBased"; // None | MidNight | OneSeventh | AngleBased
    // Per-prayer fine-tune in minutes (match the local mosque); clamped -60..60 on save.
    public int FajrAdjust { get; set; }
    public int DhuhrAdjust { get; set; }
    public int AsrAdjust { get; set; }
    public int MaghribAdjust { get; set; }
    public int IshaAdjust { get; set; }
    public bool Use24Hour { get; set; } = false;
    public string WidgetAnchor { get; set; } = "Right"; // Left | Right
    public int WidgetOffset { get; set; } = 12;          // px gap from that edge (or from the tray)
    public string Theme { get; set; } = "Auto";          // Auto | Dark | Light | Midnight | Slate | Warm
    public string? MonitorDeviceName { get; set; }       // null = primary monitor's taskbar
    public bool HideOnFullscreen { get; set; } = true;   // hide the pill over fullscreen apps/games
    public bool ShowNetSpeed { get; set; } = false;      // append live ↓/↑ throughput to the pill
    public bool ShowPing { get; set; } = false;          // append live ping latency (ms) to the pill
    public string PingHost { get; set; } = "1.1.1.1";    // host to ping for latency
    // 999 = use system timezone for the date (handles DST); else fixed UTC offset hours.
    public double TimezoneHours { get; set; } = 999;

    public string Language { get; set; } = "auto";       // auto | en | ar | fr | tr | ur | id
    public bool ShowHijriDate { get; set; } = true;
    public int HijriAdjust { get; set; } = 0;            // moon-sighting offset, clamped -2..2 on save
    public bool ShowIslamicEvents { get; set; } = true;  // popup line for special days / next major event
    public bool SunnahFastReminder { get; set; } = false; // eve-before balloon for Sunnah fasting days
    public bool FridayReminder { get; set; } = false;     // Friday: Al-Kahf at Fajr + Jumu'ah before Dhuhr
    public string FontFamily { get; set; } = "Segoe UI";
    public int FontScalePct { get; set; } = 100;         // clamped 80–150 on save

    public bool PopupPinned { get; set; } = false;       // keep the times panel open
    public int PopupX { get; set; } = int.MinValue;      // int.MinValue = unset -> anchor to the pill
    public int PopupY { get; set; } = int.MinValue;

    public bool RichToasts { get; set; } = true;        // prefer Action Center toasts over tray balloons
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
        HighLats = o.HighLats;
        FajrAdjust = o.FajrAdjust; DhuhrAdjust = o.DhuhrAdjust; AsrAdjust = o.AsrAdjust;
        MaghribAdjust = o.MaghribAdjust; IshaAdjust = o.IshaAdjust;
        Use24Hour = o.Use24Hour; WidgetAnchor = o.WidgetAnchor; WidgetOffset = o.WidgetOffset; Theme = o.Theme;
        MonitorDeviceName = o.MonitorDeviceName; HideOnFullscreen = o.HideOnFullscreen; ShowNetSpeed = o.ShowNetSpeed;
        ShowPing = o.ShowPing; PingHost = o.PingHost;
        TimezoneHours = o.TimezoneHours;
        Language = o.Language; ShowHijriDate = o.ShowHijriDate; HijriAdjust = o.HijriAdjust;
        ShowIslamicEvents = o.ShowIslamicEvents; SunnahFastReminder = o.SunnahFastReminder;
        FridayReminder = o.FridayReminder;
        FontFamily = o.FontFamily; FontScalePct = o.FontScalePct;
        PopupPinned = o.PopupPinned; PopupX = o.PopupX; PopupY = o.PopupY;
        RichToasts = o.RichToasts;
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

    public HighLatRule HighLat =>
        Enum.TryParse<HighLatRule>(HighLats, out var r) ? r : HighLatRule.AngleBased;

    public Dictionary<string, int> TimeAdjust() => new()
    {
        ["fajr"] = FajrAdjust, ["dhuhr"] = DhuhrAdjust, ["asr"] = AsrAdjust,
        ["maghrib"] = MaghribAdjust, ["isha"] = IshaAdjust,
    };
}
