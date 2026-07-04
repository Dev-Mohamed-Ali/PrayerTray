using System;
using System.Collections.Generic;
using System.Net.NetworkInformation;

namespace PrayerTray.Services;

/// <summary>Live NIC throughput. Sample ~1/s; returns down/up bytes-per-second since the last sample.</summary>
static class NetSpeed
{
    static readonly Dictionary<string, (long rx, long tx)> _base = new();
    static long _lastTick;
    static string? _ifaceId;

    /// <summary>Restrict counters to one adapter (null = all). VPN TUN adapters otherwise double-count.</summary>
    public static void SetInterface(string? id)
    {
        if (id != _ifaceId) { _ifaceId = id; _base.Clear(); }
    }

    public static (long down, long up) Sample()
    {
        long now = Environment.TickCount64;
        double secs = (now - _lastTick) / 1000.0;
        _lastTick = now;
        var (dr, dt) = Delta(_base, _ifaceId);
        // First sample or a long stall (sleep/resume) -> prime only, no rate.
        if (secs <= 0 || secs > 10) return (0, 0);
        return ((long)(dr / secs), (long)(dt / secs));
    }

    /// <summary>
    /// Per-adapter rx/tx deltas against <paramref name="baseline"/> (updated in place).
    /// New/reappeared adapters and counter resets prime the baseline without contributing —
    /// a flapping NIC can't inject its since-boot totals as one giant delta.
    /// </summary>
    internal static (long dr, long dt) Delta(Dictionary<string, (long rx, long tx)> baseline, string? ifaceId)
    {
        long dr = 0, dt = 0;
        foreach (var ni in NetworkInterface.GetAllNetworkInterfaces())
        {
            if (ifaceId != null)
            {
                if (ni.Id != ifaceId) continue; // explicit pick wins, even for Tunnel-type adapters
            }
            else if (ni.NetworkInterfaceType is NetworkInterfaceType.Loopback or NetworkInterfaceType.Tunnel)
                continue;
            if (ni.OperationalStatus != OperationalStatus.Up) continue;
            long rx, tx;
            try { var s = ni.GetIPv4Statistics(); rx = s.BytesReceived; tx = s.BytesSent; }
            catch { continue; /* some virtual NICs throw on stats */ }
            if (baseline.TryGetValue(ni.Id, out var b) && rx >= b.rx && tx >= b.tx)
            { dr += rx - b.rx; dt += tx - b.tx; }
            baseline[ni.Id] = (rx, tx);
        }
        return (dr, dt);
    }

    public static (string down, string up) FormatParts(long down, long up) =>
        ($"↓ {Rate(down)}", $"↑ {Rate(up)}");

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
