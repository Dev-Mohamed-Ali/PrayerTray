# PrayerTray

A lightweight Windows **10 (1809+) & 11** prayer-times companion. At its core, a pill **on the
taskbar** showing the next prayer + live countdown; click for the full day. Around it: pre-prayer
**reminders**, **azan** playback, themes and fonts, and a fully **localized** UI (English / العربية
with right-to-left layout). Native WinForms + raw Win32, **zero NuGet dependencies**, times computed
**offline** (PrayTimes.org algorithm). ~10 MB private RAM (vs Awqat-Salaat's WinUI 3, ~60–150 MB).

> Built and tested on Windows 11. Windows 10 (1809+) is supported by the same overlay approach
> (`Shell_TrayWnd`/`TrayNotifyWnd` exist there; the Win11-only DWM rounded-corner hint on the popup
> just no-ops) but is **not yet hardware-tested** — please report issues. A vertical taskbar is not
> handled on either OS.

## Use

- **Taskbar pill** (right side by default, just left of the clock): `● Dhuhr 12:59 PM · 1:15` —
  next prayer, time, and live countdown (switches to a per-second `45s` countdown in the final minute).
  Dark, rounded, DPI-aware.
- **Left-click** → popup with today's full list (next prayer highlighted + "X in h:mm" chip).
  Click away to dismiss, or **pin** it (thumbtack) to keep it open and drag it anywhere.
- **Right-click the pill** (or the tray icon) → Show times · Refresh now · Start with Windows ·
  Settings · Stop sound · Exit.
- A tray icon is kept as a safety net + notification host; a balloon fires for each prayer (and for
  the pre-prayer reminder, if enabled).
- **Reminders & azan** — optionally get a balloon + sound N minutes before each prayer, and play an
  azan at prayer time (two bundled adhans — Makkah / Madinah — or your own audio file).
- **Stays in sync** — recomputes automatically on a system clock/timezone change or resume-from-sleep;
  "Refresh now" forces it on demand.

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

Right-click → **Settings** (a themed 2-column card grid; visual changes preview live, Save persists,
Cancel reverts): city label, latitude/longitude, method (MWL / ISNA / Egypt / Makkah / Karachi),
Asr juristic (Standard / Hanafi), 12/24h clock, widget side (Left/Right) + gap.
Saved to `%APPDATA%\PrayerTray\config.json`. Defaults to Makkah until changed.

**Language:** `System default` (follows the Windows display language), `English`, or `العربية`.
Arabic switches the whole UI — pill, popup, menus, and the Settings dialog — to **right-to-left**.
Numerals stay Western (e.g. `12:30`).

**Reminders & azan:** enable a reminder 1–60 minutes before each prayer with an optional sound
(a small synthesized set, or a custom `.wav`/`.mp3`); choose an azan at prayer time — `Off`, a bundled
adhan (Makkah / Madinah), or a custom file. **Test**/**Stop** buttons preview each sound.

**Appearance & placement:**

- **Theme** — `Auto` follows the Windows app light/dark setting live (no restart); or pick a fixed
  palette: `Dark`, `Light`, `Midnight`, `Slate`, `Warm`. Recolors both the pill and the popup.
- **Font & size** — pick any installed font family and a scale (80–150%); applies to the pill and popup.
- **Monitor** — put the pill on any display (listed by real monitor name). It rides that monitor's
  taskbar; if that monitor has no taskbar (e.g. *Show my taskbar on all displays* is off), the pill
  floats at the bottom of that screen instead.
- **Hide over fullscreen apps** (on by default) — the pill hides while a game or fullscreen video
  owns its monitor, and reappears when you exit fullscreen.

**Set location — two ways** (both auto-run/appear on first launch):

- **Detect** (one click): tries the **Windows location service** first (accurate; needs *Settings ▸
  Privacy ▸ Location* on), falls back to **approximate IP-geolocation**, then fills lat/lng/city and
  the country-appropriate method for you to **confirm or edit before saving**.
- **Pick on map**: **Open Maps** launches Google Maps in your browser (pre-centered on your rough IP
  area). Right-click your exact spot, click the lat/lng to copy them, then **paste** into the box and
  hit **Set** — most accurate, since you pin the point yourself. The box also accepts a pasted map
  URL or a `maps.app.goo.gl` share link.

Either way, the network is touched only while setting location — prayer times are always computed
offline.

Timezone auto-detected from Windows (handles DST). High-latitude summer (e.g. London) uses the
angle-based night-portion fallback for Fajr/Isha.

## Build / run

```powershell
dotnet build -c Release
dotnet run   -c Release
dotnet publish -c Release -r win-x64 --no-self-contained -o .\publish   # single-file exe
```

Requires the .NET 8 Desktop Runtime (installed). Use `--self-contained` for a no-runtime-needed exe.

## Releases

Pushing a `vX.Y.Z` tag triggers GitHub Actions, which builds and publishes two assets — no local upload:

- **`PrayerTray-autodetect-win-x64.exe`** — self-contained, compressed single file; runs on Win10
  1809+/Win11 with nothing installed (includes one-click location auto-detect).
- **`PrayerTray-manual-win-x64.exe`** — tiny framework-dependent build; needs the .NET 8 Desktop
  Runtime, location set manually.

## Files

Source is grouped by concern (single `PrayerTray` assembly, nested namespaces):

| File | Role |
|------|------|
| `Calc/PrayerTimes.cs` | Offline astronomical calculation (validated, incl. high-latitude fix) |
| `Config/AppConfig.cs` | JSON config load/save |
| `I18n/Strings.cs` | Embedded EN/AR string catalog + RTL/date helpers (no satellite assemblies) |
| `Services/AudioPlayer.cs` | Azan/reminder playback via winmm MCI + synthesized reminder tones |
| `Services/LocationService.cs` | One-shot location detect (Windows Location → IP) + map-link parsing |
| `Native/Interop.cs` | Win32/DWM P/Invoke: taskbar, geometry, DPI, fullscreen detect, light/dark title bar |
| `Native/Displays.cs` | Monitor enumeration + friendly names (CCD DisplayConfig API) |
| `UI/Theme.cs` | Palettes (Dark/Light/Midnight/Slate/Warm) + Auto-follow + taskbar geometry |
| `UI/IconRenderer.cs` | Renders the tray glyph |
| `UI/TaskbarWidget.cs` | The overlay pill — raw Win32 window, DPI-aware, per-monitor, instant re-raise |
| `UI/PrayerPopup.cs` | Today's-times popup (custom-painted) |
| `UI/SettingsForm.cs` | Settings dialog |
| `App/Program.cs` | Entry point (single-instance) |
| `App/AppHost.cs` | Orchestrator: widget + tray + popup + timers + notifications |
| `App/TaskbarWatcher.cs` | Rebuilds the pill on Explorer restart; signals Windows theme changes |
