using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using PrayerTray.Calc;
using PrayerTray.Config;
using PrayerTray.Native;
using PrayerTray.Services;

namespace PrayerTray.UI;

/// <summary>Themed settings dialog laid out as a 2-column card grid (no scrolling).
/// Visual changes preview live; Save persists, Cancel reverts to the opening snapshot.</summary>
public class SettingsForm : Form
{
    readonly AppConfig _cfg;
    readonly AppConfig _snapshot;
    readonly Action? _live;
    bool _ready;

    readonly TextBox _city = new() { Width = 200 };
#if !MANUAL_ONLY
    readonly Button _detect = new() { Text = "Detect", Width = 64, Margin = new Padding(4, 0, 0, 0) };
#endif
    readonly Button _openMap = new() { Text = "Open Maps", AutoSize = true };
    readonly TextBox _paste = new() { Width = 200, PlaceholderText = "lat, lng or map link" };
    readonly Button _setPaste = new() { Text = "Set", Width = 64, Margin = new Padding(4, 0, 0, 0) };
    readonly TextBox _lat = new() { Width = 200 };
    readonly TextBox _lng = new() { Width = 200 };
    readonly ComboBox _method = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _asr = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _position = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _offset = new() { Width = 200 };
    readonly ComboBox _theme = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _font = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _fontSize = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _monitor = new() { Width = 240, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly List<DisplayInfo> _displays = Displays.All();
    readonly CheckBox _h24 = new() { Text = "Use 24-hour clock", AutoSize = true };
    readonly CheckBox _hideFs = new() { Text = "Hide over fullscreen apps", AutoSize = true };

    readonly CheckBox _remEnable = new() { Text = "Remind me before each prayer", AutoSize = true };
    readonly NumericUpDown _remMins = new() { Width = 90, Minimum = 1, Maximum = 60 };
    readonly CheckBox _remSound = new() { Text = "Play a sound", AutoSize = true };
    readonly ComboBox _remSoundCombo = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _remFile = new() { Width = 160, PlaceholderText = "custom file" };
    readonly Button _remBrowse = new() { Text = "…", Width = 30, Margin = new Padding(4, 0, 0, 0) };
    readonly Button _remTest = new() { Text = "Test", Width = 50, Margin = new Padding(4, 0, 0, 0) };
    readonly ComboBox _azan = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _azanFile = new() { Width = 160, PlaceholderText = ".mp3 / .wav" };
    readonly Button _azanBrowse = new() { Text = "…", Width = 30, Margin = new Padding(4, 0, 0, 0) };
    readonly Button _azanTest = new() { Text = "Test", Width = 50 };
    readonly Button _azanStop = new() { Text = "Stop", Width = 50, Margin = new Padding(4, 0, 0, 0) };

    readonly List<string> _remSoundIds = new();
    readonly List<string> _azanIds = new();
    readonly List<Panel> _cards = new();
    static readonly int[] SizeSteps = { 80, 90, 100, 110, 125, 150 };

    public SettingsForm(AppConfig cfg, Action? livePreview = null, DetectedLocation? prefill = null)
    {
        _cfg = cfg;
        _snapshot = cfg.Clone();
        _live = livePreview;

        Text = "Prayer Tray — Settings";
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterScreen;
        MaximizeBox = false; MinimizeBox = false;
        TopMost = true; // dialog + dropdowns above the always-on-top pill, so combos don't get dismissed
        BackColor = Theme.Bg;
        ForeColor = Theme.Text;
        Font = new Font(Theme.Family, 9f);
        AutoSize = true; AutoSizeMode = AutoSizeMode.GrowAndShrink;

        // --- Location card ---
        var locBody = Body();
        var cityRow = Row(_city);
#if !MANUAL_ONLY
        cityRow.Controls.Add(_detect);
#endif
        AddRow(locBody, "City (label):", cityRow);
        AddRow(locBody, "Latitude:", _lat);
        AddRow(locBody, "Longitude:", _lng);
        AddRow(locBody, "Pick on map:", _openMap);
        AddRow(locBody, "Paste result:", Row(_paste, _setPaste));

        // --- Calculation card ---
        var calcBody = Body();
        AddRow(calcBody, "Method:", _method);
        AddRow(calcBody, "Asr:", _asr);

        // --- Appearance card ---
        var appBody = Body();
        AddRow(appBody, "Theme:", _theme);
        AddRow(appBody, "Font:", _font);
        AddRow(appBody, "Font size:", _fontSize);
        AddRow(appBody, "Widget side:", _position);
        AddRow(appBody, "Widget gap (px):", _offset);
        AddRow(appBody, "Monitor:", _monitor);
        AddSpan(appBody, _h24);
        AddSpan(appBody, _hideFs);

        // --- Notifications card ---
        var notifBody = Body();
        AddSpan(notifBody, _remEnable);
        AddRow(notifBody, "Minutes before:", _remMins);
        AddSpan(notifBody, _remSound);
        AddRow(notifBody, "Sound:", _remSoundCombo);
        AddRow(notifBody, "Custom file:", Row(_remFile, _remBrowse, _remTest));
        AddRow(notifBody, "Azan:", _azan);
        AddRow(notifBody, "Azan file:", Row(_azanFile, _azanBrowse));
        AddRow(notifBody, "", Row(_azanTest, _azanStop));

        var grid = new TableLayoutPanel { ColumnCount = 2, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Margin = new Padding(0) };
        grid.Controls.Add(Card("Location", locBody), 0, 0);
        grid.Controls.Add(Card("Calculation", calcBody), 0, 1);
        grid.Controls.Add(Card("Appearance", appBody), 1, 0);
        grid.Controls.Add(Card("Notifications", notifBody), 1, 1);

        // --- items ---
        foreach (var (key, m) in CalcMethod.All) _method.Items.Add($"{key} — {m.Name}");
        _asr.Items.Add("Standard (Shafi'i, Maliki, Hanbali)");
        _asr.Items.Add("Hanafi");
        _position.Items.Add("Right (near the clock)");
        _position.Items.Add("Left (corner)");
        foreach (var name in Theme.Names) _theme.Items.Add(name);
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var f in FontFamily.Families) if (seen.Add(f.Name)) _font.Items.Add(f.Name);
        foreach (var n in SizeSteps) _fontSize.Items.Add($"{n}%");
        foreach (var d in _displays)
            _monitor.Items.Add($"{d.FriendlyName} ({d.Bounds.Width}x{d.Bounds.Height}){(d.Primary ? " — Primary" : "")}");
        foreach (var (id, label) in AudioPlayer.ReminderSounds) { _remSoundIds.Add(id); _remSoundCombo.Items.Add(label); }
        _remSoundIds.Add("custom"); _remSoundCombo.Items.Add("Custom file…");
        _azanIds.Add("None"); _azan.Items.Add("Off");
        foreach (var (id, label) in AudioPlayer.BuiltinAdhans) { _azanIds.Add(id); _azan.Items.Add(label); }
        _azanIds.Add("Custom"); _azan.Items.Add("Custom file…");

        LoadValues();
        if (prefill != null) ApplyDetected(prefill);

        var ok = new Button { Text = "Save", DialogResult = DialogResult.OK, Width = 90, Height = 30, Margin = new Padding(6, 0, 0, 0) };
        var cancel = new Button { Text = "Cancel", DialogResult = DialogResult.Cancel, Width = 90, Height = 30 };
        ok.Click += OnSave;
        var buttons = new FlowLayoutPanel { FlowDirection = FlowDirection.RightToLeft, Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(8) };
        buttons.Controls.Add(ok);
        buttons.Controls.Add(cancel);

        var root = new TableLayoutPanel { ColumnCount = 1, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Fill };
        root.Controls.Add(grid, 0, 0);
        root.Controls.Add(buttons, 0, 1);
        Controls.Add(root);
        AcceptButton = ok; CancelButton = cancel;

        Stylize();
        StyleButton(ok, accent: true);
        WireEvents();
        SyncEnabled();
        _ready = true;

        FormClosing += (_, _) =>
        {
            AudioPlayer.Stop();
            if (DialogResult != DialogResult.OK) { _cfg.CopyFrom(_snapshot); _live?.Invoke(); }
        };
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Interop.TitleBar(Handle, Theme.Current.IsDark);
    }

    void LoadValues()
    {
        _city.Text = _cfg.City;
        _lat.Text = _cfg.Latitude.ToString(CultureInfo.InvariantCulture);
        _lng.Text = _cfg.Longitude.ToString(CultureInfo.InvariantCulture);
        _method.SelectedIndex = Math.Max(0, IndexOfMethod(_cfg.Method));
        _asr.SelectedIndex = _cfg.Asr == (int)AsrJuristic.Hanafi ? 1 : 0;
        _position.SelectedIndex = string.Equals(_cfg.WidgetAnchor, "Left", StringComparison.OrdinalIgnoreCase) ? 1 : 0;
        _offset.Text = _cfg.WidgetOffset.ToString(CultureInfo.InvariantCulture);
        _theme.SelectedIndex = Math.Max(0, Array.IndexOf(Theme.Names, _cfg.Theme));
        _font.SelectedItem = _cfg.FontFamily;
        if (_font.SelectedIndex < 0) _font.Text = _cfg.FontFamily;
        _fontSize.SelectedIndex = Math.Max(0, Array.IndexOf(SizeSteps, NearestSize(_cfg.FontScalePct)));
        int monIdx = _displays.FindIndex(d => _cfg.MonitorDeviceName == null
            ? d.Primary
            : string.Equals(d.DeviceName, _cfg.MonitorDeviceName, StringComparison.OrdinalIgnoreCase));
        if (monIdx < 0) monIdx = _displays.FindIndex(d => d.Primary);
        _monitor.SelectedIndex = Math.Max(0, monIdx);
        _h24.Checked = _cfg.Use24Hour;
        _hideFs.Checked = _cfg.HideOnFullscreen;

        _remEnable.Checked = _cfg.ReminderEnabled;
        _remMins.Value = Math.Clamp(_cfg.ReminderMinutes, 1, 60);
        _remSound.Checked = _cfg.ReminderSound;
        int si = _remSoundIds.IndexOf(_cfg.ReminderSoundId); _remSoundCombo.SelectedIndex = si < 0 ? 0 : si;
        _remFile.Text = _cfg.ReminderSoundPath ?? "";
        int ai = _azanIds.IndexOf(_cfg.AzanMode);
        if (ai < 0 && _cfg.AzanMode == "Builtin") ai = _azanIds.Count > 2 ? 1 : 0; // legacy -> first builtin
        _azan.SelectedIndex = ai < 0 ? 0 : ai;
        _azanFile.Text = _cfg.AzanCustomPath ?? "";
    }

    // Visual settings preview live; text fields apply on validated leave; audio settings only on Save.
    void WireEvents()
    {
        _theme.SelectedIndexChanged += (_, _) => Live(() => _cfg.Theme = Theme.Names[_theme.SelectedIndex]);
        _font.SelectedIndexChanged += (_, _) => Live(() => { if (_font.SelectedItem is string f) _cfg.FontFamily = f; });
        _fontSize.SelectedIndexChanged += (_, _) => Live(() => _cfg.FontScalePct = SizeSteps[_fontSize.SelectedIndex]);
        _position.SelectedIndexChanged += (_, _) => Live(() => _cfg.WidgetAnchor = _position.SelectedIndex == 1 ? "Left" : "Right");
        _h24.CheckedChanged += (_, _) => Live(() => _cfg.Use24Hour = _h24.Checked);
        _hideFs.CheckedChanged += (_, _) => Live(() => _cfg.HideOnFullscreen = _hideFs.Checked);
        _method.SelectedIndexChanged += (_, _) => Live(() => _cfg.Method = new List<string>(CalcMethod.All.Keys)[_method.SelectedIndex]);
        _asr.SelectedIndexChanged += (_, _) => Live(() => _cfg.Asr = _asr.SelectedIndex == 1 ? (int)AsrJuristic.Hanafi : (int)AsrJuristic.Standard);

        _lat.Validated += (_, _) => Live(() => { if (TryLat(out var v)) _cfg.Latitude = v; });
        _lng.Validated += (_, _) => Live(() => { if (TryLng(out var v)) _cfg.Longitude = v; });
        _offset.Validated += (_, _) => Live(() => { if (int.TryParse(_offset.Text, out var o)) _cfg.WidgetOffset = Math.Clamp(o, 0, 2000); });

        _remEnable.CheckedChanged += (_, _) => SyncEnabled();
        _remSound.CheckedChanged += (_, _) => SyncEnabled();
        _remSoundCombo.SelectedIndexChanged += (_, _) => SyncEnabled();
        _azan.SelectedIndexChanged += (_, _) => SyncEnabled();

#if !MANUAL_ONLY
        _detect.Click += OnDetect;
#endif
        _openMap.Click += OnOpenMap;
        _setPaste.Click += OnSetPaste;
        _remBrowse.Click += (_, _) => PickFile(_remFile, "Audio|*.wav;*.mp3");
        _remTest.Click += (_, _) => AudioPlayer.Play(CurrentReminderPath());
        _azanBrowse.Click += (_, _) => PickFile(_azanFile, "Audio|*.mp3;*.wav");
        _azanTest.Click += (_, _) => { var p = CurrentAzanPath(); if (p != null) AudioPlayer.Play(p); };
        _azanStop.Click += (_, _) => AudioPlayer.Stop();
    }

    void Live(Action set)
    {
        if (!_ready) return;
        set();
        _live?.Invoke();
    }

    bool TryLat(out double v) => double.TryParse(_lat.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out v) && v >= -90 && v <= 90;
    bool TryLng(out double v) => double.TryParse(_lng.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out v) && v >= -180 && v <= 180;

    void SyncEnabled()
    {
        bool rem = _remEnable.Checked;
        _remMins.Enabled = rem;
        _remSound.Enabled = rem;
        bool snd = rem && _remSound.Checked;
        _remSoundCombo.Enabled = snd;
        bool custom = snd && _remSoundIds[Math.Max(0, _remSoundCombo.SelectedIndex)] == "custom";
        _remFile.Enabled = _remBrowse.Enabled = custom;
        _remTest.Enabled = snd;
        bool azanCustom = _azanIds[Math.Max(0, _azan.SelectedIndex)] == "Custom";
        _azanFile.Enabled = _azanBrowse.Enabled = azanCustom;
        _azanTest.Enabled = _azanIds[Math.Max(0, _azan.SelectedIndex)] != "None";
    }

    string CurrentReminderPath()
    {
        string id = _remSoundIds[Math.Max(0, _remSoundCombo.SelectedIndex)];
        return id == "custom" && !string.IsNullOrWhiteSpace(_remFile.Text) ? _remFile.Text : AudioPlayer.SynthPath(id);
    }

    string? CurrentAzanPath() => _azanIds[Math.Max(0, _azan.SelectedIndex)] switch
    {
        "None" => null,
        "Custom" => string.IsNullOrWhiteSpace(_azanFile.Text) ? null : _azanFile.Text,
        var id => AudioPlayer.BuiltinAdhanPath(id),
    };

    void PickFile(TextBox target, string filter)
    {
        using var d = new OpenFileDialog { Filter = filter + "|All files|*.*" };
        if (d.ShowDialog(this) == DialogResult.OK) { target.Text = d.FileName; SyncEnabled(); }
    }

#if !MANUAL_ONLY
    async void OnDetect(object? sender, EventArgs e)
    {
        _detect.Enabled = false; _detect.Text = "…";
        try
        {
            var loc = await LocationService.DetectAsync();
            if (loc == null)
                MessageBox.Show(this, "Couldn't detect location — enter it manually.",
                    "Detect location", MessageBoxButtons.OK, MessageBoxIcon.Information);
            else ApplyDetected(loc);
        }
        finally { _detect.Enabled = true; _detect.Text = "Detect"; }
    }
#endif

    async void OnOpenMap(object? sender, EventArgs e)
    {
        _openMap.Enabled = false;
        try
        {
            var rough = await LocationService.IpRoughAsync();
            string url = LocationService.MapsUrl(rough?.Lat, rough?.Lng);
            Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
        }
        catch { /* browser launch failed; nothing to do */ }
        finally { _openMap.Enabled = true; }
    }

    async void OnSetPaste(object? sender, EventArgs e)
    {
        var loc = await LocationService.ParseAsync(_paste.Text);
        if (loc == null)
        {
            MessageBox.Show(this, "Couldn't read coordinates. Right-click your spot in Google Maps, click the\n" +
                "lat/lng to copy them, and paste here — or paste the map's address-bar URL.",
                "Paste result", MessageBoxButtons.OK, MessageBoxIcon.Information);
            return;
        }
        ApplyDetected(loc);
    }

    void ApplyDetected(DetectedLocation loc)
    {
        _lat.Text = loc.Lat.ToString(CultureInfo.InvariantCulture);
        _lng.Text = loc.Lng.ToString(CultureInfo.InvariantCulture);
        if (!string.IsNullOrWhiteSpace(loc.City)) _city.Text = loc.City;
        else if (string.IsNullOrWhiteSpace(_city.Text)) _city.Text = "My location";
        _method.SelectedIndex = Math.Max(0, IndexOfMethod(LocationService.MethodForCountry(loc.CountryIso)));
        Live(() => { if (TryLat(out var la)) _cfg.Latitude = la; if (TryLng(out var lo)) _cfg.Longitude = lo; });
    }

    static int IndexOfMethod(string key)
    {
        int i = 0;
        foreach (var k in CalcMethod.All.Keys) { if (k == key) return i; i++; }
        return 0;
    }

    static int NearestSize(int pct)
    {
        int best = SizeSteps[0];
        foreach (var s in SizeSteps) if (Math.Abs(s - pct) < Math.Abs(best - pct)) best = s;
        return best;
    }

    void OnSave(object? sender, EventArgs e)
    {
        if (!TryLat(out var lat)) { Warn("Latitude must be a number between -90 and 90."); return; }
        if (!TryLng(out var lng)) { Warn("Longitude must be a number between -180 and 180."); return; }
        string azanMode = _azanIds[Math.Max(0, _azan.SelectedIndex)];
        if (azanMode == "Custom" && string.IsNullOrWhiteSpace(_azanFile.Text))
        { Warn("Pick an azan audio file, or set Azan to Off."); return; }

        _cfg.City = string.IsNullOrWhiteSpace(_city.Text) ? "Custom" : _city.Text.Trim();
        _cfg.Latitude = lat; _cfg.Longitude = lng;
        _cfg.Method = new List<string>(CalcMethod.All.Keys)[_method.SelectedIndex];
        _cfg.Asr = _asr.SelectedIndex == 1 ? (int)AsrJuristic.Hanafi : (int)AsrJuristic.Standard;
        _cfg.WidgetAnchor = _position.SelectedIndex == 1 ? "Left" : "Right";
        _cfg.Theme = Theme.Names[_theme.SelectedIndex];
        if (_font.SelectedItem is string fam && !string.IsNullOrWhiteSpace(fam)) _cfg.FontFamily = fam;
        _cfg.FontScalePct = SizeSteps[Math.Max(0, _fontSize.SelectedIndex)];
        var mon = _displays[_monitor.SelectedIndex];
        _cfg.MonitorDeviceName = mon.Primary ? null : mon.DeviceName;
        if (int.TryParse(_offset.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var off))
            _cfg.WidgetOffset = Math.Clamp(off, 0, 2000);
        _cfg.Use24Hour = _h24.Checked;
        _cfg.HideOnFullscreen = _hideFs.Checked;

        _cfg.ReminderEnabled = _remEnable.Checked;
        _cfg.ReminderMinutes = Math.Clamp((int)_remMins.Value, 1, 60);
        _cfg.ReminderSound = _remSound.Checked;
        _cfg.ReminderSoundId = _remSoundIds[Math.Max(0, _remSoundCombo.SelectedIndex)];
        _cfg.ReminderSoundPath = string.IsNullOrWhiteSpace(_remFile.Text) ? null : _remFile.Text.Trim();
        _cfg.AzanMode = azanMode;
        _cfg.AzanCustomPath = string.IsNullOrWhiteSpace(_azanFile.Text) ? null : _azanFile.Text.Trim();
        _cfg.Save();
        _live?.Invoke();
    }

    void Warn(string msg)
    {
        DialogResult = DialogResult.None;
        MessageBox.Show(this, msg, "Invalid input", MessageBoxButtons.OK, MessageBoxIcon.Warning);
    }

    // --- layout helpers ---
    static FlowLayoutPanel Row(params Control[] controls)
    {
        var f = new FlowLayoutPanel { AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Margin = new Padding(0), FlowDirection = FlowDirection.LeftToRight, WrapContents = false };
        foreach (var c in controls) f.Controls.Add(c);
        return f;
    }

    static TableLayoutPanel Body() =>
        new() { ColumnCount = 2, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Fill, Margin = new Padding(0) };

    static void AddRow(TableLayoutPanel t, string label, Control c)
    {
        int r = t.RowCount;
        t.Controls.Add(new Label { Text = label, AutoSize = true, ForeColor = Theme.Text, Anchor = AnchorStyles.Left, Margin = new Padding(3, 6, 8, 3) }, 0, r);
        t.Controls.Add(c, 1, r);
        t.RowCount++;
    }

    static void AddSpan(TableLayoutPanel t, Control c)
    {
        int r = t.RowCount;
        c.Margin = new Padding(3, 8, 3, 3);
        t.Controls.Add(c, 0, r);
        t.SetColumnSpan(c, 2);
        t.RowCount++;
    }

    Panel Card(string title, Control body)
    {
        var header = new Label { Text = title, AutoSize = true, ForeColor = Theme.Accent, Font = new Font(Theme.Family, 10.5f, FontStyle.Bold), Margin = new Padding(0, 0, 0, 8) };
        var inner = new TableLayoutPanel { ColumnCount = 1, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Fill, Margin = new Padding(0) };
        inner.Controls.Add(header, 0, 0);
        inner.Controls.Add(body, 0, 1);
        var card = new Panel { AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, BackColor = Theme.Panel, Padding = new Padding(14), Margin = new Padding(8), MinimumSize = new Size(260, 0) };
        card.Controls.Add(inner);
        card.Resize += (_, _) => RoundPanel(card);
        card.HandleCreated += (_, _) => RoundPanel(card);
        _cards.Add(card);
        return card;
    }

    static void RoundPanel(Panel p)
    {
        if (p.Handle == IntPtr.Zero || p.Width <= 0 || p.Height <= 0) return;
        Interop.SetWindowRgn(p.Handle, Interop.CreateRoundRectRgn(0, 0, p.Width + 1, p.Height + 1, 16, 16), true);
    }

    // --- theming ---
    void Stylize()
    {
        foreach (var cb in new[] { _method, _asr, _position, _theme, _font, _fontSize, _monitor, _remSoundCombo, _azan }) StyleCombo(cb);
        foreach (var tb in new[] { _city, _paste, _lat, _lng, _offset, _remFile, _azanFile }) StyleText(tb);
        foreach (var ck in new[] { _h24, _hideFs, _remEnable, _remSound }) StyleCheck(ck);
        StyleNumeric(_remMins);
        var btns = new List<Button> { _openMap, _setPaste, _remBrowse, _remTest, _azanBrowse, _azanTest, _azanStop };
#if !MANUAL_ONLY
        btns.Add(_detect);
#endif
        foreach (var b in btns) StyleButton(b);
    }

    static void StyleCombo(ComboBox c)
    {
        c.FlatStyle = FlatStyle.Flat;
        c.BackColor = Theme.BgHover;
        c.ForeColor = Theme.Text;
        c.DrawMode = DrawMode.OwnerDrawFixed;
        c.DrawItem += ComboDraw;
    }

    static void ComboDraw(object? sender, DrawItemEventArgs e)
    {
        var c = (ComboBox)sender!;
        bool sel = (e.State & DrawItemState.Selected) != 0;
        using (var bg = new SolidBrush(sel ? Theme.AccentSoft : Theme.BgHover)) e.Graphics.FillRectangle(bg, e.Bounds);
        if (e.Index >= 0)
            using (var b = new SolidBrush(Theme.Text))
                e.Graphics.DrawString(c.Items[e.Index]?.ToString(), e.Font ?? c.Font, b, e.Bounds.X + 1, e.Bounds.Y + 1);
    }

    static void StyleText(TextBox t) { t.BorderStyle = BorderStyle.FixedSingle; t.BackColor = Theme.BgHover; t.ForeColor = Theme.Text; }
    static void StyleCheck(CheckBox c) { c.ForeColor = Theme.Text; c.FlatStyle = FlatStyle.Flat; }
    static void StyleNumeric(NumericUpDown n) { n.BorderStyle = BorderStyle.FixedSingle; n.BackColor = Theme.BgHover; n.ForeColor = Theme.Text; }

    static void StyleButton(Button b, bool accent = false)
    {
        b.FlatStyle = FlatStyle.Flat;
        b.FlatAppearance.BorderSize = 1;
        b.FlatAppearance.BorderColor = accent ? Theme.Accent : Theme.BgHover;
        b.BackColor = accent ? Theme.Accent : Theme.BgHover;
        b.ForeColor = accent ? OnAccent() : Theme.Text;
        b.UseVisualStyleBackColor = false;
    }

    static Color OnAccent()
    {
        var a = Theme.Accent;
        return (a.R * 299 + a.G * 587 + a.B * 114) / 1000 > 140 ? Color.Black : Color.White;
    }
}
