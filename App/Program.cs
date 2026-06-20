using System;
using System.IO;
using System.Threading;
using System.Windows.Forms;

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
            MessageBox.Show("Prayer Tray hit an unexpected error and recovered.\n" +
                "Details were written to %APPDATA%\\PrayerTray\\error.log.",
                "Prayer Tray", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }
        catch { /* no UI available */ }
    }
}
