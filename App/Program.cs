using System;
using System.IO;
using System.Threading;
using System.Windows.Forms;
using PrayerTray.I18n;

namespace PrayerTray;

static class Program
{
    static Mutex? _mutex;

    [STAThread]
    static void Main()
    {
        _mutex = new Mutex(true, "PrayerTray.SingleInstance", out bool isNew);
        if (!isNew) return;

        CleanupOldUpdate();

        // Survive a stray exception instead of dying silently; log the stack so it can be diagnosed.
        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
        Application.ThreadException += (_, e) => Report(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => Report(e.ExceptionObject as Exception);

        // WinForms throws in its WM_INPUTLANGCHANGE handler under InvariantGlobalization (InputLanguage
        // .LanguageTag builds a CultureInfo). Swallow the post-change notice; the layout still switches.
        Application.AddMessageFilter(new InputLangCrashGuard());

        ApplicationConfiguration.Initialize();
        Application.Run(new AppHost());
    }

    // Called on the UI thread right before spawning the updated exe (mutex is thread-affine).
    internal static void ReleaseSingleInstance()
    {
        try { _mutex?.ReleaseMutex(); _mutex?.Dispose(); _mutex = null; }
        catch { /* already released/disposed */ }
    }

    // The previous exe left behind by a self-update; may still be locked for a moment — next launch retries.
    static void CleanupOldUpdate()
    {
        try
        {
            string? p = Environment.ProcessPath;
            if (p != null && File.Exists(p + ".old")) File.Delete(p + ".old");
        }
        catch { /* best effort */ }
    }

    static void Report(Exception? ex)
    {
        if (ex == null) return;
        try
        {
            string dir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "PrayerTray");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "error.log"),
                $"{DateTime.Now:yyyy-MM-dd HH:mm:ss} {ex}{Environment.NewLine}{Environment.NewLine}");
        }
        catch { /* logging must never throw */ }

        try
        {
            // May fire before AppHost initializes Strings — defaults to English, which is fine here.
            MessageBox.Show(Strings.T("crash.body"), Strings.T("app.name"),
                MessageBoxButtons.OK, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button1, Strings.MsgOpts);
        }
        catch { /* no UI available */ }
    }

    // Drops WM_INPUTLANGCHANGE (0x0051) before WinForms' crashing handler sees it.
    sealed class InputLangCrashGuard : IMessageFilter
    {
        public bool PreFilterMessage(ref Message m) => m.Msg == 0x0051;
    }
}
