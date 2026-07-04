using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.IO;
using System.Net.NetworkInformation;
using System.Text.Json;
using System.Windows.Forms;
using PrayerTray.Calc;
using PrayerTray.Config;
using PrayerTray.I18n;
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
    readonly Action<bool>? _testNotify;   // (useToast) -> fire a sample notification
    bool _ready;

    readonly TextBox _city = new() { Width = 200 };
#if !MANUAL_ONLY
    readonly Button _detect = new() { Text = Strings.T("btn.detect"), Width = 64, Margin = new Padding(4, 0, 0, 0) };
#endif
    readonly Button _openMap = new() { Text = Strings.T("btn.openMaps"), AutoSize = true };
    readonly TextBox _paste = new() { Width = 200, PlaceholderText = Strings.T("ph.paste") };
    readonly Button _setPaste = new() { Text = Strings.T("btn.set"), Width = 64, Margin = new Padding(4, 0, 0, 0) };
    readonly TextBox _lat = new() { Width = 200 };
    readonly TextBox _lng = new() { Width = 200 };
    readonly ComboBox _method = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _asr = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _highLat = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly NumericUpDown _adjFajr = new() { Width = 90, Minimum = -60, Maximum = 60 };
    readonly NumericUpDown _adjDhuhr = new() { Width = 90, Minimum = -60, Maximum = 60 };
    readonly NumericUpDown _adjAsr = new() { Width = 90, Minimum = -60, Maximum = 60 };
    readonly NumericUpDown _adjMaghrib = new() { Width = 90, Minimum = -60, Maximum = 60 };
    readonly NumericUpDown _adjIsha = new() { Width = 90, Minimum = -60, Maximum = 60 };
    readonly NumericUpDown _iqFajr = new() { Width = 90, Minimum = 0, Maximum = 60 };
    readonly NumericUpDown _iqDhuhr = new() { Width = 90, Minimum = 0, Maximum = 60 };
    readonly NumericUpDown _iqAsr = new() { Width = 90, Minimum = 0, Maximum = 60 };
    readonly NumericUpDown _iqMaghrib = new() { Width = 90, Minimum = 0, Maximum = 60 };
    readonly NumericUpDown _iqIsha = new() { Width = 90, Minimum = 0, Maximum = 60 };
    readonly ComboBox _position = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _offset = new() { Width = 200 };
    readonly ComboBox _language = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _theme = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _font = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _fontSize = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _monitor = new() { Width = 240, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly List<DisplayInfo> _displays = Displays.All();
    readonly CheckBox _h24 = new() { Text = Strings.T("chk.use24"), AutoSize = true };
    readonly CheckBox _hideFs = new() { Text = Strings.T("chk.hideFs"), AutoSize = true };
    readonly CheckBox _showHijri = new() { Text = Strings.T("chk.showHijri"), AutoSize = true };
    readonly CheckBox _showEvents = new() { Text = Strings.T("chk.showEvents"), AutoSize = true };
    readonly CheckBox _sunnahFast = new() { Text = Strings.T("chk.sunnahFast"), AutoSize = true };
    readonly CheckBox _fridayRem = new() { Text = Strings.T("chk.fridayReminder"), AutoSize = true };
    readonly CheckBox _netSpeed = new() { Text = Strings.T("chk.netSpeed"), AutoSize = true };
    readonly CheckBox _ping = new() { Text = Strings.T("chk.ping"), AutoSize = true };
    readonly TextBox _pingHost = new() { Width = 200 };
    readonly CheckBox _pingTcp = new() { Text = Strings.T("chk.pingTcp"), AutoSize = true };
    readonly ComboBox _netIface = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly List<string> _ifaceIds = new();
    readonly CheckBox _compactMeters = new() { Text = Strings.T("chk.compactMeters"), AutoSize = true };
    readonly CheckBox _trackUsage = new() { Text = Strings.T("chk.trackUsage"), AutoSize = true };
    readonly CheckBox _showUsage = new() { Text = Strings.T("chk.showUsage"), AutoSize = true };
    readonly NumericUpDown _hijriAdjust = new() { Width = 90, Minimum = -2, Maximum = 2 };

    readonly CheckBox _richToasts = new() { Text = Strings.T("chk.richToasts"), AutoSize = true };
    readonly Button _testToast = new() { Text = Strings.T("btn.test"), AutoSize = true, Margin = new Padding(8, 0, 0, 0) };
    readonly CheckBox _remEnable = new() { Text = Strings.T("chk.remind"), AutoSize = true };
    readonly NumericUpDown _remMins = new() { Width = 90, Minimum = 1, Maximum = 60 };
    readonly CheckBox _remSound = new() { Text = Strings.T("chk.playSound"), AutoSize = true };
    readonly ComboBox _remSoundCombo = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _remFile = new() { Width = 160, PlaceholderText = Strings.T("ph.customFile") };
    readonly Button _remBrowse = new() { Text = "…", Width = 30, Margin = new Padding(4, 0, 0, 0) };
    readonly Button _remTest = new() { Text = Strings.T("btn.test"), Width = 50, Margin = new Padding(4, 0, 0, 0) };
    readonly ComboBox _azan = new() { Width = 200, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _azanFile = new() { Width = 160, PlaceholderText = ".mp3 / .wav" };
    readonly Button _azanBrowse = new() { Text = "…", Width = 30, Margin = new Padding(4, 0, 0, 0) };
    readonly Button _azanTest = new() { Text = Strings.T("btn.test"), Width = 50 };
    readonly Button _azanStop = new() { Text = Strings.T("btn.stop"), Width = 50, Margin = new Padding(4, 0, 0, 0) };

    readonly List<string> _remSoundIds = new();
    readonly List<string> _azanIds = new();
    readonly List<string> _langIds = new();
    static readonly string[] HighLatIds = { "AngleBased", "MidNight", "OneSeventh", "None" };
    readonly List<Button> _navButtons = new();
    readonly List<Panel> _sectionCards = new();
    static readonly int[] SizeSteps = { 80, 90, 100, 110, 125, 150 };

    /// <summary>Set when the user changes language; the host reopens the dialog so it rebuilds in the new
    /// language/direction (strings + RTL are fixed at construction).</summary>
    public bool RestartRequested { get; private set; }

    public SettingsForm(AppConfig cfg, Action? livePreview = null, DetectedLocation? prefill = null,
        AppConfig? snapshotOverride = null, Action<bool>? testNotify = null)
    {
        _cfg = cfg;
        _snapshot = snapshotOverride ?? cfg.Clone();
        _live = livePreview;
        _testNotify = testNotify;

        SuspendLayout(); // ~60 controls; without this every Add cascades a full AutoSize relayout
        Text = Strings.T("settings.title");
        if (Strings.IsRtl) { RightToLeft = RightToLeft.Yes; RightToLeftLayout = true; }
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
        AddRow(locBody, Strings.T("label.city"), cityRow);
        AddRow(locBody, Strings.T("label.lat"), _lat);
        AddRow(locBody, Strings.T("label.lng"), _lng);
        AddRow(locBody, Strings.T("label.pickMap"), _openMap);
        AddRow(locBody, Strings.T("label.pasteResult"), Row(_paste, _setPaste));

        // --- Calculation card ---
        var calcBody = Body();
        AddRow(calcBody, Strings.T("label.method"), _method);
        AddRow(calcBody, Strings.T("label.asr"), _asr);
        AddRow(calcBody, Strings.T("label.highLat"), _highLat);
        AddSpan(calcBody, new Label { Text = Strings.T("label.tuneTimes"), AutoSize = true, ForeColor = Theme.TextDim });
        AddRow(calcBody, Strings.Prayer("fajr"), _adjFajr);
        AddRow(calcBody, Strings.Prayer("dhuhr"), _adjDhuhr);
        AddRow(calcBody, Strings.Prayer("asr"), _adjAsr);
        AddRow(calcBody, Strings.Prayer("maghrib"), _adjMaghrib);
        AddRow(calcBody, Strings.Prayer("isha"), _adjIsha);
        AddSpan(calcBody, new Label { Text = Strings.T("label.iqamahTune"), AutoSize = true, ForeColor = Theme.TextDim });
        AddRow(calcBody, Strings.Prayer("fajr"), _iqFajr);
        AddRow(calcBody, Strings.Prayer("dhuhr"), _iqDhuhr);
        AddRow(calcBody, Strings.Prayer("asr"), _iqAsr);
        AddRow(calcBody, Strings.Prayer("maghrib"), _iqMaghrib);
        AddRow(calcBody, Strings.Prayer("isha"), _iqIsha);

        // --- Appearance card ---
        var appBody = Body();
        AddRow(appBody, Strings.T("label.language"), _language);
        AddRow(appBody, Strings.T("label.theme"), _theme);
        AddRow(appBody, Strings.T("label.font"), _font);
        AddRow(appBody, Strings.T("label.fontSize"), _fontSize);
        AddRow(appBody, Strings.T("label.widgetSide"), _position);
        AddRow(appBody, Strings.T("label.widgetGap"), _offset);
        AddRow(appBody, Strings.T("label.monitor"), _monitor);
        AddSpan(appBody, _h24);
        AddSpan(appBody, _hideFs);

        // --- Network card ---
        var netBody = Body();
        AddSpan(netBody, _netSpeed);
        AddSpan(netBody, _ping);
        AddRow(netBody, Strings.T("label.pingHost"), _pingHost);
        AddSpan(netBody, _pingTcp);
        AddRow(netBody, Strings.T("label.netInterface"), _netIface);
        AddSpan(netBody, _compactMeters);
        AddSpan(netBody, _trackUsage);
        AddSpan(netBody, _showUsage);

        // --- Religious card ---
        var relBody = Body();
        AddSpan(relBody, _showHijri);
        AddRow(relBody, Strings.T("label.hijriAdjust"), _hijriAdjust);
        AddSpan(relBody, _showEvents);
        AddSpan(relBody, _sunnahFast);
        AddSpan(relBody, _fridayRem);

        // --- Notifications card ---
        var notifBody = Body();
        AddSpan(notifBody, Row(_richToasts, _testToast));
        AddSpan(notifBody, _remEnable);
        AddRow(notifBody, Strings.T("label.minutesBefore"), _remMins);
        AddSpan(notifBody, _remSound);
        AddRow(notifBody, Strings.T("label.sound"), _remSoundCombo);
        AddRow(notifBody, Strings.T("label.customFile"), Row(_remFile, _remBrowse, _remTest));
        AddRow(notifBody, Strings.T("label.azan"), _azan);
        AddRow(notifBody, Strings.T("label.azanFile"), Row(_azanFile, _azanBrowse));
        AddRow(notifBody, "", Row(_azanTest, _azanStop));

        // --- side-nav: section list (left) + swappable content (right) ---
        var sections = new (string title, Control body)[]
        {
            (Strings.T("card.location"), locBody),
            (Strings.T("card.calculation"), calcBody),
            (Strings.T("card.appearance"), appBody),
            (Strings.T("card.network"), netBody),
            (Strings.T("card.religious"), relBody),
            (Strings.T("card.notifications"), notifBody),
        };

        var content = new Panel { AutoSize = false, Margin = new Padding(8, 0, 0, 0) };
        int maxW = 0, maxH = 0;
        foreach (var (title, body) in sections)
        {
            var inner = new TableLayoutPanel { ColumnCount = 1, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Top, Margin = new Padding(0), Font = Font };
            inner.Controls.Add(new Label { Text = title, AutoSize = true, ForeColor = Theme.Accent, Font = new Font(Theme.Family, 12f, FontStyle.Bold), Margin = new Padding(0, 0, 0, 10) }, 0, 0);
            inner.Controls.Add(body, 0, 1);
            var ps = inner.GetPreferredSize(Size.Empty);
            maxW = Math.Max(maxW, ps.Width); maxH = Math.Max(maxH, ps.Height);
            var card = new Panel { AutoSize = false, Dock = DockStyle.Fill, BackColor = Theme.Panel, Padding = new Padding(16), Visible = false };
            card.Controls.Add(inner);
            card.Resize += (_, _) => RoundPanel(card);
            card.HandleCreated += (_, _) => RoundPanel(card);
            content.Controls.Add(card);
            _sectionCards.Add(card);
        }
        content.Size = new Size(maxW + 40, maxH + 40);

        var nav = new FlowLayoutPanel { FlowDirection = FlowDirection.TopDown, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, WrapContents = false, Margin = new Padding(0) };
        for (int i = 0; i < sections.Length; i++)
        {
            int idx = i;
            var b = new Button
            {
                Text = sections[i].title, Width = 132, Height = 36,
                TextAlign = Strings.IsRtl ? ContentAlignment.MiddleRight : ContentAlignment.MiddleLeft,
                Margin = new Padding(0, 0, 0, 6), FlatStyle = FlatStyle.Flat, UseVisualStyleBackColor = false,
            };
            b.FlatAppearance.BorderSize = 0;
            b.Click += (_, _) => SelectSection(idx);
            nav.Controls.Add(b); _navButtons.Add(b);
        }

        var grid = new TableLayoutPanel { ColumnCount = 2, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Margin = new Padding(0) };
        grid.Controls.Add(nav, 0, 0);
        grid.Controls.Add(content, 1, 0);

        // --- items ---
        // Calculation-method names are org/proper nouns -> not localized.
        foreach (var (key, m) in CalcMethod.All) _method.Items.Add($"{key} — {m.Name}");
        _asr.Items.Add(Strings.T("asr.standard"));
        _asr.Items.Add(Strings.T("asr.hanafi"));
        foreach (var id in HighLatIds) _highLat.Items.Add(Strings.T("highLat." + id));
        _position.Items.Add(Strings.T("side.right"));
        _position.Items.Add(Strings.T("side.left"));
        _langIds.Add("auto"); _language.Items.Add(Strings.T("lang.auto"));
        _langIds.Add("en"); _language.Items.Add("English");
        _langIds.Add("ar"); _language.Items.Add("العربية");
        _langIds.Add("fr"); _language.Items.Add("Français");
        _langIds.Add("tr"); _language.Items.Add("Türkçe");
        _langIds.Add("ur"); _language.Items.Add("اردو");
        _langIds.Add("id"); _language.Items.Add("Bahasa Indonesia");
        foreach (var name in Theme.Names) _theme.Items.Add(name);
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var f in FontFamily.Families) if (seen.Add(f.Name)) _font.Items.Add(f.Name);
        foreach (var n in SizeSteps) _fontSize.Items.Add($"{n}%");
        foreach (var d in _displays)
            _monitor.Items.Add($"{d.FriendlyName} ({d.Bounds.Width}x{d.Bounds.Height}){(d.Primary ? Strings.T("monitor.primary") : "")}");
        foreach (var (id, _) in AudioPlayer.ReminderSounds) { _remSoundIds.Add(id); _remSoundCombo.Items.Add(Strings.T("sound." + id)); }
        _remSoundIds.Add("custom"); _remSoundCombo.Items.Add(Strings.T("combo.customFile"));
        _azanIds.Add("None"); _azan.Items.Add(Strings.T("azan.off"));
        foreach (var (id, _) in AudioPlayer.BuiltinAdhans) { _azanIds.Add(id); _azan.Items.Add(Strings.T("adhan." + id)); }
        _azanIds.Add("Custom"); _azan.Items.Add(Strings.T("combo.customFile"));
        _ifaceIds.Add(""); _netIface.Items.Add(Strings.T("iface.all"));
        try
        {
            foreach (var ni in NetworkInterface.GetAllNetworkInterfaces())
            {
                if (ni.NetworkInterfaceType == NetworkInterfaceType.Loopback) continue;
                _ifaceIds.Add(ni.Id); _netIface.Items.Add(ni.Name);
            }
        }
        catch { /* enumeration failed -> "All" only */ }

        LoadValues();
        if (prefill != null) ApplyDetected(prefill);

        var ok = new Button { Text = Strings.T("btn.save"), DialogResult = DialogResult.OK, Width = 90, Height = 30, Margin = new Padding(6, 0, 0, 0) };
        var cancel = new Button { Text = Strings.T("btn.cancel"), DialogResult = DialogResult.Cancel, Width = 90, Height = 30 };
        var export = new Button { Text = Strings.T("btn.exportCfg"), Width = 90, Height = 30, Margin = new Padding(24, 0, 0, 0) };
        var import = new Button { Text = Strings.T("btn.importCfg"), Width = 90, Height = 30 };
        ok.Click += OnSave;
        export.Click += OnExport;
        import.Click += OnImport;
        // RightToLeftLayout already mirrors the panel, so flip the flow back to keep Save on the inner side.
        var buttons = new FlowLayoutPanel { FlowDirection = Strings.IsRtl ? FlowDirection.LeftToRight : FlowDirection.RightToLeft, Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(8) };
        buttons.Controls.Add(ok);
        buttons.Controls.Add(cancel);
        buttons.Controls.Add(export);
        buttons.Controls.Add(import);

        var root = new TableLayoutPanel { ColumnCount = 1, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Dock = DockStyle.Fill };
        root.Controls.Add(grid, 0, 0);
        root.Controls.Add(buttons, 0, 1);
        Controls.Add(root);
        AcceptButton = ok; CancelButton = cancel;

        Stylize();
        StyleButton(ok, accent: true);
        StyleButton(export);
        StyleButton(import);
        WireEvents();
        SyncEnabled();
        SelectSection(0);
        ResumeLayout(true);
        _ready = true;

        FormClosing += (_, _) =>
        {
            AudioPlayer.Stop();
            // A language restart (Retry) keeps the in-progress config; only a real Cancel reverts.
            if (DialogResult != DialogResult.OK && !RestartRequested)
            {
                _cfg.CopyFrom(_snapshot);
                Strings.Set(_cfg.Language); // restore direction/language if it was changed this session
                _live?.Invoke();
            }
        };
    }

    // WS_EX_COMPOSITED: the form and all children paint into one buffer -> no visible
    // card-by-card repaint when switching sections or toggling enabled states.
    protected override CreateParams CreateParams
    {
        get { var cp = base.CreateParams; cp.ExStyle |= 0x02000000; return cp; }
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
        _highLat.SelectedIndex = Math.Max(0, Array.IndexOf(HighLatIds, _cfg.HighLats));
        _adjFajr.Value = Math.Clamp(_cfg.FajrAdjust, -60, 60);
        _adjDhuhr.Value = Math.Clamp(_cfg.DhuhrAdjust, -60, 60);
        _adjAsr.Value = Math.Clamp(_cfg.AsrAdjust, -60, 60);
        _adjMaghrib.Value = Math.Clamp(_cfg.MaghribAdjust, -60, 60);
        _adjIsha.Value = Math.Clamp(_cfg.IshaAdjust, -60, 60);
        _iqFajr.Value = Math.Clamp(_cfg.FajrIqamah, 0, 60);
        _iqDhuhr.Value = Math.Clamp(_cfg.DhuhrIqamah, 0, 60);
        _iqAsr.Value = Math.Clamp(_cfg.AsrIqamah, 0, 60);
        _iqMaghrib.Value = Math.Clamp(_cfg.MaghribIqamah, 0, 60);
        _iqIsha.Value = Math.Clamp(_cfg.IshaIqamah, 0, 60);
        _position.SelectedIndex = string.Equals(_cfg.WidgetAnchor, "Left", StringComparison.OrdinalIgnoreCase) ? 1 : 0;
        _offset.Text = _cfg.WidgetOffset.ToString(CultureInfo.InvariantCulture);
        int li = _langIds.IndexOf(_cfg.Language); _language.SelectedIndex = li < 0 ? 0 : li;
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
        _netSpeed.Checked = _cfg.ShowNetSpeed;
        _ping.Checked = _cfg.ShowPing;
        _pingHost.Text = _cfg.PingHost;
        _pingTcp.Checked = _cfg.PingTcp;
        int ii = _ifaceIds.IndexOf(_cfg.NetInterfaceId ?? "");
        if (ii < 0)
        {
            // Adapter currently absent (unplugged/disabled): keep the choice instead of
            // silently rewriting it to "All" on the next unrelated Save.
            _ifaceIds.Add(_cfg.NetInterfaceId!);
            _netIface.Items.Add(Strings.T("iface.missing"));
            ii = _ifaceIds.Count - 1;
        }
        _netIface.SelectedIndex = ii;
        _compactMeters.Checked = _cfg.CompactMeters;
        _trackUsage.Checked = _cfg.TrackDataUsage;
        _showUsage.Checked = _cfg.ShowDataUsage;
        _showHijri.Checked = _cfg.ShowHijriDate;
        _showEvents.Checked = _cfg.ShowIslamicEvents;
        _sunnahFast.Checked = _cfg.SunnahFastReminder;
        _fridayRem.Checked = _cfg.FridayReminder;
        _hijriAdjust.Value = Math.Clamp(_cfg.HijriAdjust, -2, 2);

        _richToasts.Checked = _cfg.RichToasts;
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
        _language.SelectedIndexChanged += (_, _) => OnLanguageChanged();
        _theme.SelectedIndexChanged += (_, _) => Live(() => _cfg.Theme = Theme.Names[_theme.SelectedIndex]);
        _font.SelectedIndexChanged += (_, _) => Live(() => { if (_font.SelectedItem is string f) _cfg.FontFamily = f; });
        _fontSize.SelectedIndexChanged += (_, _) => Live(() => _cfg.FontScalePct = SizeSteps[_fontSize.SelectedIndex]);
        _position.SelectedIndexChanged += (_, _) => Live(() => _cfg.WidgetAnchor = _position.SelectedIndex == 1 ? "Left" : "Right");
        _h24.CheckedChanged += (_, _) => Live(() => _cfg.Use24Hour = _h24.Checked);
        _hideFs.CheckedChanged += (_, _) => Live(() => _cfg.HideOnFullscreen = _hideFs.Checked);
        _netSpeed.CheckedChanged += (_, _) => { Live(() => _cfg.ShowNetSpeed = _netSpeed.Checked); SyncEnabled(); };
        _ping.CheckedChanged += (_, _) => { Live(() => _cfg.ShowPing = _ping.Checked); SyncEnabled(); };
        _pingHost.TextChanged += (_, _) => Live(() => _cfg.PingHost = _pingHost.Text.Trim());
        _pingTcp.CheckedChanged += (_, _) => Live(() => _cfg.PingTcp = _pingTcp.Checked);
        _netIface.SelectedIndexChanged += (_, _) => Live(() =>
        {
            string id = _ifaceIds[Math.Max(0, _netIface.SelectedIndex)];
            _cfg.NetInterfaceId = id.Length == 0 ? null : id;
        });
        _compactMeters.CheckedChanged += (_, _) => Live(() => _cfg.CompactMeters = _compactMeters.Checked);
        _trackUsage.CheckedChanged += (_, _) => { Live(() => _cfg.TrackDataUsage = _trackUsage.Checked); SyncEnabled(); };
        _showUsage.CheckedChanged += (_, _) => { Live(() => _cfg.ShowDataUsage = _showUsage.Checked); SyncEnabled(); };
        _showHijri.CheckedChanged += (_, _) => { Live(() => _cfg.ShowHijriDate = _showHijri.Checked); SyncEnabled(); };
        _showEvents.CheckedChanged += (_, _) => Live(() => _cfg.ShowIslamicEvents = _showEvents.Checked);
        _hijriAdjust.ValueChanged += (_, _) => Live(() => _cfg.HijriAdjust = (int)_hijriAdjust.Value);
        _method.SelectedIndexChanged += (_, _) => Live(() => _cfg.Method = new List<string>(CalcMethod.All.Keys)[_method.SelectedIndex]);
        _asr.SelectedIndexChanged += (_, _) => Live(() => _cfg.Asr = _asr.SelectedIndex == 1 ? (int)AsrJuristic.Hanafi : (int)AsrJuristic.Standard);
        _highLat.SelectedIndexChanged += (_, _) => Live(() => _cfg.HighLats = HighLatIds[Math.Max(0, _highLat.SelectedIndex)]);
        _adjFajr.ValueChanged += (_, _) => Live(() => _cfg.FajrAdjust = (int)_adjFajr.Value);
        _adjDhuhr.ValueChanged += (_, _) => Live(() => _cfg.DhuhrAdjust = (int)_adjDhuhr.Value);
        _adjAsr.ValueChanged += (_, _) => Live(() => _cfg.AsrAdjust = (int)_adjAsr.Value);
        _adjMaghrib.ValueChanged += (_, _) => Live(() => _cfg.MaghribAdjust = (int)_adjMaghrib.Value);
        _adjIsha.ValueChanged += (_, _) => Live(() => _cfg.IshaAdjust = (int)_adjIsha.Value);
        _iqFajr.ValueChanged += (_, _) => Live(() => _cfg.FajrIqamah = (int)_iqFajr.Value);
        _iqDhuhr.ValueChanged += (_, _) => Live(() => _cfg.DhuhrIqamah = (int)_iqDhuhr.Value);
        _iqAsr.ValueChanged += (_, _) => Live(() => _cfg.AsrIqamah = (int)_iqAsr.Value);
        _iqMaghrib.ValueChanged += (_, _) => Live(() => _cfg.MaghribIqamah = (int)_iqMaghrib.Value);
        _iqIsha.ValueChanged += (_, _) => Live(() => _cfg.IshaIqamah = (int)_iqIsha.Value);

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
        _testToast.Click += (_, _) => _testNotify?.Invoke(_richToasts.Checked);
    }

    void Live(Action set)
    {
        if (!_ready) return;
        set();
        _live?.Invoke();
    }

    // Language is a structural change (strings + RTL are fixed at construction): apply it, then ask the
    // host to reopen the dialog so it rebuilds in the new language.
    void OnLanguageChanged()
    {
        if (!_ready) return;
        string id = _langIds[Math.Max(0, _language.SelectedIndex)];
        if (id == _cfg.Language) return;
        _cfg.Language = id;
        Strings.Set(id);
        _live?.Invoke();          // pill/popup follow immediately
        RestartRequested = true;
        DialogResult = DialogResult.Retry; // closes; host reopens
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
        _hijriAdjust.Enabled = _showHijri.Checked;
        _pingHost.Enabled = _ping.Checked;
        _pingTcp.Enabled = _ping.Checked;
        _netIface.Enabled = _netSpeed.Checked || _ping.Checked || _trackUsage.Checked;
        _showUsage.Enabled = _trackUsage.Checked;
        _compactMeters.Enabled = _netSpeed.Checked || _ping.Checked || (_trackUsage.Checked && _showUsage.Checked);
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
                Msg(Strings.T("msg.detectFail"), Strings.T("msg.detectCaption"), MessageBoxIcon.Information);
            else ApplyDetected(loc);
        }
        finally { _detect.Enabled = true; _detect.Text = Strings.T("btn.detect"); }
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
            Msg(Strings.T("msg.pasteFail"), Strings.T("msg.pasteCaption"), MessageBoxIcon.Information);
            return;
        }
        ApplyDetected(loc);
    }

    void ApplyDetected(DetectedLocation loc)
    {
        _lat.Text = loc.Lat.ToString(CultureInfo.InvariantCulture);
        _lng.Text = loc.Lng.ToString(CultureInfo.InvariantCulture);
        if (!string.IsNullOrWhiteSpace(loc.City)) _city.Text = loc.City;
        else if (string.IsNullOrWhiteSpace(_city.Text)) _city.Text = Strings.T("city.myLocation");
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
        if (!TryCollect(_cfg)) return;
        _cfg.Save();
        _live?.Invoke();
    }

    // Validate the form and write every control into <paramref name="c"/>; false (after a Warn) if invalid.
    bool TryCollect(AppConfig c)
    {
        if (!TryLat(out var lat)) { Warn(Strings.T("msg.latRange")); return false; }
        if (!TryLng(out var lng)) { Warn(Strings.T("msg.lngRange")); return false; }
        string azanMode = _azanIds[Math.Max(0, _azan.SelectedIndex)];
        if (azanMode == "Custom" && string.IsNullOrWhiteSpace(_azanFile.Text))
        { Warn(Strings.T("msg.azanFile")); return false; }

        c.City = string.IsNullOrWhiteSpace(_city.Text) ? "Custom" : _city.Text.Trim();
        c.Latitude = lat; c.Longitude = lng;
        c.Method = new List<string>(CalcMethod.All.Keys)[_method.SelectedIndex];
        c.Asr = _asr.SelectedIndex == 1 ? (int)AsrJuristic.Hanafi : (int)AsrJuristic.Standard;
        c.HighLats = HighLatIds[Math.Max(0, _highLat.SelectedIndex)];
        c.FajrAdjust = Math.Clamp((int)_adjFajr.Value, -60, 60);
        c.DhuhrAdjust = Math.Clamp((int)_adjDhuhr.Value, -60, 60);
        c.AsrAdjust = Math.Clamp((int)_adjAsr.Value, -60, 60);
        c.MaghribAdjust = Math.Clamp((int)_adjMaghrib.Value, -60, 60);
        c.IshaAdjust = Math.Clamp((int)_adjIsha.Value, -60, 60);
        c.FajrIqamah = Math.Clamp((int)_iqFajr.Value, 0, 60);
        c.DhuhrIqamah = Math.Clamp((int)_iqDhuhr.Value, 0, 60);
        c.AsrIqamah = Math.Clamp((int)_iqAsr.Value, 0, 60);
        c.MaghribIqamah = Math.Clamp((int)_iqMaghrib.Value, 0, 60);
        c.IshaIqamah = Math.Clamp((int)_iqIsha.Value, 0, 60);
        c.WidgetAnchor = _position.SelectedIndex == 1 ? "Left" : "Right";
        c.Theme = Theme.Names[_theme.SelectedIndex];
        if (_font.SelectedItem is string fam && !string.IsNullOrWhiteSpace(fam)) c.FontFamily = fam;
        c.FontScalePct = SizeSteps[Math.Max(0, _fontSize.SelectedIndex)];
        var mon = _displays[_monitor.SelectedIndex];
        c.MonitorDeviceName = mon.Primary ? null : mon.DeviceName;
        if (int.TryParse(_offset.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var off))
            c.WidgetOffset = Math.Clamp(off, 0, 2000);
        c.Use24Hour = _h24.Checked;
        c.HideOnFullscreen = _hideFs.Checked;
        c.ShowNetSpeed = _netSpeed.Checked;
        c.ShowPing = _ping.Checked;
        c.PingHost = _pingHost.Text.Trim();
        c.PingTcp = _pingTcp.Checked;
        string ifaceId = _ifaceIds[Math.Max(0, _netIface.SelectedIndex)];
        c.NetInterfaceId = ifaceId.Length == 0 ? null : ifaceId;
        c.CompactMeters = _compactMeters.Checked;
        c.TrackDataUsage = _trackUsage.Checked;
        c.ShowDataUsage = _showUsage.Checked;
        c.ShowHijriDate = _showHijri.Checked;
        c.ShowIslamicEvents = _showEvents.Checked;
        c.SunnahFastReminder = _sunnahFast.Checked;
        c.FridayReminder = _fridayRem.Checked;
        c.HijriAdjust = Math.Clamp((int)_hijriAdjust.Value, -2, 2);

        c.RichToasts = _richToasts.Checked;
        c.ReminderEnabled = _remEnable.Checked;
        c.ReminderMinutes = Math.Clamp((int)_remMins.Value, 1, 60);
        c.ReminderSound = _remSound.Checked;
        c.ReminderSoundId = _remSoundIds[Math.Max(0, _remSoundCombo.SelectedIndex)];
        c.ReminderSoundPath = string.IsNullOrWhiteSpace(_remFile.Text) ? null : _remFile.Text.Trim();
        c.AzanMode = azanMode;
        c.AzanCustomPath = string.IsNullOrWhiteSpace(_azanFile.Text) ? null : _azanFile.Text.Trim();
        return true;
    }

    void OnExport(object? sender, EventArgs e)
    {
        var tmp = _cfg.Clone();
        if (!TryCollect(tmp)) return;
        using var d = new SaveFileDialog { Filter = "JSON|*.json", FileName = "PrayerTray-config.json" };
        if (d.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            File.WriteAllText(d.FileName, JsonSerializer.Serialize(tmp, new JsonSerializerOptions { WriteIndented = true }));
        }
        catch { Msg(Strings.T("msg.exportError"), Strings.T("msg.invalidCaption"), MessageBoxIcon.Warning); }
    }

    void OnImport(object? sender, EventArgs e)
    {
        using var d = new OpenFileDialog { Filter = "JSON|*.json|All files|*.*" };
        if (d.ShowDialog(this) != DialogResult.OK) return;
        AppConfig? imported = null;
        try { imported = JsonSerializer.Deserialize<AppConfig>(File.ReadAllText(d.FileName)); }
        catch { /* handled below */ }
        if (imported is null)
        {
            Msg(Strings.T("msg.importError"), Strings.T("msg.invalidCaption"), MessageBoxIcon.Warning);
            return;
        }

        string prevLang = _cfg.Language;
        imported.Sanitize();
        _cfg.CopyFrom(imported);
        _ready = false;
        LoadValues();
        _ready = true;
        SyncEnabled();
        if (_cfg.Language != prevLang)
        {
            // Rebuild in the imported language, like a manual language change.
            Strings.Set(_cfg.Language);
            _live?.Invoke();
            RestartRequested = true;
            DialogResult = DialogResult.Retry;
            return;
        }
        _live?.Invoke();
    }

    void Warn(string msg)
    {
        DialogResult = DialogResult.None;
        Msg(msg, Strings.T("msg.invalidCaption"), MessageBoxIcon.Warning);
    }

    DialogResult Msg(string text, string caption, MessageBoxIcon icon) =>
        MessageBox.Show(this, text, caption, MessageBoxButtons.OK, icon, MessageBoxDefaultButton.Button1, Strings.MsgOpts);

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

    void SelectSection(int idx)
    {
        for (int i = 0; i < _sectionCards.Count; i++) _sectionCards[i].Visible = i == idx;
        for (int i = 0; i < _navButtons.Count; i++) StyleNav(_navButtons[i], i == idx);
    }

    static void StyleNav(Button b, bool active)
    {
        b.BackColor = active ? Theme.Accent : Theme.Panel;
        b.ForeColor = active ? OnAccent() : Theme.Text;
        b.FlatAppearance.MouseOverBackColor = active ? Theme.Accent : Theme.BgHover;
    }

    static void RoundPanel(Panel p)
    {
        if (p.Handle == IntPtr.Zero || p.Width <= 0 || p.Height <= 0) return;
        Interop.SetWindowRgn(p.Handle, Interop.CreateRoundRectRgn(0, 0, p.Width + 1, p.Height + 1, 16, 16), true);
    }

    // --- theming ---
    void Stylize()
    {
        foreach (var cb in new[] { _method, _asr, _highLat, _position, _language, _theme, _font, _fontSize, _monitor, _netIface, _remSoundCombo, _azan }) StyleCombo(cb);
        foreach (var tb in new[] { _city, _paste, _lat, _lng, _offset, _pingHost, _remFile, _azanFile }) StyleText(tb);
        foreach (var ck in new[] { _h24, _hideFs, _netSpeed, _ping, _pingTcp, _compactMeters, _trackUsage, _showUsage, _showHijri, _showEvents, _sunnahFast, _fridayRem, _richToasts, _remEnable, _remSound }) StyleCheck(ck);
        StyleNumeric(_remMins);
        StyleNumeric(_hijriAdjust);
        foreach (var n in new[] { _adjFajr, _adjDhuhr, _adjAsr, _adjMaghrib, _adjIsha, _iqFajr, _iqDhuhr, _iqAsr, _iqMaghrib, _iqIsha }) StyleNumeric(n);
        var btns = new List<Button> { _openMap, _setPaste, _testToast, _remBrowse, _remTest, _azanBrowse, _azanTest, _azanStop };
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
        if (Strings.IsRtl) c.RightToLeft = RightToLeft.Yes;
        c.DrawMode = DrawMode.OwnerDrawFixed;
        c.DrawItem += ComboDraw;
    }

    static void ComboDraw(object? sender, DrawItemEventArgs e)
    {
        var c = (ComboBox)sender!;
        bool sel = (e.State & DrawItemState.Selected) != 0;
        using (var bg = new SolidBrush(sel ? Theme.AccentSoft : Theme.BgHover)) e.Graphics.FillRectangle(bg, e.Bounds);
        if (e.Index < 0) return;
        var sf = new StringFormat { LineAlignment = StringAlignment.Center };
        if (Strings.IsRtl) sf.Alignment = StringAlignment.Far;
        var rect = new RectangleF(e.Bounds.X + 2, e.Bounds.Y, e.Bounds.Width - 4, e.Bounds.Height);
        using var b = new SolidBrush(Theme.Text);
        e.Graphics.DrawString(c.Items[e.Index]?.ToString(), e.Font ?? c.Font, b, rect, sf);
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
