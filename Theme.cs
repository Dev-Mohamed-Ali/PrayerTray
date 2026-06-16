using System.Drawing;

namespace PrayerTray;

internal static class Theme
{
    public static readonly Color Bg = Color.FromArgb(32, 32, 32);     // ~ taskbar dark
    public static readonly Color BgHover = Color.FromArgb(48, 48, 50);
    public static readonly Color Panel = Color.FromArgb(43, 43, 46);
    public static readonly Color Text = Color.FromArgb(240, 240, 240);
    public static readonly Color TextDim = Color.FromArgb(165, 170, 178);
    public static readonly Color Accent = Color.FromArgb(96, 205, 255);   // cyan
    public static readonly Color AccentSoft = Color.FromArgb(38, 64, 78);
    public static readonly Color Good = Color.FromArgb(126, 224, 158);    // green countdown

    public const string Family = "Segoe UI";

    // Bottom strip reserved by the taskbar on the primary screen.
    public static Rectangle TaskbarStrip()
    {
        var s = System.Windows.Forms.Screen.PrimaryScreen!;
        var b = s.Bounds;
        var wa = s.WorkingArea;
        int h = b.Bottom - wa.Bottom;
        if (h <= 0) h = 48; // taskbar auto-hidden or on another edge -> assume 48px bottom
        return new Rectangle(b.Left, wa.Bottom, b.Width, h);
    }
}
