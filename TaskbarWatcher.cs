using System;
using System.Windows.Forms;

namespace PrayerTray;

/// <summary>
/// Hidden top-level window that listens for the "TaskbarCreated" broadcast (Explorer restart),
/// so the embedded widget can be rebuilt. Child windows don't receive broadcasts, hence this proxy.
/// </summary>
public sealed class TaskbarWatcher : NativeWindow, IDisposable
{
    readonly int _taskbarCreated;
    public event Action? Recreated;

    public TaskbarWatcher()
    {
        _taskbarCreated = (int)Interop.RegisterWindowMessage("TaskbarCreated");
        CreateHandle(new CreateParams { X = -2000, Y = -2000, Width = 1, Height = 1 });
    }

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == _taskbarCreated) Recreated?.Invoke();
        base.WndProc(ref m);
    }

    public void Dispose()
    {
        if (Handle != IntPtr.Zero) DestroyHandle();
    }
}
