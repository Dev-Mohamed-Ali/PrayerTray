# PrayerTray

A lightweight prayer-times companion for **Windows 10 (1809+) and 11**. It puts a pill **on the
taskbar** showing the next prayer and a live countdown — click it for the full day. Reminders, azan
playback, themes, and a localized RTL-aware UI round it out.

Native Win32 written in Rust — **one ~3.5 MB exe, zero dependencies, no runtime to install** — with
times computed **fully offline** (PrayTimes.org algorithm). Idle footprint ~30 MB RAM.

```
● Dhuhr  12:59 PM · 1:15 · ↓ 1.2 MB/s · ↑ 88 KB/s · 24 ms · Σ 3.4 GB
```

## Download

Grab **`PrayerTray-win-x64.exe`** from [**Releases**](../../releases/latest) — no install wizard,
no .NET, nothing else to download. Run it; it lands on the taskbar and (on first launch) helps you
set your location. Settings from a v1.x install are picked up automatically (same config file).

> **First run:** Windows SmartScreen may say it's an *unrecognized app* — that's the unsigned-app notice, not malware. Click **More info → Run anyway**.

## Features

- **Taskbar pill** — next prayer, time, and live countdown (per-second in the final minute). Dark,
  rounded, DPI-aware, per-monitor.
- **Click for the day** — popup with all of today's times, next prayer highlighted. Pin it to keep it
  open and drag it anywhere.
- **Reminders & azan** — optional toast + sound N minutes before each prayer; play a bundled adhan
  (Makkah / Madinah) or your own file at prayer time.
- **Quiet when you're busy** — no adhan blasting through a Teams call, a full-screen game, a
  presentation, or Do not disturb. The notification still arrives; the sound doesn't. Once you're
  free again you get a single silent "the azan was muted" line. On by default, one checkbox to
  turn off.
- **Network meters** *(optional)* — live download/upload speed, latency (TCP :443 or ICMP), and a
  running per-day data total appended to the pill as a compact, fixed-width tail. A **Data usage**
  window shows per-day history (kept 90 days). Meter one chosen adapter or all of them.
- **Rich notifications** — Action Center toasts, tray balloons as fallback.
- **Stays in sync** — recomputes on clock/timezone change or resume-from-sleep.
- **Hijri date & Islamic events** — Umm al-Qura date with a moon-sighting adjuster; special-day and
  next-event lines (Ramadan, the Eids, Arafah, Ashura, white days, …).
- **Sunnah & Friday reminders** — eve-before nudge for recommended fasts; Al-Kahf at Fajr and a
  Jumu'ah heads-up on Fridays.
- **Themes & fonts** — Auto (follows Windows light/dark) or a fixed palette; any installed font, 80–150%.
- **Check for updates** — a tray-menu item that compares against the latest GitHub release and
  installs in place. Manual only — the network is never touched unless you click it.
- **Localized** — English, العربية, Français, Türkçe, اردو, Indonesia. Arabic and Urdu switch the whole
  UI to right-to-left; numerals stay Western.

## Settings

Right-click the pill (or tray icon) → **Settings** — a themed dialog (Location · Calculation ·
Appearance · Network · Religious · Notifications) with live preview. Set city, lat/long, calculation method
(MWL / ISNA / Egypt / Makkah / Karachi), Asr juristic, high-latitude rule, per-prayer ± minute
fine-tuning, clock format, widget side/gap, monitor, and more. Saved to
`%APPDATA%\PrayerTray\config.json` (same file across v1 and v2); defaults to Makkah until changed.

**Set your location** — *Detect* (Windows Location service, falls back to IP geolocation) or *Pick on
map* (opens Google Maps; paste the coordinates or a share link). The network is touched only while
setting location — prayer times are always computed offline.

## Build from source

```powershell
cd rust
cargo build --release      # needs MSVC Build Tools + Windows SDK
cargo test                 # calc fixtures (exact-match vs the C# engine), config, i18n
```

Pushing a `vX.Y.Z` tag builds, tests, and drafts a GitHub release via Actions. See
[`rust/README.md`](rust/README.md) for dev-mode notes and data-regeneration tools.

The original C# (.NET 8 / WinForms) implementation was removed after the Rust rewrite; it remains in
git history at tag [`v1.14.0`](../../releases/tag/v1.14.0).

## Internals

How the pill rides the taskbar, exactness guarantees of the port, threading, and a file-by-file map
live in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
