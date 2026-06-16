using System;
using System.Drawing;
using System.Runtime.InteropServices;

namespace PrayerTray;

/// <summary>All Win32/DWM interop in one place: taskbar lookup, geometry, child-window plumbing, DPI, DWM styling.</summary>
internal static class Interop
{
    // --- window lookup / geometry ---
    [DllImport("user32.dll", SetLastError = true)]
    static extern IntPtr FindWindow(string? cls, string? win);
    [DllImport("user32.dll", SetLastError = true)]
    static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string? cls, string? win);
    [DllImport("user32.dll")]
    static extern bool GetWindowRect(IntPtr h, out RECT r);

    public static IntPtr Taskbar() => FindWindow("Shell_TrayWnd", null);

    public static Rectangle WindowRect(IntPtr h) =>
        h != IntPtr.Zero && GetWindowRect(h, out var r) ? Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom) : Rectangle.Empty;

    /// <summary>Screen-x of the system tray (clock/notification cluster) left edge, or 0 if not found.</summary>
    public static int TrayNotifyLeft()
    {
        var tb = Taskbar();
        if (tb == IntPtr.Zero) return 0;
        var notify = FindWindowEx(tb, IntPtr.Zero, "TrayNotifyWnd", null);
        return notify != IntPtr.Zero && GetWindowRect(notify, out var r) ? r.Left : 0;
    }

    // --- overlay window plumbing ---
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool InvalidateRect(IntPtr h, IntPtr rect, bool erase);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);

    static readonly IntPtr HWND_TOPMOST = new(-1);
    const uint SWP_NOMOVE = 0x2, SWP_NOSIZE = 0x1, SWP_NOACTIVATE = 0x10;

    /// <summary>Re-assert top-most z-order without moving/activating.</summary>
    public static void RaiseTopmost(IntPtr h) =>
        SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);

    // --- event hook: re-raise instantly when the taskbar comes forward (no visible flicker) ---
    public delegate void WinEventProc(IntPtr hook, uint evt, IntPtr hwnd, int idObj, int idChild, uint thread, uint time);
    [DllImport("user32.dll")]
    public static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr hmod, WinEventProc cb, uint pid, uint tid, uint flags);
    [DllImport("user32.dll")] public static extern bool UnhookWinEvent(IntPtr hook);

    public const uint EVENT_SYSTEM_FOREGROUND = 0x0003, EVENT_OBJECT_REORDER = 0x8004, WINEVENT_OUTOFCONTEXT = 0;

    public const int SW_SHOWNA = 8;

    // --- painting ---
    [DllImport("user32.dll")] public static extern IntPtr BeginPaint(IntPtr h, ref PAINTSTRUCT ps);
    [DllImport("user32.dll")] public static extern bool EndPaint(IntPtr h, ref PAINTSTRUCT ps);
    [DllImport("user32.dll")] public static extern bool TrackMouseEvent(ref TRACKMOUSEEVENT tme);
    [DllImport("user32.dll")] public static extern int SetWindowRgn(IntPtr h, IntPtr rgn, bool redraw);
    [DllImport("gdi32.dll")] public static extern IntPtr CreateRoundRectRgn(int l, int t, int r, int b, int w, int h);

    public const int TME_LEAVE = 0x2;

    // --- DPI ---
    [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);

    public static float Scale(IntPtr h)
    {
        if (h == IntPtr.Zero) return 1f;
        uint dpi = GetDpiForWindow(h);
        return dpi == 0 ? 1f : dpi / 96f;
    }

    // --- broadcast / DWM ---
    [DllImport("user32.dll")] public static extern uint RegisterWindowMessage(string msg);
    [DllImport("dwmapi.dll")] static extern int DwmSetWindowAttribute(IntPtr h, int attr, ref int val, int size);

    const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2;

    public static void DarkTitleBar(IntPtr h) { int v = 1; DwmSetWindowAttribute(h, DWMWA_USE_IMMERSIVE_DARK_MODE, ref v, 4); }
    public static void RoundCorners(IntPtr h) { int v = DWMWCP_ROUND; DwmSetWindowAttribute(h, DWMWA_WINDOW_CORNER_PREFERENCE, ref v, 4); }

    // --- structs / messages ---
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct TRACKMOUSEEVENT { public int cbSize; public int dwFlags; public IntPtr hwndTrack; public int dwHoverTime; }

    [StructLayout(LayoutKind.Sequential)]
    public struct PAINTSTRUCT
    {
        public IntPtr hdc; public bool fErase; public int l, t, r, b;
        public bool fRestore; public bool fIncUpdate;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)] public byte[] rgbReserved;
    }

    public const int WM_PAINT = 0x000F, WM_ERASEBKGND = 0x0014, WM_MOUSEMOVE = 0x0200,
        WM_MOUSELEAVE = 0x02A3, WM_LBUTTONUP = 0x0202, WM_RBUTTONUP = 0x0205, WM_MOUSEACTIVATE = 0x0021;
    public const int MA_NOACTIVATE = 3;
}
