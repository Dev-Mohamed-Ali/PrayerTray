using System;
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

        ApplicationConfiguration.Initialize();
        Application.Run(new AppHost());
    }
}
