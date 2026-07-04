using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using PrayerTray.Native;

namespace PrayerTray;

/// <summary>
/// Hidden top-level window that listens for the "TaskbarCreated" broadcast (Explorer restart),
/// so the embedded widget can be rebuilt. Child windows don't receive broadcasts, hence this proxy.
/// </summary>
public sealed class TaskbarWatcher : NativeWindow, IDisposable
{
    readonly int _taskbarCreated;
    public event Action? Recreated;
    public event Action? ThemeChanged; // Windows app light/dark flip (WM_SETTINGCHANGE / ImmersiveColorSet)

    public TaskbarWatcher()
    {
        _taskbarCreated = (int)Interop.RegisterWindowMessage("TaskbarCreated");
        CreateHandle(new CreateParams { X = -2000, Y = -2000, Width = 1, Height = 1 });
    }

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == _taskbarCreated) Recreated?.Invoke();
        else if (m.Msg == Interop.WM_SETTINGCHANGE && IsColorChange(m.LParam)) ThemeChanged?.Invoke();
        base.WndProc(ref m);
    }

    static bool IsColorChange(IntPtr lParam) =>
        lParam != IntPtr.Zero && Marshal.PtrToStringAuto(lParam) == "ImmersiveColorSet";

    public void Dispose()
    {
        if (Handle != IntPtr.Zero) DestroyHandle();
    }
}
