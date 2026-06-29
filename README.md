# PrayerTray

A lightweight prayer-times companion for **Windows 10 (1809+) and 11**. It puts a pill **on the
taskbar** showing the next prayer and a live countdown — click it for the full day. Reminders, azan
playback, themes, and a localized RTL-aware UI round it out.

Native WinForms + raw Win32, **zero dependencies**, times computed **fully offline**
(PrayTimes.org algorithm). Idle footprint ~10 MB RAM.

```
● Dhuhr  12:59 PM · 1:15
```

## Download

Grab the latest from [**Releases**](../../releases/latest) — two builds, no install wizard:

| Asset | Size | Needs .NET? | Use it if… |
|-------|------|-------------|------------|
| `PrayerTray-standalone-win-x64.exe` | ~77 MB | No | **You're not sure.** Self-contained, runs anywhere. Adds one-click location auto-detect. |
| `PrayerTray-needs-dotnet8-win-x64.exe` | ~3 MB | [.NET 8 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/8.0) | You already have .NET 8 and want a tiny download. |

Run the exe — it lands on the taskbar and (on first launch) helps you set your location.

## Features

- **Taskbar pill** — next prayer, time, and live countdown (per-second in the final minute). Dark,
  rounded, DPI-aware, per-monitor.
- **Click for the day** — popup with all of today's times, next prayer highlighted. Pin it to keep it
  open and drag it anywhere.
- **Reminders & azan** — optional balloon/toast + sound N minutes before each prayer; play a bundled
  adhan (Makkah / Madinah) or your own file at prayer time.
- **Rich notifications** — Action Center toasts where available, tray balloons as fallback.
- **Stays in sync** — recomputes on clock/timezone change or resume-from-sleep.
- **Hijri date & Islamic events** — Umm al-Qura date with a moon-sighting adjuster; special-day and
  next-event lines (Ramadan, the Eids, Arafah, Ashura, white days, …).
- **Sunnah & Friday reminders** — eve-before nudge for recommended fasts; Al-Kahf at Fajr and a
  Jumu'ah heads-up on Fridays.
- **Optional pill meters** — live internet speed (↓/↑) and/or ping latency (ms).
- **Themes & fonts** — Auto (follows Windows light/dark) or a fixed palette; any installed font, 80–150%.
- **Localized** — English, العربية, Français, Türkçe, اردو, Indonesia. Arabic and Urdu switch the whole
  UI to right-to-left; numerals stay Western.

## Settings

Right-click the pill (or tray icon) → **Settings** — a themed dialog (Location · Calculation ·
Appearance · Notifications) with live preview. Set city, lat/long, calculation method
(MWL / ISNA / Egypt / Makkah / Karachi), Asr juristic, high-latitude rule, per-prayer ± minute
fine-tuning, clock format, widget side/gap, monitor, and more. Saved to
`%APPDATA%\PrayerTray\config.json`; defaults to Makkah until changed.

**Set your location** — *Detect* (Windows Location service, falls back to IP geolocation) or *Pick on
map* (opens Google Maps; paste the coordinates or a share link). The network is touched only while
setting location — prayer times are always computed offline.

## Build from source

```powershell
dotnet build  -c Release
dotnet run    -c Release
dotnet publish -c Release -r win-x64 --self-contained -o .\publish   # standalone single-file exe
```

Requires the .NET 8 SDK. Pushing a `vX.Y.Z` tag builds and publishes both release assets via GitHub
Actions.

## Internals

How the pill rides the taskbar, the two build variants, localization, and a file-by-file map live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
