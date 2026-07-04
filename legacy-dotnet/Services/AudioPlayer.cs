using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using PrayerTray.Config;

namespace PrayerTray.Services;

/// <summary>Fire-and-forget MP3/WAV playback via winmm MCI (no NuGet). Single track at a time.</summary>
internal static class AudioPlayer
{
    [DllImport("winmm.dll", CharSet = CharSet.Unicode)]
    static extern int mciSendString(string cmd, StringBuilder? ret, int retLen, IntPtr cb);

    const string Alias = "ptaudio";
    static readonly object _lock = new();
    static bool _open;

    static string TempDir
    {
        get
        {
            string d = Path.Combine(Path.GetTempPath(), "PrayerTray");
            Directory.CreateDirectory(d);
            return d;
        }
    }

    /// <summary>Start playing a file (async). Replaces any current track.</summary>
    public static void Play(string? path)
    {
        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path)) return;
        lock (_lock)
        {
            CloseLocked();
            string type = path.EndsWith(".wav", StringComparison.OrdinalIgnoreCase) ? "waveaudio" : "mpegvideo";
            if (mciSendString($"open \"{path}\" type {type} alias {Alias}", null, 0, IntPtr.Zero) != 0) return;
            _open = true;
            mciSendString($"play {Alias}", null, 0, IntPtr.Zero);
        }
    }

    public static void Stop()
    {
        lock (_lock) CloseLocked();
    }

    static void CloseLocked()
    {
        if (!_open) return;
        mciSendString($"close {Alias}", null, 0, IntPtr.Zero);
        _open = false;
    }

    // --- built-in adhans (embedded resources, optional) ---
    static readonly (string id, string label)[] AdhanCatalog =
    {
        ("makkah", "Makkah"), ("madinah", "Madinah"),
    };

    static List<(string id, string label)>? _adhans;
    static readonly Dictionary<string, string?> _adhanPaths = new();

    /// <summary>The adhans actually embedded in this build, in catalog order.</summary>
    public static IReadOnlyList<(string id, string label)> BuiltinAdhans
    {
        get
        {
            if (_adhans != null) return _adhans;
            var names = Assembly.GetExecutingAssembly().GetManifestResourceNames();
            _adhans = new();
            foreach (var a in AdhanCatalog)
                if (Array.Exists(names, n => n.EndsWith($"azan-{a.id}.mp3", StringComparison.OrdinalIgnoreCase)))
                    _adhans.Add(a);
            return _adhans;
        }
    }

    /// <summary>Extract a bundled adhan to temp (cached), or null if that id isn't embedded.</summary>
    public static string? BuiltinAdhanPath(string id)
    {
        if (_adhanPaths.TryGetValue(id, out var cached)) return cached;
        string res = $"azan-{id}.mp3";
        var path = Extract(res, res);
        _adhanPaths[id] = path;
        return path;
    }

    static string? Extract(string resourceSuffix, string fileName)
    {
        var asm = Assembly.GetExecutingAssembly();
        string? res = Array.Find(asm.GetManifestResourceNames(),
            n => n.EndsWith(resourceSuffix, StringComparison.OrdinalIgnoreCase));
        if (res == null) return null;
        string outPath = Path.Combine(TempDir, fileName);
        try
        {
            if (!File.Exists(outPath))
            {
                using var s = asm.GetManifestResourceStream(res)!;
                using var f = File.Create(outPath);
                s.CopyTo(f);
            }
            return outPath;
        }
        catch { return null; }
    }

    // --- reminder sounds: synthesized bank (no bundled assets) + custom file ---
    public static readonly (string id, string label)[] ReminderSounds =
    {
        ("chime", "Chime"), ("bell", "Bell"), ("ding", "Ding"),
        ("beep", "Beep"), ("double", "Double beep"),
    };

    static readonly Dictionary<string, string> _synthPaths = new();

    /// <summary>Play the reminder sound chosen in config (custom file, else a synth-bank tone).</summary>
    public static void PlayReminder(AppConfig cfg)
    {
        if (cfg.ReminderSoundId == "custom" && !string.IsNullOrWhiteSpace(cfg.ReminderSoundPath))
            Play(cfg.ReminderSoundPath);
        else
            Play(SynthPath(cfg.ReminderSoundId));
    }

    /// <summary>A short synthesized WAV for the given bank id, generated once into temp.</summary>
    public static string SynthPath(string id)
    {
        if (_synthPaths.TryGetValue(id, out var p) && File.Exists(p)) return p;
        string outPath = Path.Combine(TempDir, $"rem-{id}.wav");
        try { if (!File.Exists(outPath)) File.WriteAllBytes(outPath, BuildSound(id)); }
        catch { /* Play no-ops if missing */ }
        _synthPaths[id] = outPath;
        return outPath;
    }

    // (freq Hz, length ms, decay) blips, played in sequence. decay>0 = exponential ring-out (bell).
    static byte[] BuildSound(string id)
    {
        var blips = id switch
        {
            "bell" => new (double f, int ms, double dk)[] { (659.3, 900, 4.5) },
            "ding" => new[] { (1046.5, 350, 3.0) },
            "beep" => new[] { (880.0, 180, 0.0) },
            "double" => new[] { (988.0, 110, 0.0), (0.0, 70, 0.0), (988.0, 110, 0.0) },
            _ => new[] { (880.0, 160, 0.0), (1174.7, 240, 1.5) }, // chime
        };
        const int rate = 44100, bits = 16;
        int total = 0;
        foreach (var (_, ms2, _) in blips) total += rate * ms2 / 1000;
        int dataBytes = total * (bits / 8);

        using var ms = new MemoryStream();
        using var w = new BinaryWriter(ms);
        w.Write(Encoding.ASCII.GetBytes("RIFF"));
        w.Write(36 + dataBytes);
        w.Write(Encoding.ASCII.GetBytes("WAVE"));
        w.Write(Encoding.ASCII.GetBytes("fmt "));
        w.Write(16);
        w.Write((short)1); w.Write((short)1);   // PCM, mono
        w.Write(rate);
        w.Write(rate * (bits / 8));
        w.Write((short)(bits / 8)); w.Write((short)bits);
        w.Write(Encoding.ASCII.GetBytes("data"));
        w.Write(dataBytes);

        foreach (var (freq, msLen, decay) in blips)
        {
            int n = rate * msLen / 1000;
            int fade = rate * 8 / 1000; // 8 ms ramps to avoid clicks
            for (int i = 0; i < n; i++)
            {
                if (freq <= 0) { w.Write((short)0); continue; } // silence gap
                double env = decay > 0
                    ? Math.Exp(-decay * i / n) * Math.Min(1.0, i / (double)fade)
                    : Math.Min(1.0, Math.Min(i, n - i) / (double)fade);
                double sample = Math.Sin(2 * Math.PI * freq * i / rate) * env * 0.35;
                w.Write((short)(sample * short.MaxValue));
            }
        }
        return ms.ToArray();
    }
}
