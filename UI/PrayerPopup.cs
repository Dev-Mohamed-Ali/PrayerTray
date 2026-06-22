using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using PrayerTray.I18n;
using PrayerTray.Native;

namespace PrayerTray.UI;

/// <summary>Borderless popup listing today's times, shown above the widget on click.
/// Can be pinned to stay open (draggable, remembers position).</summary>
public class PrayerPopup : Form
{
    static readonly string[] Order = { "fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha" };

    record Row(string Label, string Time, bool IsNext, bool IsSunrise);

    public event Action<bool>? PinChanged;     // user toggled the pin
    public event Action<int, int>? Moved;      // pinned popup dragged to a new spot

    string _city = "", _date = "", _hijri = "", _countdown = "", _nextLabel = "";
    readonly List<Row> _rows = new();
    Rectangle _widget;
    bool _anchorRight = true;

    bool _pinned;
    int _savedX = int.MinValue, _savedY = int.MinValue;
    bool _positioning;          // suppress Moved while we set Location programmatically
    Rectangle _pinBox;

    int Pad => Scaled(16);
    int RowH => Scaled(38);
    int HeaderH => Scaled(_hijri.Length > 0 ? 74 : 60);
    static int Scaled(int v) => (int)Math.Round(v * Theme.FontScale);

    public PrayerPopup()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        BackColor = Theme.Panel;
        DoubleBuffered = true;
        TopMost = true;
        Width = Scaled(268);
        Deactivate += (_, _) => { if (!_pinned) Hide(); };
        MouseDown += OnMouseDown;
        MouseUp += OnMouseUp;
        LocationChanged += (_, _) => { if (_pinned && !_positioning) SavePosition(); };
    }

    /// <summary>Seed pin state + last position from config (called by AppHost).</summary>
    public void InitPin(bool pinned, int x, int y) { _pinned = pinned; _savedX = x; _savedY = y; }
    public bool Pinned => _pinned;

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Interop.RoundCorners(Handle);
    }

    public void ShowTimes(string city, DateTime date, Dictionary<string, TimeSpan> times,
        string? nextKey, bool use24, string countdown, Rectangle widgetRect, bool anchorRight,
        string hijri = "")
    {
        _widget = widgetRect;
        _anchorRight = anchorRight;
        BackColor = Theme.Panel;
        _city = city;
        _date = Strings.FormatPopupDate(date);
        _hijri = hijri;
        _countdown = countdown;
        _rows.Clear();
        foreach (var key in Order)
        {
            if (!times.TryGetValue(key, out var ts)) continue;
            string label = Strings.Prayer(key);
            bool isNext = key == nextKey;
            if (isNext) _nextLabel = label;
            _rows.Add(new Row(label, Format(ts, use24), isNext, key == "sunrise"));
        }

        Width = Scaled(268);
        Height = HeaderH + _rows.Count * RowH + Pad;
        Position();
        Invalidate();
        Show();
        Activate();
    }

    public static string Format(TimeSpan ts, bool use24)
    {
        var dt = DateTime.Today.Add(ts);
        return use24 ? dt.ToString("HH:mm") : $"{dt:h:mm} {Strings.AmPm(dt)}";
    }

    void Position()
    {
        _positioning = true;
        try
        {
            if (_pinned && _savedX != int.MinValue)
            {
                var wa = Screen.FromPoint(new Point(_savedX, _savedY)).WorkingArea;
                Location = new Point(
                    Math.Clamp(_savedX, wa.Left, Math.Max(wa.Left, wa.Right - Width)),
                    Math.Clamp(_savedY, wa.Top, Math.Max(wa.Top, wa.Bottom - Height)));
                return;
            }
            var b = Screen.FromRectangle(_widget).Bounds;
            int x = _anchorRight ? _widget.Right - Width : _widget.Left;
            x = Math.Clamp(x, b.Left + 8, b.Right - Width - 8);
            int y = _widget.Top - 8 - Height;
            Location = new Point(x, y);
        }
        finally { _positioning = false; }
    }

    void SavePosition()
    {
        _savedX = Location.X; _savedY = Location.Y;
        Moved?.Invoke(_savedX, _savedY);
    }

    void OnMouseDown(object? sender, MouseEventArgs e)
    {
        if (e.Button != MouseButtons.Left) return;
        if (_pinBox.Contains(e.Location)) return;            // pin toggles on mouse-up
        if (e.Y < HeaderH)                                   // drag from the header
        {
            Interop.ReleaseCapture();
            Interop.SendMessage(Handle, Interop.WM_NCLBUTTONDOWN, (IntPtr)Interop.HTCAPTION, IntPtr.Zero);
        }
    }

    void OnMouseUp(object? sender, MouseEventArgs e)
    {
        if (e.Button == MouseButtons.Left && _pinBox.Contains(e.Location))
        {
            _pinned = !_pinned;
            PinChanged?.Invoke(_pinned);
            if (_pinned) { _savedX = Location.X; _savedY = Location.Y; Moved?.Invoke(_savedX, _savedY); }
            Invalidate();
        }
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;
        g.Clear(Theme.Panel);

        bool rtl = Strings.IsRtl;
        float fs = Theme.FontScale;
        using var fCity = new Font(Theme.Family, 11f * fs, FontStyle.Bold);
        using var fDate = new Font(Theme.Family, 8.5f * fs, FontStyle.Regular);
        using var fRow = new Font(Theme.Family, 10.5f * fs, FontStyle.Regular);
        using var fRowB = new Font(Theme.Family, 10.5f * fs, FontStyle.Bold);
        using var fChip = new Font(Theme.Family, 9f * fs, FontStyle.Bold);

        // Measure the countdown chip first so the date row can reserve room for it (avoids overlap).
        string chip = string.IsNullOrEmpty(_countdown) ? "" : $"{_nextLabel} {Strings.T("popup.in")} {_countdown}";
        float chipW = chip.Length == 0 ? 0 : g.MeasureString(chip, fChip).Width + Scaled(14);

        var sfHdr = new StringFormat { Alignment = rtl ? StringAlignment.Far : StringAlignment.Near };
        var hdrRect = new RectangleF(Pad, Scaled(12), Width - 2 * Pad, Scaled(22));
        float dateW = Width - 2 * Pad - (chipW > 0 ? chipW + Scaled(6) : 0);
        var dateRect = new RectangleF(rtl ? Width - Pad - dateW : Pad, Scaled(34), dateW, Scaled(20));
        using (var b = new SolidBrush(Theme.Text)) g.DrawString(_city, fCity, b, hdrRect, sfHdr);
        using (var b = new SolidBrush(Theme.TextDim)) g.DrawString(_date, fDate, b, dateRect, sfHdr);
        if (_hijri.Length > 0)
        {
            var hijriRect = new RectangleF(Pad, Scaled(52), Width - 2 * Pad, Scaled(18));
            using var b = new SolidBrush(Theme.TextDim);
            g.DrawString(_hijri, fDate, b, hijriRect, sfHdr);
        }

        DrawPin(g, rtl);

        if (chipW > 0)
        {
            float chipX = rtl ? Pad + Scaled(2) : Width - Pad - chipW - Scaled(2);
            var chipRect = new RectangleF(chipX, Scaled(34), chipW, Scaled(22));
            using (var b = new SolidBrush(Theme.AccentSoft)) FillRounded(g, b, chipRect, Scaled(11));
            using (var b = new SolidBrush(Theme.Accent)) g.DrawString(chip, fChip, b, chipRect.X + Scaled(7), chipRect.Y + Scaled(3));
        }

        using (var p = new Pen(Color.FromArgb(60, 60, 64))) g.DrawLine(p, Pad, HeaderH - Scaled(6), Width - Pad, HeaderH - Scaled(6));

        var sfLabel = new StringFormat { LineAlignment = StringAlignment.Center, Alignment = rtl ? StringAlignment.Far : StringAlignment.Near };
        var sfTime = new StringFormat { LineAlignment = StringAlignment.Center, Alignment = rtl ? StringAlignment.Near : StringAlignment.Far };
        int y = HeaderH;
        foreach (var r in _rows)
        {
            var rowRect = new Rectangle(Scaled(8), y, Width - Scaled(16), RowH - Scaled(4));
            if (r.IsNext)
            {
                using var b = new SolidBrush(Theme.AccentSoft);
                FillRounded(g, b, rowRect, Scaled(10));
                using var bar = new SolidBrush(Theme.Accent);
                float barX = rtl ? rowRect.Right - Scaled(6) : rowRect.X + 2;
                FillRounded(g, bar, new RectangleF(barX, rowRect.Y + Scaled(7), 4, rowRect.Height - Scaled(14)), 2);
            }

            Color fg = r.IsNext ? Theme.Accent : r.IsSunrise ? Theme.TextDim : Theme.Text;
            var font = r.IsNext ? fRowB : fRow;
            var rect = new RectangleF(Pad + Scaled(6), y, Width - 2 * Pad - Scaled(6), RowH - Scaled(4));
            using (var b = new SolidBrush(fg))
            {
                g.DrawString(r.Label, font, b, rect, sfLabel);
                g.DrawString(r.Time, font, b, rect, sfTime);
            }
            y += RowH;
        }
    }

    // Thumbtack in the top corner (right for LTR, left for RTL); filled accent when pinned, hollow otherwise.
    void DrawPin(Graphics g, bool rtl)
    {
        int s = Scaled(18);
        _pinBox = new Rectangle(rtl ? Pad : Width - Pad - s, Scaled(11), s, s);
        var c = _pinned ? Theme.Accent : Theme.TextDim;
        float hx = _pinBox.X + s * 0.5f, hy = _pinBox.Y + s * 0.34f, hr = s * 0.30f;
        using var pen = new Pen(c, Math.Max(1.4f, s * 0.10f));
        g.DrawLine(pen, hx, hy + hr, hx, _pinBox.Bottom - Scaled(2)); // needle
        if (_pinned) { using var b = new SolidBrush(c); g.FillEllipse(b, hx - hr, hy - hr, hr * 2, hr * 2); }
        else g.DrawEllipse(pen, hx - hr, hy - hr, hr * 2, hr * 2);    // head
    }

    static void FillRounded(Graphics g, Brush b, RectangleF r, int radius)
    {
        using var path = new GraphicsPath();
        float d = radius * 2;
        path.AddArc(r.X, r.Y, d, d, 180, 90);
        path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        g.FillPath(b, path);
    }
}
