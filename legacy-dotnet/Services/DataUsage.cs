using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace PrayerTray.Services;

/// <summary>Daily traffic accumulator persisted to usage.json (kept ~90 days). Tick ~1/s.</summary>
static class DataUsage
{
    public record Day(long Rx, long Tx);

    static Dictionary<string, Day> _days = new();
    static readonly Dictionary<string, (long rx, long tx)> _base = new();
    static long _lastTick, _lastSave;
    static bool _dirty;
    static string? _ifaceId;

    const int KeepDays = 90;

    static string FilePath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "PrayerTray", "usage.json");

    public static void SetInterface(string? id)
    {
        if (id != _ifaceId) { _ifaceId = id; _base.Clear(); }
    }

    public static void Load()
    {
        try
        {
            if (File.Exists(FilePath))
                _days = JsonSerializer.Deserialize<Dictionary<string, Day>>(File.ReadAllText(FilePath)) ?? new();
        }
        catch { _days = new(); /* corrupt store -> start fresh */ }
    }

    public static void Tick()
    {
        long now = Environment.TickCount64;
        // A gap (sleep, tracking toggled off) means the baselines are stale; re-prime
        // rather than attribute the whole untracked window to this one tick.
        if (_lastTick > 0 && now - _lastTick > 10_000) _base.Clear();
        _lastTick = now;

        var (dr, dt) = NetSpeed.Delta(_base, _ifaceId);
        if (dr > 0 || dt > 0)
        {
            string day = DateTime.Now.ToString("yyyy-MM-dd");
            var cur = _days.TryGetValue(day, out var d) ? d : new Day(0, 0);
            _days[day] = new Day(cur.Rx + dr, cur.Tx + dt);
            _dirty = true;
        }

        if (_dirty && now - _lastSave >= 300_000) Save();
    }

    public static (long rx, long tx) Today() =>
        _days.TryGetValue(DateTime.Now.ToString("yyyy-MM-dd"), out var d) ? (d.Rx, d.Tx) : (0, 0);

    /// <summary>History newest-first, including today.</summary>
    public static List<(string date, long rx, long tx)> History()
    {
        var list = new List<(string, long, long)>(_days.Count);
        foreach (var (date, d) in _days) list.Add((date, d.Rx, d.Tx));
        list.Sort((a, b) => string.CompareOrdinal(b.Item1, a.Item1));
        return list;
    }

    public static void Reset()
    {
        _days.Clear();
        _dirty = true;
        Save();
    }

    public static void Flush() { if (_dirty) Save(); }

    static void Save()
    {
        _lastSave = Environment.TickCount64; // even on failure, retry at the normal cadence
        try
        {
            string cutoff = DateTime.Now.AddDays(-KeepDays).ToString("yyyy-MM-dd");
            var stale = new List<string>();
            foreach (var k in _days.Keys) if (string.CompareOrdinal(k, cutoff) < 0) stale.Add(k);
            foreach (var k in stale) _days.Remove(k);

            Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
            File.WriteAllText(FilePath, JsonSerializer.Serialize(_days));
            _dirty = false;
        }
        catch { /* best effort; retried on the next interval */ }
    }

    /// <summary>"512 B" / "3.25 KB" / "1.2 GB" — invariant digits, 3 significant figures.</summary>
    public static string Size(long bytes)
    {
        if (bytes < 1024) return $"{bytes} B";
        double v = bytes / 1024.0;
        if (v < 1024) return $"{Num(v)} KB";
        v /= 1024.0;
        if (v < 1024) return $"{Num(v)} MB";
        v /= 1024.0;
        if (v < 1024) return $"{Num(v)} GB";
        return $"{Num(v / 1024.0)} TB";
    }

    static string Num(double v) => v < 10 ? v.ToString("0.00") : v < 100 ? v.ToString("0.0") : v.ToString("0");
}
