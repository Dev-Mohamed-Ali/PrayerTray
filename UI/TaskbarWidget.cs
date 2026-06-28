using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using PrayerTray.I18n;
using PrayerTray.Native;
using PrayerTray.Services;

namespace PrayerTray.UI;

/// <summary>
/// The next-prayer pill, created as a real WS_CHILD of the taskbar (Shell_TrayWnd) so it is painted
/// with the taskbar and cannot be covered by taskbar clicks. Raw NativeWindow — a WinForms Form forces
/// itself back to top-level and won't stay embedded.
/// </summary>
public sealed class TaskbarWidget : NativeWindow, IDisposable
{
    public event Action? Clicked;
    public event Action<Point>? RightClicked;
    public bool AnchorRight { get; set; } = true;
    public bool HideOnFullscreen { get; set; } = true;
    public int Offset { get; set; } = 12;
    readonly string? _deviceName; // target monitor (null = primary)
    public string? DeviceName => _deviceName;

    string _name = "—", _time = "", _count = "…", _net = "";
    bool _hover, _tracking, _paused, _suppressed;
    int _w = 160, _h = 32;
    float _scale = 1f;
    Rectangle _lastRect = Rectangle.Empty;
    Bitmap? _buffer;
    readonly Bitmap _measureBmp = new(1, 1);
    readonly Graphics _measure;

    // Top-level always-on-top overlay owned by the taskbar. (Embedding as a child of Shell_TrayWnd is
    // invisible on Win11 — the modern DesktopWindowContentBridge composition surface covers GDI children.)
    const int WS_POPUP = unchecked((int)0x80000000), WS_VISIBLE = 0x10000000;
    const int WS_EX_NOACTIVATE = 0x08000000, WS_EX_TOOLWINDOW = 0x00000080, WS_EX_TOPMOST = 0x00000008;

    Interop.WinEventProc? _winCb;
    IntPtr _fgHook, _reorderHook;
    int _lastRaise;

    public TaskbarWidget(string? deviceName = null)
    {
        _deviceName = deviceName;
        _measure = Graphics.FromImage(_measureBmp);
        var tb = Interop.TaskbarForDevice(_deviceName);
        _scale = Interop.Scale(tb);
        CreateHandle(new CreateParams
        {
            ClassName = null,
            Style = WS_POPUP | WS_VISIBLE,
            ExStyle = WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            Parent = tb, // owner -> rides in the taskbar's z-band
            X = 0, Y = 0, Width = _w, Height = _h,
        });

        // Re-raise the instant the taskbar comes forward (e.g. a taskbar/desktop click) -> no flicker.
        // Foreground hook is global (infrequent); the chatty reorder hook is scoped to the taskbar thread.
        _winCb = OnWinEvent;
        uint tid = tb != IntPtr.Zero ? Interop.GetWindowThreadProcessId(tb, out _) : 0;
        _fgHook = Interop.SetWinEventHook(Interop.EVENT_SYSTEM_FOREGROUND, Interop.EVENT_SYSTEM_FOREGROUND,
            IntPtr.Zero, _winCb, 0, 0, Interop.WINEVENT_OUTOFCONTEXT);
        // Scope the chatty reorder hook to the taskbar thread; never install it system-wide (tid==0).
        if (tid != 0)
            _reorderHook = Interop.SetWinEventHook(Interop.EVENT_OBJECT_REORDER, Interop.EVENT_OBJECT_REORDER,
                IntPtr.Zero, _winCb, 0, tid, Interop.WINEVENT_OUTOFCONTEXT);

        RenderBuffer();
        SyncPosition();
    }

    void OnWinEvent(IntPtr hook, uint evt, IntPtr hwnd, int o, int c, uint t, uint tm)
    {
        if (Handle == IntPtr.Zero || _paused) return;
        int now = Environment.TickCount;
        if (now - _lastRaise < 16) return; // debounce the chatty reorder events
        _lastRaise = now;
        UpdateVisibility();
    }

    // Single authority for show/hide/raise. Owned by the taskbar, so re-raising over a fullscreen app
    // drags the whole taskbar up (Win10) — suppress instead. Driven by the win-event hooks (instant)
    // and the 1s safety Tick.
    public void Tick() => UpdateVisibility();

    void UpdateVisibility()
    {
        if (Handle == IntPtr.Zero || _paused) return;
        if (HideOnFullscreen && Interop.IsFullscreenAppOnScreen(TargetScreen().Bounds))
        {
            if (!_suppressed) { _suppressed = true; Hide(); }
            return;
        }
        if (_suppressed) { _suppressed = false; Show(); }
        SyncPosition();
    }

    public Rectangle ScreenRect => Interop.WindowRect(Handle);
    public bool Visible => Interop.IsWindowVisible(Handle);
    public void Show() => Interop.ShowWindow(Handle, Interop.SW_SHOWNA);
    public void Hide() => Interop.ShowWindow(Handle, Interop.SW_HIDE);

    // Stop fighting modal dialogs: a re-raise (SetWindowPos topmost) dismisses an open combo dropdown.
    public void Pause() => _paused = true;
    public void Resume() { _paused = false; UpdateVisibility(); }

    int S(float v) => (int)Math.Round(v * _scale);
    Font MainFont() => new(Theme.Family, 13f * _scale * Theme.FontScale, FontStyle.Regular, GraphicsUnit.Pixel);
    Font CountFont() => new(Theme.Family, 13f * _scale * Theme.FontScale, FontStyle.Bold, GraphicsUnit.Pixel);

    public void SetData(string name, string time, string countdown)
    {
        _name = name; _time = time; _count = countdown;
        ResizeToContent();
        RenderBuffer();
        Invalidate();
    }

    // Live throughput, refreshed every second. Width is template-reserved, so only toggling the meter
    // on/off resizes the pill — the per-second value just repaints.
    public void SetNet(string net)
    {
        if (_net == net) return;
        bool toggled = (_net.Length == 0) != (net.Length == 0);
        _net = net;
        if (toggled) ResizeToContent();
        RenderBuffer();
        Invalidate();
    }

    void ResizeToContent()
    {
        using var fMain = MainFont();
        using var fCount = CountFont();
        string left = $"{_name}  {_time}".Trim();
        int wLeft = (int)Math.Ceiling(_measure.MeasureString(left, fMain).Width);
        int wCount = (int)Math.Ceiling(_measure.MeasureString(_count, fCount).Width);
        int wNet = _net.Length == 0 ? 0 : (int)Math.Ceiling(_measure.MeasureString(NetSpeed.Template, fMain).Width);
        // [pad][dot][gap] left [gap] · [gap] count [ [gap] · [gap] net ] [pad]
        int tail = S(8) + S(6) + S(8) + wCount + (wNet > 0 ? S(8) + S(6) + S(8) + wNet : 0);
        _w = S(12) + S(8) + S(8) + wLeft + tail + S(12);
        _w = Math.Max(S(110), _w);
    }

    Screen TargetScreen()
    {
        if (!string.IsNullOrEmpty(_deviceName))
            foreach (var s in Screen.AllScreens)
                if (string.Equals(s.DeviceName, _deviceName, StringComparison.OrdinalIgnoreCase))
                    return s;
        return Screen.PrimaryScreen!;
    }

    /// <summary>Position over the target monitor's taskbar; if that monitor has none, hug its bottom edge.</summary>
    public void SyncPosition() { if (_paused) return; PositionCore(raise: true); }

    // Reposition + region update for Settings live preview: bypasses the pause guard and never re-raises
    // (a raise would dismiss an open combo dropdown in the dialog).
    public void PreviewReposition() => PositionCore(raise: false);

    void PositionCore(bool raise)
    {
        var screen = TargetScreen();
        var tb = Interop.TaskbarForDevice(_deviceName);
        bool onItsBar = tb != IntPtr.Zero &&
                        string.Equals(Screen.FromHandle(tb).DeviceName, screen.DeviceName, StringComparison.OrdinalIgnoreCase);

        _scale = Interop.Scale(onItsBar ? tb : Handle);
        if (_scale <= 0) _scale = 1f;

        Rectangle strip;        // the bar/edge the pill sits on
        int rightEdge, leftEdge;
        if (onItsBar)
        {
            strip = Interop.WindowRect(tb);
            if (strip.IsEmpty) return;
            rightEdge = TrayRightAnchor(strip, tb);
            leftEdge = strip.Left;
        }
        else
        {
            var b = screen.Bounds;          // no taskbar on this monitor -> float at its bottom
            strip = new Rectangle(b.Left, b.Bottom - S(40), b.Width, S(40));
            rightEdge = b.Right;
            leftEdge = b.Left;
        }

        _h = Math.Min(S(32), strip.Height - S(4));
        int y = strip.Top + (strip.Height - _h) / 2;
        int x = AnchorRight ? rightEdge - _w - S(Offset) : leftEdge + S(Offset);

        var target = new Rectangle(x, y, _w, _h);
        if (target == _lastRect) { if (raise) Interop.RaiseTopmost(Handle); return; }
        _lastRect = target;
        Interop.MoveWindow(Handle, x, y, _w, _h, true);
        if (raise) Interop.RaiseTopmost(Handle);
        UpdateRegion();
    }

    int TrayRightAnchor(Rectangle tr, IntPtr tb)
    {
        int trayLeft = Interop.TrayNotifyLeft(tb);
        return trayLeft > 0 ? trayLeft : tr.Right;
    }

    void UpdateRegion()
    {
        int r = S(8);
        IntPtr rgn = Interop.CreateRoundRectRgn(0, 0, _w + 1, _h + 1, r * 2, r * 2);
        Interop.SetWindowRgn(Handle, rgn, true); // window takes ownership
    }

    void Invalidate() { if (Handle != IntPtr.Zero) Interop.InvalidateRect(Handle, IntPtr.Zero, false); }

    void RenderBuffer()
    {
        _buffer?.Dispose();
        _buffer = new Bitmap(Math.Max(1, _w), Math.Max(1, _h));
        using var g = Graphics.FromImage(_buffer);
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;
        g.Clear(_hover ? Theme.BgHover : Theme.Bg);

        int cy = _h / 2;
        int dotD = S(8);
        using var fMain = MainFont();
        using var fCount = CountFont();
        string left = $"{_name}  {_time}".Trim();

        if (Strings.IsRtl)
        {
            // Mirror: dot at the right, content flows right-to-left.
            using (var b = new SolidBrush(Theme.Accent))
                g.FillEllipse(b, _w - S(12) - dotD, cy - dotD / 2, dotD, dotD);
            var far = new StringFormat { LineAlignment = StringAlignment.Center, Alignment = StringAlignment.Far };
            float xr = _w - S(12) - dotD - S(8);
            using (var b = new SolidBrush(Theme.Text))
                g.DrawString(left, fMain, b, new RectangleF(0, 0, xr, _h), far);
            xr -= _measure.MeasureString(left, fMain).Width + S(8);
            using (var b = new SolidBrush(Theme.TextDim))
                g.DrawString("·", fMain, b, new RectangleF(0, 0, xr, _h), far);
            xr -= S(6) + S(8);
            using (var b = new SolidBrush(Theme.Good))
                g.DrawString(_count, fCount, b, new RectangleF(0, 0, xr, _h), far);
            if (_net.Length > 0)
            {
                xr -= _measure.MeasureString(_count, fCount).Width + S(8);
                using (var b = new SolidBrush(Theme.TextDim))
                    g.DrawString("·", fMain, b, new RectangleF(0, 0, xr, _h), far);
                xr -= S(6) + S(8);
                using (var b = new SolidBrush(Theme.TextDim))
                    g.DrawString(_net, fMain, b, new RectangleF(0, 0, xr, _h), far);
            }
            return;
        }

        using (var b = new SolidBrush(Theme.Accent))
            g.FillEllipse(b, S(12), cy - dotD / 2, dotD, dotD);
        var sf = new StringFormat { LineAlignment = StringAlignment.Center };
        float x = S(12) + dotD + S(8);
        using (var b = new SolidBrush(Theme.Text))
            g.DrawString(left, fMain, b, new RectangleF(x, 0, _w, _h), sf);
        x += _measure.MeasureString(left, fMain).Width + S(8);
        using (var b = new SolidBrush(Theme.TextDim))
            g.DrawString("·", fMain, b, new RectangleF(x, 0, S(6), _h), sf);
        x += S(6) + S(8);
        using (var b = new SolidBrush(Theme.Good))
            g.DrawString(_count, fCount, b, new RectangleF(x, 0, _w, _h), sf);
        if (_net.Length > 0)
        {
            x += _measure.MeasureString(_count, fCount).Width + S(8);
            using (var b = new SolidBrush(Theme.TextDim))
                g.DrawString("·", fMain, b, new RectangleF(x, 0, S(6), _h), sf);
            x += S(6) + S(8);
            using (var b = new SolidBrush(Theme.TextDim))
                g.DrawString(_net, fMain, b, new RectangleF(x, 0, _w, _h), sf);
        }
    }

    protected override void WndProc(ref Message m)
    {
        switch (m.Msg)
        {
            case Interop.WM_ERASEBKGND:
                m.Result = (IntPtr)1;
                return;
            case Interop.WM_MOUSEACTIVATE:
                m.Result = (IntPtr)Interop.MA_NOACTIVATE;
                return;
            case Interop.WM_PAINT:
                Paint();
                return;
            case Interop.WM_MOUSEMOVE:
                if (!_tracking) { TrackLeave(); _tracking = true; }
                if (!_hover) { _hover = true; RenderBuffer(); Invalidate(); }
                break;
            case Interop.WM_MOUSELEAVE:
                _tracking = false;
                if (_hover) { _hover = false; RenderBuffer(); Invalidate(); }
                break;
            case Interop.WM_LBUTTONUP:
                Clicked?.Invoke();
                break;
            case Interop.WM_RBUTTONUP:
                RightClicked?.Invoke(Cursor.Position);
                break;
        }
        base.WndProc(ref m);
    }

    void Paint()
    {
        var ps = new Interop.PAINTSTRUCT();
        IntPtr hdc = Interop.BeginPaint(Handle, ref ps);
        try
        {
            using var g = Graphics.FromHdc(hdc);
            if (_buffer != null) g.DrawImageUnscaled(_buffer, 0, 0);
        }
        finally { Interop.EndPaint(Handle, ref ps); }
    }

    void TrackLeave()
    {
        var tme = new Interop.TRACKMOUSEEVENT
        {
            cbSize = System.Runtime.InteropServices.Marshal.SizeOf<Interop.TRACKMOUSEEVENT>(),
            dwFlags = Interop.TME_LEAVE,
            hwndTrack = Handle,
        };
        Interop.TrackMouseEvent(ref tme);
    }

    public void Dispose()
    {
        if (_fgHook != IntPtr.Zero) Interop.UnhookWinEvent(_fgHook);
        if (_reorderHook != IntPtr.Zero) Interop.UnhookWinEvent(_reorderHook);
        _buffer?.Dispose();
        _measure.Dispose();
        _measureBmp.Dispose();
        if (Handle != IntPtr.Zero) DestroyHandle();
    }
}
