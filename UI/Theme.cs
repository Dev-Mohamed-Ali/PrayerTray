using System;
using System.Drawing;
using Microsoft.Win32;

namespace PrayerTray.UI;

public record Palette(
    Color Bg, Color BgHover, Color Panel, Color Text, Color TextDim,
    Color Accent, Color AccentSoft, Color Good, bool IsDark);

internal static class Theme
{
    // "Auto" follows the Windows app theme; the rest are fixed presets shown in Settings.
    public static readonly string[] Names = { "Auto", "Dark", "Light", "Midnight", "Slate", "Warm" };

    public static readonly Palette Dark = new(
        Color.FromArgb(32, 32, 32), Color.FromArgb(48, 48, 50), Color.FromArgb(43, 43, 46),
        Color.FromArgb(240, 240, 240), Color.FromArgb(165, 170, 178),
        Color.FromArgb(96, 205, 255), Color.FromArgb(38, 64, 78), Color.FromArgb(126, 224, 158), true);

    public static readonly Palette Light = new(
        Color.FromArgb(243, 243, 243), Color.FromArgb(225, 225, 228), Color.FromArgb(250, 250, 252),
        Color.FromArgb(24, 24, 24), Color.FromArgb(92, 98, 108),
        Color.FromArgb(0, 120, 200), Color.FromArgb(205, 230, 245), Color.FromArgb(30, 150, 80), false);

    public static readonly Palette Midnight = new(
        Color.FromArgb(16, 22, 40), Color.FromArgb(28, 36, 60), Color.FromArgb(22, 30, 52),
        Color.FromArgb(232, 236, 245), Color.FromArgb(150, 160, 185),
        Color.FromArgb(120, 160, 255), Color.FromArgb(36, 48, 90), Color.FromArgb(120, 220, 170), true);

    public static readonly Palette Slate = new(
        Color.FromArgb(30, 34, 42), Color.FromArgb(44, 50, 60), Color.FromArgb(38, 43, 52),
        Color.FromArgb(232, 236, 242), Color.FromArgb(150, 158, 170),
        Color.FromArgb(130, 180, 210), Color.FromArgb(48, 62, 76), Color.FromArgb(130, 210, 165), true);

    public static readonly Palette Warm = new(
        Color.FromArgb(34, 30, 26), Color.FromArgb(50, 44, 38), Color.FromArgb(44, 39, 34),
        Color.FromArgb(244, 238, 230), Color.FromArgb(180, 168, 150),
        Color.FromArgb(255, 180, 90), Color.FromArgb(78, 60, 38), Color.FromArgb(200, 210, 120), true);

    public static Palette Current { get; private set; } = Dark;

    public static void Apply(string? name) => Current = Resolve(name);

    static Palette Resolve(string? name) => name switch
    {
        "Dark" => Dark,
        "Light" => Light,
        "Midnight" => Midnight,
        "Slate" => Slate,
        "Warm" => Warm,
        _ => WindowsUsesLightTheme() ? Light : Dark, // "Auto" or unknown
    };

    static bool WindowsUsesLightTheme()
    {
        try
        {
            using var k = Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return k?.GetValue("AppsUseLightTheme") is int v && v != 0;
        }
        catch { return false; }
    }

    public static Color Bg => Current.Bg;
    public static Color BgHover => Current.BgHover;
    public static Color Panel => Current.Panel;
    public static Color Text => Current.Text;
    public static Color TextDim => Current.TextDim;
    public static Color Accent => Current.Accent;
    public static Color AccentSoft => Current.AccentSoft;
    public static Color Good => Current.Good;

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
