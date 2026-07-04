using System;
using System.IO;
using System.Text.Json;

namespace PrayerTray.Config;

/// <summary>Small persisted app state (not user settings) — survives restarts so reminders don't double-fire.</summary>
public class AppState
{
    public string SunnahFastNoticed { get; set; } = ""; // yyyy-MM-dd of the last eve-before fast notice

    static string Path => System.IO.Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "PrayerTray", "state.json");

    public static AppState Load()
    {
        try
        {
            if (File.Exists(Path))
                return JsonSerializer.Deserialize<AppState>(File.ReadAllText(Path)) ?? new AppState();
        }
        catch { /* corrupt -> defaults */ }
        return new AppState();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(System.IO.Path.GetDirectoryName(Path)!);
            File.WriteAllText(Path, JsonSerializer.Serialize(this));
        }
        catch { /* best effort */ }
    }
}
