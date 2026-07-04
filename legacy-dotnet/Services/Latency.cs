using System;
using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Threading;
using System.Threading.Tasks;

namespace PrayerTray.Services;

/// <summary>Background latency probe. Sample() kicks a ping at most ~1/3s and returns the last result (ms, or -1).</summary>
static class Latency
{
    static volatile int _ms = -1;
    static volatile bool _inFlight;
    static long _lastSent;
    static string _host = "1.1.1.1";
    static bool _tcp;
    static string? _ifaceId;

    public static void SetHost(string? host)
    {
        host = string.IsNullOrWhiteSpace(host) ? "1.1.1.1" : host.Trim();
        if (host != _host) { _host = host; _ms = -1; }
    }

    // TCP mode: the probe is a real app-owned socket, so per-process VPN/proxy rules apply to it
    // (kernel ICMP has no owning process). Optionally source-bound to the selected adapter.
    public static void SetMode(bool tcp, string? interfaceId)
    {
        if (tcp != _tcp || interfaceId != _ifaceId) { _tcp = tcp; _ifaceId = interfaceId; _ms = -1; }
    }

    public static int Sample()
    {
        long now = Environment.TickCount64;
        if (!_inFlight && now - _lastSent >= 3000)
        {
            _lastSent = now;
            _inFlight = true;
            if (_tcp) TcpProbe();
            else IcmpProbe();
        }
        return _ms;
    }

    static void IcmpProbe()
    {
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

    static void TcpProbe()
    {
        string host = _host;
        string? ifaceId = _ifaceId;
        Task.Run(async () =>
        {
            var sock = new Socket(AddressFamily.InterNetwork, SocketType.Stream, ProtocolType.Tcp)
            { NoDelay = true };
            try
            {
                if (LocalAddress(ifaceId) is { } local) sock.Bind(new IPEndPoint(local, 0));
                using var cts = new CancellationTokenSource(2000);
                long t0 = Environment.TickCount64;
                await sock.ConnectAsync(host, 443, cts.Token);
                _ms = (int)(Environment.TickCount64 - t0);
            }
            catch { _ms = -1; }
            finally
            {
                try { sock.Dispose(); } catch { }
                _inFlight = false;
            }
        });
    }

    static IPAddress? LocalAddress(string? ifaceId)
    {
        if (string.IsNullOrEmpty(ifaceId)) return null;
        try
        {
            foreach (var ni in NetworkInterface.GetAllNetworkInterfaces())
            {
                if (ni.Id != ifaceId || ni.OperationalStatus != OperationalStatus.Up) continue;
                foreach (var ua in ni.GetIPProperties().UnicastAddresses)
                    if (ua.Address.AddressFamily == AddressFamily.InterNetwork) return ua.Address;
            }
        }
        catch { /* enumeration hiccup -> unbound */ }
        return null;
    }

    public static string Format(int ms) => ms < 0 ? "— ms" : $"{ms} ms";
}
