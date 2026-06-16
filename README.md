# PrayerTray

Lightweight Windows 11 prayer-times app. A pill **on the taskbar** showing the next prayer + live
countdown; click for the full day. Native WinForms + raw Win32, **zero NuGet dependencies**, times
computed **offline** (PrayTimes.org algorithm). ~10 MB private RAM (vs Awqat-Salaat's WinUI 3, ~60–150 MB).

## Use

- **Taskbar pill** (right side by default, just left of the clock): `● Dhuhr 12:59 PM · 1:15` —
  next prayer, time, and live countdown. Dark, rounded, DPI-aware.
- **Left-click** → popup with today's full list (next prayer highlighted + "X in h:mm" chip).
  Click away to dismiss.
- **Right-click the pill** (or the tray icon) → Show times · Start with Windows · Settings · Exit.
- A tray icon is kept as a safety net + notification host; a balloon fires when each prayer starts.

### How it sits on the taskbar — and why (Windows 11)

There is **no supported way to embed a visible widget *into* the Win11 taskbar**, and this was proven,
not assumed:
- **Deskbands** (the old toolbar API) were removed in Windows 11.
- **Reparenting** a window into `Shell_TrayWnd` (`SetParent`) *does* embed it — but it stays invisible.
  Enumerating the taskbar's children shows a `Windows.UI.Composition.DesktopWindowContentBridge`
  surface spanning the whole bar; it's composited over GDI child windows regardless of z-order, so an
  embedded pill is buried under it.

So the only option — the same one Awqat-Salaat and every other Win11 taskbar widget use — is a
**top-level always-on-top overlay** drawn over the taskbar. Its one weakness (getting covered when you
click the taskbar) is fixed properly here: a **`SetWinEventHook`** re-raises the pill the instant the
taskbar comes forward (~1 frame), so there's no visible flicker — no slow polling. If your taskbar is
left-aligned or its right half is packed, the pill can overlap icons (inherent to overlays); switch
side/offset in **Settings**.

## Settings

Right-click → **Settings**: city label, latitude/longitude, method (MWL / ISNA / Egypt / Makkah /
Karachi), Asr juristic (Standard / Hanafi), 12/24h clock, widget side (Left/Right) + gap.
Saved to `%APPDATA%\PrayerTray\config.json`. Defaults to Makkah until changed.

Timezone auto-detected from Windows (handles DST). High-latitude summer (e.g. London) uses the
angle-based night-portion fallback for Fajr/Isha.

## Build / run

```powershell
dotnet build -c Release
dotnet run   -c Release
dotnet publish -c Release -r win-x64 --no-self-contained -o .\publish   # single-file exe
```

Requires the .NET 8 Desktop Runtime (installed). Use `--self-contained` for a no-runtime-needed exe.

## Files

| File | Role |
|------|------|
| `PrayerTimes.cs` | Offline astronomical calculation (validated, incl. high-latitude fix) |
| `AppConfig.cs` | JSON config load/save |
| `AppHost.cs` | Orchestrator: widget + tray + popup + timers + notifications |
| `TaskbarWidget.cs` | The overlay pill — raw Win32 window, DPI-aware, instant re-raise hook |
| `PrayerPopup.cs` | Today's-times popup (custom-painted) |
| `SettingsForm.cs` | Settings dialog |
| `TaskbarWatcher.cs` | Rebuilds the pill when Explorer restarts |
| `Theme.cs` | Colors + taskbar geometry |
| `Interop.cs` | All Win32/DWM P/Invoke in one place |
| `IconRenderer.cs` | Renders the tray glyph |
| `Program.cs` | Entry point (single-instance) |
