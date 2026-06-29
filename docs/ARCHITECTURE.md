# PrayerTray — architecture & internals

Design notes for contributors. For user-facing docs see the [README](../README.md).

## Shape

Single `PrayerTray` assembly, nested namespaces, **zero NuGet dependencies** — only the .NET 8
Desktop framework and raw Win32/DWM/COM P/Invoke. Prayer times are computed **offline** with the
PrayTimes.org algorithm. Idle footprint ~10 MB private RAM.

Two build variants from one csproj:

| Variant | TFM | WinRT | Built with |
|---------|-----|-------|------------|
| Default | `net8.0-windows10.0.19041.0` | yes (Action Center toasts) | `dotnet build -c Release` |
| ManualOnly | `net8.0-windows` | no | `dotnet build -c Release -p:ManualOnly=true` |

WinRT-only code is gated behind `#if !MANUAL_ONLY`; the ManualOnly build falls back to tray balloons.

`InvariantGlobalization=true` keeps the binary small (no ICU) and guarantees Western digits.
`PredefinedCulturesOnly=false` is set alongside it so WinForms' `WM_INPUTLANGCHANGE` handling
(`CultureInfo.GetCultureInfo(langid)`) returns an invariant-backed culture instead of throwing on a
keyboard-layout switch — staying in invariant mode, no behavior change beyond removing that crash.

## How it sits on the taskbar — and why (Windows 11)

There is **no supported way to embed a visible widget *into* the Win11 taskbar**, and this was proven,
not assumed:

- **Deskbands** (the old toolbar API) were removed in Windows 11.
- **Reparenting** a window into `Shell_TrayWnd` (`SetParent`) *does* embed it — but it stays invisible.
  Enumerating the taskbar's children shows a `Windows.UI.Composition.DesktopWindowContentBridge`
  surface spanning the whole bar; it's composited over GDI child windows regardless of z-order, so an
  embedded pill is buried under it.

So the only option — the same one every other Win11 taskbar widget uses — is a **top-level
always-on-top overlay** drawn over the taskbar. Its one weakness (getting covered when you click the
taskbar) is fixed with a **`SetWinEventHook`** that re-raises the pill the instant the taskbar comes
forward (~1 frame), so there's no visible flicker and no slow polling.

Windows 10 (1809+) is supported by the same overlay approach (`Shell_TrayWnd`/`TrayNotifyWnd` exist;
the Win11-only DWM rounded-corner hint on the popup just no-ops) but is not yet hardware-tested.
A vertical taskbar is not handled on either OS.

## Localization

UI strings live in an embedded dict-of-dicts keyed by a `Language` enum (`I18n/Strings.cs`) — no
`.resx`, no satellite assemblies, single-file safe. Missing keys fall back to English. The OS UI
language is detected via `GetUserDefaultUILanguage() & 0x3FF` (CultureInfo can't tell us under
invariant mode). Arabic and Urdu flip the whole UI to right-to-left; numerals stay Western.

## Releases

Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`, which builds and publishes both
assets via `softprops/action-gh-release`. No version string lives in the csproj — the tag drives it.

## Files

| File | Role |
|------|------|
| `Calc/PrayerTimes.cs` | Offline astronomical calculation (incl. high-latitude rules) |
| `Calc/IslamicEvents.cs` | Islamic special days + next-major-event lookup from the Hijri date |
| `Config/AppConfig.cs` | JSON config load/save |
| `I18n/Strings.cs` | Embedded multi-language string catalog + RTL/date helpers (no satellites) |
| `Services/AudioPlayer.cs` | Azan/reminder playback via winmm MCI + synthesized reminder tones |
| `Services/LocationService.cs` | One-shot location detect (Windows Location → IP) + map-link parsing |
| `Services/NetSpeed.cs` | Live ↓/↑ throughput from NIC byte counters (optional pill meter) |
| `Services/Latency.cs` | Async ICMP ping latency (optional pill `ms` meter) |
| `Services/ToastService.cs` | Action Center toasts via WinRT + auto AUMID Start-Menu shortcut |
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
