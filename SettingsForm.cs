using System;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;

namespace PrayerTray;

public class SettingsForm : Form
{
    readonly AppConfig _cfg;
    readonly TextBox _city = new() { Width = 180 };
    readonly TextBox _lat = new() { Width = 180 };
    readonly TextBox _lng = new() { Width = 180 };
    readonly ComboBox _method = new() { Width = 180, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _asr = new() { Width = 180, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly ComboBox _position = new() { Width = 180, DropDownStyle = ComboBoxStyle.DropDownList };
    readonly TextBox _offset = new() { Width = 180 };
    readonly CheckBox _h24 = new() { Text = "Use 24-hour clock", AutoSize = true };

    public SettingsForm(AppConfig cfg)
    {
        _cfg = cfg;
        Text = "Prayer Tray — Settings";
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterScreen;
        MaximizeBox = false; MinimizeBox = false;
        AutoSize = true; AutoSizeMode = AutoSizeMode.GrowAndShrink;
        Padding = new Padding(12);

        var layout = new TableLayoutPanel { ColumnCount = 2, AutoSize = true, Dock = DockStyle.Fill };
        AddRow(layout, "City (label):", _city);
        AddRow(layout, "Latitude:", _lat);
        AddRow(layout, "Longitude:", _lng);
        AddRow(layout, "Method:", _method);
        AddRow(layout, "Asr:", _asr);
        AddRow(layout, "Widget side:", _position);
        AddRow(layout, "Widget gap (px):", _offset);
        layout.Controls.Add(new Label { Width = 1 }, 0, layout.RowCount);
        layout.Controls.Add(_h24, 1, layout.RowCount - 1);

        foreach (var (key, m) in CalcMethod.All) _method.Items.Add($"{key} — {m.Name}");
        _asr.Items.Add("Standard (Shafi'i, Maliki, Hanbali)");
        _asr.Items.Add("Hanafi");
        _position.Items.Add("Right (near the clock)");
        _position.Items.Add("Left (corner)");

        _city.Text = cfg.City;
        _lat.Text = cfg.Latitude.ToString(CultureInfo.InvariantCulture);
        _lng.Text = cfg.Longitude.ToString(CultureInfo.InvariantCulture);
        _method.SelectedIndex = Math.Max(0, IndexOfMethod(cfg.Method));
        _asr.SelectedIndex = cfg.Asr == (int)AsrJuristic.Hanafi ? 1 : 0;
        _position.SelectedIndex = string.Equals(cfg.WidgetAnchor, "Left", StringComparison.OrdinalIgnoreCase) ? 1 : 0;
        _offset.Text = cfg.WidgetOffset.ToString(CultureInfo.InvariantCulture);
        _h24.Checked = cfg.Use24Hour;

        var ok = new Button { Text = "Save", DialogResult = DialogResult.OK, Width = 80 };
        var cancel = new Button { Text = "Cancel", DialogResult = DialogResult.Cancel, Width = 80 };
        ok.Click += OnSave;
        var buttons = new FlowLayoutPanel { FlowDirection = FlowDirection.RightToLeft, Dock = DockStyle.Fill, AutoSize = true };
        buttons.Controls.Add(cancel);
        buttons.Controls.Add(ok);

        var root = new TableLayoutPanel { ColumnCount = 1, AutoSize = true, Dock = DockStyle.Fill };
        root.Controls.Add(layout);
        root.Controls.Add(buttons);
        Controls.Add(root);
        AcceptButton = ok; CancelButton = cancel;
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Interop.DarkTitleBar(Handle);
    }

    static int IndexOfMethod(string key)
    {
        int i = 0;
        foreach (var k in CalcMethod.All.Keys) { if (k == key) return i; i++; }
        return 0;
    }

    static void AddRow(TableLayoutPanel t, string label, Control c)
    {
        int r = t.RowCount;
        t.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left, Margin = new Padding(3, 6, 3, 3) }, 0, r);
        t.Controls.Add(c, 1, r);
        t.RowCount++;
    }

    void OnSave(object? sender, EventArgs e)
    {
        if (!double.TryParse(_lat.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var lat) || lat < -90 || lat > 90)
        { Warn("Latitude must be a number between -90 and 90."); return; }
        if (!double.TryParse(_lng.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var lng) || lng < -180 || lng > 180)
        { Warn("Longitude must be a number between -180 and 180."); return; }

        _cfg.City = string.IsNullOrWhiteSpace(_city.Text) ? "Custom" : _city.Text.Trim();
        _cfg.Latitude = lat;
        _cfg.Longitude = lng;
        var keys = new List<string>(CalcMethod.All.Keys);
        _cfg.Method = keys[_method.SelectedIndex];
        _cfg.Asr = _asr.SelectedIndex == 1 ? (int)AsrJuristic.Hanafi : (int)AsrJuristic.Standard;
        _cfg.WidgetAnchor = _position.SelectedIndex == 1 ? "Left" : "Right";
        if (int.TryParse(_offset.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var off))
            _cfg.WidgetOffset = Math.Clamp(off, 0, 2000);
        _cfg.Use24Hour = _h24.Checked;
        _cfg.Save();
    }

    void Warn(string msg)
    {
        DialogResult = DialogResult.None;
        MessageBox.Show(this, msg, "Invalid input", MessageBoxButtons.OK, MessageBoxIcon.Warning);
    }
}
