using System;
#if !MANUAL_ONLY
using System.IO;
using System.Runtime.InteropServices;
using System.Security;
using System.Text;
using Windows.Data.Xml.Dom;
using Windows.UI.Notifications;
using PrayerTray.Native;
#endif

namespace PrayerTray.Services;

/// <summary>Action Center toast notifications. Unpackaged apps need a Start-Menu shortcut tagged with the
/// process AUMID before Windows will display a toast; Init() sets that up. Callers fall back to a NotifyIcon
/// balloon when Show returns false (older Windows, ManualOnly build, or any failure).</summary>
public static class ToastService
{
    public const string Aumid = "DynamicEG.PrayerTray";

#if MANUAL_ONLY
    public static bool Available => false;
    public static bool Init() => false;
    public static bool Show(string title, string body) => false;
#else
    static bool _ready;
    public static bool Available => _ready;

    public static bool Init()
    {
        try
        {
            Interop.SetCurrentProcessExplicitAppUserModelID(Aumid);
            EnsureShortcut();
            _ready = true;
        }
        catch { _ready = false; }
        return _ready;
    }

    public static bool Show(string title, string body)
    {
        if (!_ready) return false;
        try
        {
            // Silent: the app plays its own reminder/azan audio, so suppress the default toast sound.
            string x = "<toast><visual><binding template='ToastGeneric'>" +
                       $"<text>{Esc(title)}</text><text>{Esc(body)}</text>" +
                       "</binding></visual><audio silent='true'/></toast>";
            var doc = new XmlDocument();
            doc.LoadXml(x);
            ToastNotificationManager.CreateToastNotifier(Aumid).Show(new ToastNotification(doc));
            return true;
        }
        catch { return false; }
    }

    static string Esc(string s) => SecurityElement.Escape(s) ?? s;

    static string ShortcutPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "Microsoft", "Windows", "Start Menu", "Programs", "Prayer Tray.lnk");

    static void EnsureShortcut()
    {
        string exe = Environment.ProcessPath ?? "";
        if (exe.Length == 0) return;
        if (File.Exists(ShortcutPath) && string.Equals(ShortcutTarget(), exe, StringComparison.OrdinalIgnoreCase))
            return;

        var link = (IShellLinkW)new CShellLink();
        link.SetPath(exe);
        link.SetArguments("");
        var dir = Path.GetDirectoryName(exe);
        if (dir != null) link.SetWorkingDirectory(dir);

        var store = (IPropertyStore)link;
        var key = PKEY_AppUserModel_ID;
        var pv = new PropVariant { vt = VT_LPWSTR, p = Marshal.StringToCoTaskMemUni(Aumid) };
        store.SetValue(ref key, ref pv);
        store.Commit();
        Marshal.FreeCoTaskMem(pv.p);

        Directory.CreateDirectory(Path.GetDirectoryName(ShortcutPath)!);
        ((IPersistFile)link).Save(ShortcutPath, true);
    }

    static string ShortcutTarget()
    {
        try
        {
            var link = (IShellLinkW)new CShellLink();
            ((IPersistFile)link).Load(ShortcutPath, 0);
            var sb = new StringBuilder(260);
            link.GetPath(sb, sb.Capacity, IntPtr.Zero, 0);
            return sb.ToString();
        }
        catch { return ""; }
    }

    const ushort VT_LPWSTR = 31;
    static PropertyKey PKEY_AppUserModel_ID = new()
    {
        fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),
        pid = 5,
    };

    [StructLayout(LayoutKind.Sequential)]
    struct PropertyKey { public Guid fmtid; public uint pid; }

    [StructLayout(LayoutKind.Explicit)]
    struct PropVariant { [FieldOffset(0)] public ushort vt; [FieldOffset(8)] public IntPtr p; }

    [ComImport, Guid("00021401-0000-0000-C000-000000000046")]
    class CShellLink { }

    [ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown), Guid("000214F9-0000-0000-C000-000000000046")]
    interface IShellLinkW
    {
        void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszFile, int cch, IntPtr pfd, int fFlags);
        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);
        void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszName, int cch);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszDir, int cch);
        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);
        void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszArgs, int cch);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);
        void GetHotkey(out short pwHotkey);
        void SetHotkey(short wHotkey);
        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);
        void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszIconPath, int cch, out int piIcon);
        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);
        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, int dwReserved);
        void Resolve(IntPtr hwnd, int fFlags);
        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown), Guid("0000010b-0000-0000-C000-000000000046")]
    interface IPersistFile
    {
        void GetClassID(out Guid pClassID);
        [PreserveSig] int IsDirty();
        void Load([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, int dwMode);
        void Save([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, [MarshalAs(UnmanagedType.Bool)] bool fRemember);
        void SaveCompleted([MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
        void GetCurFile([MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
    }

    [ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown), Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99")]
    interface IPropertyStore
    {
        void GetCount(out uint cProps);
        void GetAt(uint iProp, out PropertyKey pkey);
        void GetValue(ref PropertyKey key, out PropVariant pv);
        void SetValue(ref PropertyKey key, ref PropVariant pv);
        void Commit();
    }
#endif
}
