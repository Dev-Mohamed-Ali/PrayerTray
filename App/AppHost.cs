using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;
using Microsoft.Win32;
using PrayerTray.Calc;
using PrayerTray.Config;
using PrayerTray.I18n;
using PrayerTray.Services;
using PrayerTray.UI;

namespace PrayerTray;

/// <summary>Owns the taskbar widget, tray icon, popup, timers, and notifications.</summary>
public class AppHost : ApplicationContext
{
    static readonly string[] Order = { "fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha" };
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
    DateTime? _nextAt;                          // cached target of the upcoming prayer (for the 1s seconds countdown)
    readonly System.Threading.SynchronizationContext _ui;

    public AppHost()
    {
        _ui = System.Threading.SynchronizationContext.Current ?? new System.Threading.SynchronizationContext();
        _cfg = AppConfig.Load();
        ToastService.Init();
        Strings.Init(_cfg.Language);
        ApplyTheme();
        _widget = new TaskbarWidget(_cfg.MonitorDeviceName);

        var menu = BuildMenu();
        _tray = new NotifyIcon
        {
            Visible = true,
            Icon = LoadAppIcon(),
            Text = Strings.T("app.name"),
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
        SystemEvents.TimeChanged += OnTimeChanged;
        SystemEvents.PowerModeChanged += OnPowerModeChanged;

        _data.Tick += (_, _) => DataTick();
        _pos.Tick += (_, _) => PosTick();

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
        RenderTick(); // display only — never fire azan/balloon from a settings change
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
        RunSettings(loc);
    }

    // SystemEvents fire on a private hidden-window thread; marshal back to the UI thread.
    void OnTimeChanged(object? s, EventArgs e) => _ui.Post(_ => RefreshNow(), null);
    void OnPowerModeChanged(object? s, PowerModeChangedEventArgs e)
    { if (e.Mode == PowerModes.Resume) _ui.Post(_ => RefreshNow(), null); }

    // Recompute and re-arm after a clock/timezone change, resume-from-sleep, or manual "Refresh now".
    void RefreshNow()
    {
        Recompute();
        _fired.Clear(); _firedDate = DateTime.Today;
        DataTick();
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
        Latency.SetHost(_cfg.PingHost);
        if (!_cfg.ShowNetSpeed && !_cfg.ShowPing) _widget.SetNet(""); // PosTick fills it when enabled
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
        var menu = new ContextMenuStrip { RightToLeft = Strings.IsRtl ? RightToLeft.Yes : RightToLeft.No };
        menu.Items.Add(Strings.T("menu.showTimes"), null, (_, _) => ShowPopup());
        menu.Items.Add(Strings.T("menu.refresh"), null, (_, _) => RefreshNow());
        menu.Items.Add(new ToolStripSeparator());
        var startup = new ToolStripMenuItem(Strings.T("menu.startup")) { Checked = IsStartupEnabled() };
        startup.Click += (_, _) => { SetStartup(!startup.Checked); startup.Checked = IsStartupEnabled(); };
        menu.Items.Add(startup);
        menu.Items.Add(Strings.T("menu.settings"), null, (_, _) => OpenSettings());
        menu.Items.Add(Strings.T("menu.stopSound"), null, (_, _) => AudioPlayer.Stop());
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(Strings.T("menu.exit"), null, (_, _) => ExitApp());
        return menu;
    }

    // Rebuild the tray menu after a language change (menu items are created once, in BuildMenu).
    void RebuildMenu()
    {
        var old = _tray.ContextMenuStrip;
        _tray.ContextMenuStrip = BuildMenu();
        old?.Dispose();
    }

    void TogglePopup()
    {
        if (_popup.Visible) _popup.Hide();
        else if ((DateTime.Now - _popupHiddenAt).TotalMilliseconds > 250) ShowPopup();
    }

    void ShowPopup()
    {
        EnsureToday();
        var (nextKey, _, countdown) = CurrentOrNext();
        string hijri = _cfg.ShowHijriDate ? Strings.FormatHijri(DateTime.Today, _cfg.HijriAdjust) : "";
        _popup.ShowTimes(_cfg.City, DateTime.Today, _times, nextKey, _cfg.Use24Hour, ShownCountdown(countdown),
            _widget.ScreenRect, _widget.AnchorRight, hijri, TodaysEvent());
    }

    // Popup line: today's special day, else a countdown to the next major event (within ~6 weeks).
    string TodaysEvent()
    {
        if (!_cfg.ShowIslamicEvents) return "";
        var today = IslamicEvents.ForDate(DateTime.Today, _cfg.HijriAdjust);
        if (today != null) return Strings.Event(today);
        if (IslamicEvents.NextMajor(DateTime.Today, _cfg.HijriAdjust) is { } n && n.days <= 45)
            return n.days == 1
                ? Strings.F("event.tomorrow", Strings.Event(n.key))
                : Strings.F("event.inDays", Strings.Event(n.key), n.days);
        return "";
    }

    void OpenSettings() => RunSettings(null);

    // Reopen-loop: a language change closes the dialog with Retry so it rebuilds in the new language/RTL.
    // The opening snapshot is carried across reopens so Cancel still reverts to the true original.
    void RunSettings(DetectedLocation? prefill)
    {
        _widget.Pause(); _pos.Stop(); _data.Stop();
        try
        {
            var orig = _cfg.Clone();
            bool restart;
            do
            {
                using var form = new SettingsForm(_cfg, LivePreview, prefill, orig, TestNotify);
                var result = form.ShowDialog();
                restart = form.RestartRequested;
                prefill = null; // only apply detected prefill on the first open
                RebuildMenu();  // language may have changed
                if (!restart && result == DialogResult.OK
                    && !DeviceEquals(_widget.DeviceName, _cfg.MonitorDeviceName))
                    RebuildWidget(); // monitor change is the only thing live preview defers
            } while (restart);
        }
        finally { _data.Start(); _pos.Start(); _widget.Resume(); }
    }

    void Recompute()
    {
        var today = DateTime.Today;
        _times = PrayerCalculator.Compute(today, _cfg.Latitude, _cfg.Longitude,
            _cfg.ResolveTimezone(today), _cfg.CalcMethod, (AsrJuristic)_cfg.Asr, _cfg.TimeAdjust(), _cfg.HighLat);
        _timesDate = today;
    }

    void EnsureToday() { if (_timesDate != DateTime.Today || _times.Count == 0) Recompute(); }

    void DataTick() { RenderTick(); CheckNotification(); }

    // 1s timer: reposition the pill, and within the final minute re-render so the seconds countdown ticks live.
    void PosTick()
    {
        if (_cfg.ShowNetSpeed || _cfg.ShowPing)
        {
            string tail = "";
            if (_cfg.ShowNetSpeed)
            {
                var (down, up) = NetSpeed.Sample();
                tail = NetSpeed.Format(down, up);
            }
            if (_cfg.ShowPing)
            {
                string p = Latency.Format(Latency.Sample());
                tail = tail.Length == 0 ? p : $"{tail}   {p}";
            }
            _widget.SetNet(tail);
        }
        _widget.Tick();
        if (_nextAt is DateTime a)
        {
            double s = (a - DateTime.Now).TotalSeconds;
            if (s > -60 && s <= 60) RenderTick(); // last minute (seconds countdown) + the "now" minute
        }
    }

    // Display-only refresh (no notifications) — safe to call from Settings live preview.
    void RenderTick()
    {
        EnsureToday();
        var (nextKey, at, countdown) = CurrentOrNext();
        _nextAt = at;
        bool isNow = countdown == "now";
        string label = nextKey is null ? "?" : LabelOf(nextKey);
        string timeStr = nextKey is null || !_times.TryGetValue(nextKey, out var ts)
            ? "" : PrayerPopup.Format(ts, _cfg.Use24Hour);

        _widget.SetData(label, timeStr, ShownCountdown(countdown));
        _tray.Text = Trunc(isNow
            ? $"{Strings.T("tray.now")} {label} {timeStr}"
            : $"{Strings.T("tray.next")} {label} {timeStr} ({Strings.T("tray.in")} {countdown})");

        if (_popup.Visible) ShowPopup();
    }

    // Map the internal "now" sentinel to a localized word; pass other countdowns ("12m"/"45s") through.
    static string ShownCountdown(string countdown) =>
        countdown == "now" ? Strings.T("countdown.now") : countdown;

    // Edge-triggered, day-aware: each reminder/azan fires once, within the fire window of its minute (tick = 15s).
    void CheckNotification()
    {
        var now = DateTime.Now;
        if (_firedDate != DateTime.Today) { _fired.Clear(); _firedDate = DateTime.Today; }

        foreach (var key in Order)
        {
            if (key == "sunrise" || !_times.TryGetValue(key, out var ts)) continue;
            var at = DateTime.Today.Add(ts);
            string label = Strings.Prayer(key);
            try
            {
                if (_cfg.ReminderEnabled && InWindow(now, at.AddMinutes(-_cfg.ReminderMinutes))
                    && _fired.Add($"{now:yyyyMMdd}:{key}:rem"))
                {
                    Notify(Strings.T("balloon.reminderTitle"),
                        Strings.F("balloon.reminderBody", label, _cfg.ReminderMinutes, PrayerPopup.Format(ts, _cfg.Use24Hour)));
                    if (_cfg.ReminderSound) AudioPlayer.PlayReminder(_cfg);
                }

                if (InWindow(now, at) && _fired.Add($"{now:yyyyMMdd}:{key}"))
                {
                    Notify(Strings.T("balloon.timeTitle"),
                        Strings.F("balloon.timeBody", label, PrayerPopup.Format(ts, _cfg.Use24Hour)));
                    PlayAzan();
                }
            }
            catch { /* a balloon/audio hiccup must not kill the tick; logged by the global handler if fatal */ }
        }

        // Eve-before nudge for tomorrow's Sunnah fast, anchored to today's Maghrib (time to plan suhoor).
        if (_cfg.SunnahFastReminder && _times.TryGetValue("maghrib", out var mts)
            && InWindow(now, DateTime.Today.Add(mts)) && _fired.Add($"{now:yyyyMMdd}:sunnahfast"))
        {
            var reason = SunnahFastReason(DateTime.Today.AddDays(1));
            if (reason != null)
                Notify(Strings.T("balloon.fastTitle"), Strings.F("balloon.fastBody", FastReasonName(reason)));
        }

        // Friday nudges: Surah Al-Kahf at Fajr, and a Jumu'ah heads-up shortly before Dhuhr.
        if (_cfg.FridayReminder && now.DayOfWeek == DayOfWeek.Friday)
        {
            if (_times.TryGetValue("fajr", out var fts) && InWindow(now, DateTime.Today.Add(fts))
                && _fired.Add($"{now:yyyyMMdd}:kahf"))
                Notify(Strings.T("balloon.jumuahTitle"), Strings.T("balloon.kahfBody"));
            if (_times.TryGetValue("dhuhr", out var dts)
                && InWindow(now, DateTime.Today.Add(dts).AddMinutes(-JumuahLeadMin))
                && _fired.Add($"{now:yyyyMMdd}:jumuah"))
                Notify(Strings.T("balloon.jumuahTitle"), Strings.T("balloon.jumuahBody"));
        }
    }

    const int JumuahLeadMin = 30;

    // Prefer a rich Action Center toast; fall back to a tray balloon (older Windows / ManualOnly / failure).
    void Notify(string title, string body)
    {
        if (_cfg.RichToasts && ToastService.Show(title, body)) return;
        try { _tray.ShowBalloonTip(8000, title, body, ToolTipIcon.Info); }
        catch { /* balloon hiccup must not kill the tick */ }
    }

    // Settings "Test" button: fire a sample notification honoring the checkbox's current (unsaved) state.
    void TestNotify(bool useToast)
    {
        string title = Strings.T("app.name"), body = Strings.T("toast.test");
        if (useToast && ToastService.Show(title, body)) return;
        try { _tray.ShowBalloonTip(8000, title, body, ToolTipIcon.Info); }
        catch { /* ignore */ }
    }

    // Why tomorrow is a Sunnah fasting day (or null). Fixed days win; skips days where fasting is forbidden.
    string? SunnahFastReason(DateTime day)
    {
        var ev = IslamicEvents.ForDate(day, _cfg.HijriAdjust);
        if (ev is "eidFitr" or "eidAdha" or "tashreeq") return null; // fasting forbidden
        if (ev is "arafah" or "ashura" or "whiteDays") return ev;
        return day.DayOfWeek switch
        {
            DayOfWeek.Monday => "monday",
            DayOfWeek.Thursday => "thursday",
            _ => null,
        };
    }

    static string FastReasonName(string r) => r switch
    {
        "monday" => Strings.T("fast.monday"),
        "thursday" => Strings.T("fast.thursday"),
        _ => Strings.Event(r),
    };

    static bool InWindow(DateTime now, DateTime target)
    {
        double s = (now - target).TotalSeconds;
        return s >= 0 && s < 120; // fire within ~2 min of the minute (tick = 15s); _fired dedupes
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

    // Prayer times are whole minutes; a prayer is "current" (shows "now") during its own minute, so the
    // pill flips to "now" the same minute the azan fires — not a minute early.
    (string? key, DateTime? at, string countdown) CurrentOrNext()
    {
        var now = DateTime.Now;
        foreach (var key in Order)
        {
            if (key == "sunrise" || !_times.TryGetValue(key, out var ts)) continue;
            var at = DateTime.Today.Add(ts);
            if (now < at) return (key, at, FormatCountdown(at - now)); // upcoming
            if (now < at.AddMinutes(1)) return (key, at, "now");        // happening this minute
        }
        var tomorrow = DateTime.Today.AddDays(1);
        var t = PrayerCalculator.Compute(tomorrow, _cfg.Latitude, _cfg.Longitude,
            _cfg.ResolveTimezone(tomorrow), _cfg.CalcMethod, (AsrJuristic)_cfg.Asr, _cfg.TimeAdjust(), _cfg.HighLat);
        var fajrAt = tomorrow.Add(t["fajr"]);
        return ("fajr", fajrAt, FormatCountdown(fajrAt - now));
    }

    static string FormatCountdown(TimeSpan span)
    {
        if (span.TotalSeconds < 60) return $"{Math.Max(0, (int)span.TotalSeconds)}{Strings.T("unit.s")}";
        if (span.TotalMinutes < 60) return $"{(int)span.TotalMinutes}{Strings.T("unit.m")}";
        return $"{(int)span.TotalHours}:{span.Minutes:00}";
    }

    static string LabelOf(string key) => Strings.Prayer(key);

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
        SystemEvents.TimeChanged -= OnTimeChanged;
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
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
