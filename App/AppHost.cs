using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;
using Microsoft.Win32;
using PrayerTray.Calc;
using PrayerTray.Config;
using PrayerTray.Services;
using PrayerTray.UI;

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
    TaskbarWidget _widget;
    AppConfig _cfg;

    Dictionary<string, TimeSpan> _times = new();
    DateTime _timesDate = DateTime.MinValue;
    DateTime _popupHiddenAt = DateTime.MinValue;
    readonly HashSet<string> _fired = new();   // reminder/azan ids already fired today
    DateTime _firedDate = DateTime.MinValue;

    public AppHost()
    {
        _cfg = AppConfig.Load();
        ApplyTheme();
        _widget = new TaskbarWidget(_cfg.MonitorDeviceName);

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
        _popup.InitPin(_cfg.PopupPinned, _cfg.PopupX, _cfg.PopupY);
        _popup.PinChanged += pinned => { _cfg.PopupPinned = pinned; _cfg.Save(); };
        _popup.Moved += (x, y) => { _cfg.PopupX = x; _cfg.PopupY = y; _cfg.Save(); };
        _watcher.Recreated += RebuildWidget;
        _watcher.ThemeChanged += OnSystemThemeChanged;

        _data.Tick += (_, _) => DataTick();
        _pos.Tick += (_, _) => _widget.Tick();

        Recompute();
        DataTick();
        _widget.Show();
        _data.Start();
        _pos.Start();
        if (_cfg.PopupPinned) ShowPopup();

        if (AppConfig.IsFirstRun) ScheduleFirstRunDetect();
    }

    void ApplyTheme()
    {
        Theme.Apply(_cfg.Theme);
        Theme.Family = _cfg.FontFamily;
        Theme.FontScale = _cfg.FontScale;
    }

    // Settings live preview: re-apply the (in-place mutated) config to the pill without leaving pause —
    // monitor changes are deferred to Save (RebuildWidget), so we just reposition + re-render here.
    void LivePreview()
    {
        ApplyTheme();
        ApplyWidgetConfig();
        Recompute();
        DataTick();
        _widget.PreviewReposition();
    }

    // Defer to a one-shot timer: the message loop must be running before we can await on the UI thread.
    void ScheduleFirstRunDetect()
    {
        var t = new System.Windows.Forms.Timer { Interval = 200 };
        t.Tick += async (_, _) => { t.Stop(); t.Dispose(); await FirstRunDetect(); };
        t.Start();
    }

    async System.Threading.Tasks.Task FirstRunDetect()
    {
        var loc = await LocationService.DetectAsync();
        _widget.Pause(); _pos.Stop(); _data.Stop();
        try
        {
        using var form = new SettingsForm(_cfg, LivePreview, loc);
        if (form.ShowDialog() == DialogResult.OK
            && !DeviceEquals(_widget.DeviceName, _cfg.MonitorDeviceName))
            RebuildWidget(); // monitor change is the only thing live preview defers
        }
        finally { _data.Start(); _pos.Start(); _widget.Resume(); }
    }

    // Live-follow Windows' light/dark flip, but only when the user left the theme on "Auto".
    void OnSystemThemeChanged()
    {
        if (!string.Equals(_cfg.Theme, "Auto", StringComparison.OrdinalIgnoreCase)) return;
        ApplyTheme();
        DataTick(); // re-renders the pill; re-shows the popup if open
    }

    void WireWidget()
    {
        _widget.Clicked += TogglePopup;
        _widget.RightClicked += pt => _tray.ContextMenuStrip?.Show(pt);
        ApplyWidgetConfig();
    }

    // Compare the widget's actual current monitor to config (null = primary) — immune to the
    // shared-_cfg mutation that SettingsForm.OnSave does in place.
    static bool DeviceEquals(string? a, string? b) =>
        string.Equals(a ?? "", b ?? "", StringComparison.OrdinalIgnoreCase);

    void ApplyWidgetConfig()
    {
        _widget.AnchorRight = !string.Equals(_cfg.WidgetAnchor, "Left", StringComparison.OrdinalIgnoreCase);
        _widget.Offset = _cfg.WidgetOffset;
        _widget.HideOnFullscreen = _cfg.HideOnFullscreen;
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
        menu.Items.Add("Stop sound", null, (_, _) => AudioPlayer.Stop());
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
        _widget.Pause(); _pos.Stop(); _data.Stop();
        try
        {
        using var form = new SettingsForm(_cfg, LivePreview);
        if (form.ShowDialog() == DialogResult.OK
            && !DeviceEquals(_widget.DeviceName, _cfg.MonitorDeviceName))
            RebuildWidget(); // monitor change is the only thing live preview defers
        }
        finally { _data.Start(); _pos.Start(); _widget.Resume(); }
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
        _tray.Text = Trunc($"Next: {label} {timeStr} (in {countdown})");

        if (_popup.Visible) ShowPopup();
        CheckNotification();
    }

    // Edge-triggered, day-aware: each reminder/azan fires once, within ~90s of its moment (tick = 15s).
    void CheckNotification()
    {
        var now = DateTime.Now;
        if (_firedDate != DateTime.Today) { _fired.Clear(); _firedDate = DateTime.Today; }

        foreach (var (key, label) in Order)
        {
            if (key == "sunrise" || !_times.TryGetValue(key, out var ts)) continue;
            var at = DateTime.Today.Add(ts);

            if (_cfg.ReminderEnabled && InWindow(now, at.AddMinutes(-_cfg.ReminderMinutes))
                && _fired.Add($"{now:yyyyMMdd}:{key}:rem"))
            {
                _tray.ShowBalloonTip(8000, "Prayer reminder",
                    $"{label} in {_cfg.ReminderMinutes} min ({PrayerPopup.Format(ts, _cfg.Use24Hour)})", ToolTipIcon.Info);
                if (_cfg.ReminderSound) AudioPlayer.PlayReminder(_cfg);
            }

            if (InWindow(now, at) && _fired.Add($"{now:yyyyMMdd}:{key}"))
            {
                _tray.ShowBalloonTip(8000, "Prayer time",
                    $"It is now {label} ({PrayerPopup.Format(ts, _cfg.Use24Hour)})", ToolTipIcon.Info);
                PlayAzan();
            }
        }
    }

    static bool InWindow(DateTime now, DateTime target)
    {
        double s = (now - target).TotalSeconds;
        return s >= 0 && s < 90;
    }

    void PlayAzan()
    {
        string mode = _cfg.AzanMode;
        if (mode == "Builtin") // legacy config -> first available builtin
            mode = AudioPlayer.BuiltinAdhans.Count > 0 ? AudioPlayer.BuiltinAdhans[0].id : "None";

        string? path = mode switch
        {
            "None" or "" => null,
            "Custom" => _cfg.AzanCustomPath,
            _ => AudioPlayer.BuiltinAdhanPath(mode),
        };
        if (path != null) AudioPlayer.Play(path);
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
        _widget = new TaskbarWidget(_cfg.MonitorDeviceName);
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
        AudioPlayer.Stop();
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
