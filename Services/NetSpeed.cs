using System;
using System.Net.NetworkInformation;

namespace PrayerTray.Services;

/// <summary>Live NIC throughput. Sample ~1/s; returns down/up bytes-per-second since the last sample.</summary>
static class NetSpeed
{
    static long _rx, _tx, _lastTick;
    static bool _primed;

    public static (long down, long up) Sample()
    {
        long rx = 0, tx = 0;
        foreach (var ni in NetworkInterface.GetAllNetworkInterfaces())
        {
            if (ni.OperationalStatus != OperationalStatus.Up) continue;
            if (ni.NetworkInterfaceType is NetworkInterfaceType.Loopback or NetworkInterfaceType.Tunnel) continue;
            try { var s = ni.GetIPv4Statistics(); rx += s.BytesReceived; tx += s.BytesSent; }
            catch { /* some virtual NICs throw on stats — skip */ }
        }

        long now = Environment.TickCount64;
        if (!_primed) { _rx = rx; _tx = tx; _lastTick = now; _primed = true; return (0, 0); }
        double secs = (now - _lastTick) / 1000.0;
        if (secs <= 0) return (0, 0);
        long down = (long)Math.Max(0, (rx - _rx) / secs);
        long up = (long)Math.Max(0, (tx - _tx) / secs);
        _rx = rx; _tx = tx; _lastTick = now;
        return (down, up);
    }

    public static string Format(long down, long up) => $"↓ {Rate(down)}  ↑ {Rate(up)}";

    // Widest the value ever draws — reserve this in the pill so the per-second number doesn't jitter its width.
    public const string Template = "↓ 999.9 MB/s  ↑ 999.9 MB/s";

    static string Rate(long bps)
    {
        if (bps < 1024) return $"{bps} B/s";
        double v = bps / 1024.0;
        if (v < 1024) return $"{Num(v)} KB/s";
        v /= 1024.0;
        if (v < 1024) return $"{Num(v)} MB/s";
        return $"{Num(v / 1024.0)} GB/s";
    }

    // One decimal under 10, none above; invariant culture (InvariantGlobalization) keeps it Western "."
    static string Num(double v) => v < 10 ? v.ToString("0.0") : v.ToString("0");
}
