using System;
using System.Net.NetworkInformation;
using System.Threading.Tasks;

namespace PrayerTray.Services;

/// <summary>Background latency probe. Sample() kicks a ping at most ~1/3s and returns the last result (ms, or -1).</summary>
static class Latency
{
    static volatile int _ms = -1;
    static volatile bool _inFlight;
    static long _lastSent;
    static string _host = "1.1.1.1";

    public static void SetHost(string? host)
    {
        host = string.IsNullOrWhiteSpace(host) ? "1.1.1.1" : host.Trim();
        if (host != _host) { _host = host; _ms = -1; }
    }

    public static int Sample()
    {
        long now = Environment.TickCount64;
        if (!_inFlight && now - _lastSent >= 3000)
        {
            _lastSent = now;
            _inFlight = true;
            try
            {
                var p = new Ping();
                p.SendPingAsync(_host, 2000).ContinueWith(t =>
                {
                    try
                    {
                        _ms = t.Status == TaskStatus.RanToCompletion && t.Result.Status == IPStatus.Success
                            ? (int)t.Result.RoundtripTime : -1;
                    }
                    catch { _ms = -1; }
                    finally { p.Dispose(); _inFlight = false; }
                });
            }
            catch { _inFlight = false; }
        }
        return _ms;
    }

    public static string Format(int ms) => ms < 0 ? "— ms" : $"{ms} ms";
}
