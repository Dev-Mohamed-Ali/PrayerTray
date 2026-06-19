using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using PrayerTray.Native;

namespace PrayerTray.UI;

/// <summary>Borderless popup listing today's times, shown above the widget on click.</summary>
public class PrayerPopup : Form
{
    static readonly (string key, string label)[] Order =
    {
        ("fajr", "Fajr"), ("sunrise", "Sunrise"), ("dhuhr", "Dhuhr"),
        ("asr", "Asr"), ("maghrib", "Maghrib"), ("isha", "Isha"),
    };

    record Row(string Label, string Time, bool IsNext, bool IsSunrise);

    string _city = "", _date = "", _countdown = "", _nextLabel = "";
    readonly List<Row> _rows = new();
    Rectangle _widget;
    bool _anchorRight = true;

    const int Pad = 16, RowH = 38, HeaderH = 60;

    public PrayerPopup()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        BackColor = Theme.Panel;
        DoubleBuffered = true;
        TopMost = true;
        Width = 268;
        Deactivate += (_, _) => Hide();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Interop.RoundCorners(Handle);
    }

    public void ShowTimes(string city, DateTime date, Dictionary<string, TimeSpan> times,
        string? nextKey, bool use24, string countdown, Rectangle widgetRect, bool anchorRight)
    {
        _widget = widgetRect;
        _anchorRight = anchorRight;
        BackColor = Theme.Panel;
        _city = city;
        _date = date.ToString("dddd, dd MMM");
        _countdown = countdown;
        _rows.Clear();
        foreach (var (key, label) in Order)
        {
            if (!times.TryGetValue(key, out var ts)) continue;
            bool isNext = key == nextKey;
            if (isNext) _nextLabel = label;
            _rows.Add(new Row(label, Format(ts, use24), isNext, key == "sunrise"));
        }

        Height = HeaderH + _rows.Count * RowH + Pad;
        Position();
        Invalidate();
        Show();
        Activate();
    }

    public static string Format(TimeSpan ts, bool use24)
    {
        var dt = DateTime.Today.Add(ts);
        return use24 ? dt.ToString("HH:mm") : dt.ToString("h:mm tt");
    }

    void Position()
    {
        var wa = Screen.PrimaryScreen!.Bounds;
        int x = _anchorRight ? _widget.Right - Width : _widget.Left;
        x = Math.Clamp(x, wa.Left + 8, wa.Right - Width - 8);
        int y = _widget.Top - 8 - Height;
        Location = new Point(x, y);
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;
        g.Clear(Theme.Panel);

        using var fCity = new Font(Theme.Family, 11f, FontStyle.Bold);
        using var fDate = new Font(Theme.Family, 8.5f, FontStyle.Regular);
        using var fRow = new Font(Theme.Family, 10.5f, FontStyle.Regular);
        using var fRowB = new Font(Theme.Family, 10.5f, FontStyle.Bold);
        using var fChip = new Font(Theme.Family, 9f, FontStyle.Bold);

        using (var b = new SolidBrush(Theme.Text)) g.DrawString(_city, fCity, b, Pad, 12);
        using (var b = new SolidBrush(Theme.TextDim)) g.DrawString(_date, fDate, b, Pad, 34);

        if (!string.IsNullOrEmpty(_countdown))
        {
            string chip = $"{_nextLabel} in {_countdown}";
            var sz = g.MeasureString(chip, fChip);
            var chipRect = new RectangleF(Width - Pad - sz.Width - 18, 16, sz.Width + 16, 24);
            using (var b = new SolidBrush(Theme.AccentSoft)) FillRounded(g, b, chipRect, 12);
            using (var b = new SolidBrush(Theme.Accent)) g.DrawString(chip, fChip, b, chipRect.X + 8, chipRect.Y + 4);
        }

        using (var p = new Pen(Color.FromArgb(60, 60, 64))) g.DrawLine(p, Pad, HeaderH - 6, Width - Pad, HeaderH - 6);

        int y = HeaderH;
        foreach (var r in _rows)
        {
            var rowRect = new Rectangle(8, y, Width - 16, RowH - 4);
            if (r.IsNext)
            {
                using var b = new SolidBrush(Theme.AccentSoft);
                FillRounded(g, b, rowRect, 10);
                using var bar = new SolidBrush(Theme.Accent);
                FillRounded(g, bar, new RectangleF(rowRect.X + 2, rowRect.Y + 7, 4, rowRect.Height - 14), 2);
            }

            Color fg = r.IsNext ? Theme.Accent : r.IsSunrise ? Theme.TextDim : Theme.Text;
            var font = r.IsNext ? fRowB : fRow;
            var sf = new StringFormat { LineAlignment = StringAlignment.Center };
            var sfR = new StringFormat { LineAlignment = StringAlignment.Center, Alignment = StringAlignment.Far };
            var rect = new RectangleF(Pad + 6, y, Width - 2 * Pad - 6, RowH - 4);
            using (var b = new SolidBrush(fg))
            {
                g.DrawString(r.Label, font, b, rect, sf);
                g.DrawString(r.Time, font, b, rect, sfR);
            }
            y += RowH;
        }
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
