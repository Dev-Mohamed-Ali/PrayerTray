using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;
using Microsoft.Win32;

namespace PrayerTray;

/// <summary>Owns the taskbar widget, tray icon, popup, timers, and notifications.</summary>
public class AppHost : ApplicationContext
{
    static readonly (string key, string label)[] Order =
    {
        ("fajr", "Fajr"), ("sunrise", "Sunrise"), ("dhuhr", "Dhuhr"),
        ("asr", "Asr"), ("maghrib", "Maghrib"), ("isha", "Isha"),
    };
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string AppName = "PrayerTray";

    readonly NotifyIcon _tray;
    readonly PrayerPopup _popup = new();
    readonly TaskbarWatcher _watcher = new();
    readonly System.Windows.Forms.Timer _data = new() { Interval = 15_000 };
    readonly System.Windows.Forms.Timer _pos = new() { Interval = 1_000 };
    TaskbarWidget _widget = new();
    AppConfig _cfg;

    Dictionary<string, TimeSpan> _times = new();
    DateTime _timesDate = DateTime.MinValue;
    DateTime _popupHiddenAt = DateTime.MinValue;
    string _lastAnnouncedId = "";

    public AppHost()
    {
        _cfg = AppConfig.Load();

        var menu = BuildMenu();
        _tray = new NotifyIcon
        {
            Visible = true,
            Icon = LoadAppIcon(),
            Text = "Prayer Tray",
            ContextMenuStrip = menu,
        };
        _tray.MouseClick += (_, e) => { if (e.Button == MouseButtons.Left) TogglePopup(); };

        WireWidget();
        _popup.VisibleChanged += (_, _) => { if (!_popup.Visible) _popupHiddenAt = DateTime.Now; };
        _watcher.Recreated += RebuildWidget;

        _data.Tick += (_, _) => DataTick();
        _pos.Tick += (_, _) => _widget.SyncPosition();

        Recompute();
        DataTick();
        _widget.Show();
        _data.Start();
        _pos.Start();
    }

    void WireWidget()
    {
        _widget.Clicked += TogglePopup;
        _widget.RightClicked += pt => _tray.ContextMenuStrip?.Show(pt);
        ApplyWidgetConfig();
    }

    void ApplyWidgetConfig()
    {
        _widget.AnchorRight = !string.Equals(_cfg.WidgetAnchor, "Left", StringComparison.OrdinalIgnoreCase);
        _widget.Offset = _cfg.WidgetOffset;
    }

    static Icon LoadAppIcon()
    {
        try
        {
            using var s = typeof(AppHost).Assembly.GetManifestResourceStream("PrayerTray.app.ico");
            if (s != null) return new Icon(s);
        }
        catch { /* fall back */ }
        return IconRenderer.Render("☾");
    }

    ContextMenuStrip BuildMenu()
    {
        var menu = new ContextMenuStrip();
        menu.Items.Add("Show times", null, (_, _) => ShowPopup());
        menu.Items.Add(new ToolStripSeparator());
        var startup = new ToolStripMenuItem("Start with Windows") { Checked = IsStartupEnabled() };
        startup.Click += (_, _) => { SetStartup(!startup.Checked); startup.Checked = IsStartupEnabled(); };
        menu.Items.Add(startup);
        menu.Items.Add("Settings…", null, (_, _) => OpenSettings());
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Exit", null, (_, _) => ExitApp());
        return menu;
    }

    void TogglePopup()
    {
        if (_popup.Visible) _popup.Hide();
        else if ((DateTime.Now - _popupHiddenAt).TotalMilliseconds > 250) ShowPopup();
    }

    void ShowPopup()
    {
        EnsureToday();
        var (nextKey, _, countdown) = NextPrayer();
        _popup.ShowTimes(_cfg.City, DateTime.Today, _times, nextKey, _cfg.Use24Hour, countdown,
            _widget.ScreenRect, _widget.AnchorRight);
    }

    void OpenSettings()
    {
        using var form = new SettingsForm(_cfg);
        if (form.ShowDialog() == DialogResult.OK)
        {
            _cfg = AppConfig.Load();
            ApplyWidgetConfig();
            Recompute();
            DataTick();
        }
    }

    void Recompute()
    {
        var today = DateTime.Today;
        _times = PrayerCalculator.Compute(today, _cfg.Latitude, _cfg.Longitude,
            _cfg.ResolveTimezone(today), _cfg.CalcMethod, (AsrJuristic)_cfg.Asr);
        _timesDate = today;
    }

    void EnsureToday() { if (_timesDate != DateTime.Today || _times.Count == 0) Recompute(); }

    void DataTick()
    {
        EnsureToday();
        var (nextKey, _, countdown) = NextPrayer();
        string label = nextKey is null ? "?" : LabelOf(nextKey);
        string timeStr = nextKey is null ? "" : PrayerPopup.Format(_times[nextKey], _cfg.Use24Hour);

        _widget.SetData(label, timeStr, countdown);
        if (!_widget.Visible) _widget.Show();
        _tray.Text = Trunc($"Next: {label} {timeStr} (in {countdown})");

        if (_popup.Visible) ShowPopup();
        CheckNotification();
    }

    // Edge-triggered, day-aware: announce a prayer once, within ~90s of it starting.
    void CheckNotification()
    {
        var now = DateTime.Now;
        foreach (var (key, label) in Order)
        {
            if (key == "sunrise" || !_times.TryGetValue(key, out var ts)) continue;
            double since = (now - DateTime.Today.Add(ts)).TotalSeconds;
            if (since < 0 || since >= 90) continue;
            string id = $"{now:yyyyMMdd}:{key}";
            if (_lastAnnouncedId == id) return;
            _lastAnnouncedId = id;
            _tray.ShowBalloonTip(8000, "Prayer time",
                $"It is now {label} ({PrayerPopup.Format(ts, _cfg.Use24Hour)})", ToolTipIcon.Info);
            return;
        }
    }

    (string? key, DateTime? at, string countdown) NextPrayer()
    {
        var now = DateTime.Now;
        foreach (var (key, _) in Order)
        {
            if (key == "sunrise") continue;
            var at = DateTime.Today.Add(_times[key]);
            if (at > now) return (key, at, FormatCountdown(at - now));
        }
        var tomorrow = DateTime.Today.AddDays(1);
        var t = PrayerCalculator.Compute(tomorrow, _cfg.Latitude, _cfg.Longitude,
            _cfg.ResolveTimezone(tomorrow), _cfg.CalcMethod, (AsrJuristic)_cfg.Asr);
        var fajrAt = tomorrow.Add(t["fajr"]);
        return ("fajr", fajrAt, FormatCountdown(fajrAt - now));
    }

    static string FormatCountdown(TimeSpan span)
    {
        if (span.TotalMinutes < 1) return "now";
        if (span.TotalMinutes < 60) return $"{(int)span.TotalMinutes}m";
        return $"{(int)span.TotalHours}:{span.Minutes:00}";
    }

    static string LabelOf(string key)
    {
        foreach (var (k, l) in Order) if (k == key) return l;
        return key;
    }

    static string Trunc(string s) => s.Length <= 127 ? s : s[..127];

    void RebuildWidget()
    {
        _widget.Clicked -= TogglePopup;
        _widget.Dispose();
        _widget = new TaskbarWidget();
        WireWidget();
        DataTick();
    }

    static bool IsStartupEnabled()
    {
        using var k = Registry.CurrentUser.OpenSubKey(RunKey);
        return k?.GetValue(AppName) != null;
    }

    static void SetStartup(bool enable)
    {
        using var k = Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
        if (k is null) return;
        if (enable) k.SetValue(AppName, $"\"{Environment.ProcessPath}\"");
        else k.DeleteValue(AppName, throwOnMissingValue: false);
    }

    void ExitApp()
    {
        _data.Stop();
        _pos.Stop();
        _tray.Visible = false;
        _tray.Dispose();
        _watcher.Dispose();
        _widget.Dispose();
        _popup.Dispose();
        ExitThread();
    }
}
