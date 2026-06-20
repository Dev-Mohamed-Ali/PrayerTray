using System;
using System.IO;
using System.Threading;
using System.Windows.Forms;
using PrayerTray.I18n;

namespace PrayerTray;

static class Program
{
    [STAThread]
    static void Main()
    {
        using var mutex = new Mutex(true, "PrayerTray.SingleInstance", out bool isNew);
        if (!isNew) return;

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
