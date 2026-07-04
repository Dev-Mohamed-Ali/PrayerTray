# PrayerTray — architecture & internals

Design notes for contributors. For user-facing docs see the [README](../README.md).

## Shape

Single Rust crate in `rust/` targeting `x86_64-pc-windows-msvc`. Dependencies: the `windows`
crate (feature-gated Win32/WinRT bindings) plus `serde`/`serde_json` for the config contract —
nothing else. Rendering is the **GDI+ flat C API** behind RAII wrappers (`ui/gdip.rs`), the same
renderer the original C# build used via System.Drawing, so text metrics and Arabic/Urdu shaping
match. Release profile: `opt-level="z"`, fat LTO, `panic="abort"`, stripped → ~3.5 MB exe
(~0.4 MB code + two embedded azan mp3s). Idle footprint ~30 MB RAM.

Prayer times are computed **offline** with the PrayTimes.org algorithm; the network is touched only
by explicit user actions (location detect, update check).

The original C# implementation lives in `legacy-dotnet/` as the porting reference. It still builds
(`dotnet build legacy-dotnet/PrayerTray.csproj`) but is no longer released.

## Exactness: ported, not re-derived

The port had to produce the *same minutes* users already see, so nothing numeric was re-implemented
from a paper spec:

- `rust/tools/genfix/` compiles the **actual C# `PrayerTimes.cs`** and dumps 1,920 fixture cases
  (8 cities × 5 methods × Asr rules × high-latitude rules, incl. polar summer) plus 406 Hijri cases;
  `cargo test` exact-matches them.
- The Umm al-Qura month table (`src/calc/umalqura_data.rs`) is dumped from .NET's
  `UmAlQuraCalendar` (1318–1500 AH), not computed — a generic tabular algorithm would shift dates.
- C# `Math.Round` is banker's rounding → the Rust side uses `round_ties_even()`.
- `src/i18n/data.rs` is generated from `legacy-dotnet/I18n/Strings.cs` by
  `tools/convert_strings.py`. Never hand-edit generated files; rerun the tools.

Config compatibility is a hard contract: `%APPDATA%\PrayerTray\config.json`, PascalCase via serde,
sentinels preserved (`i32::MIN` popup position, `999.0` = system timezone). Fields for features not
yet ported (net meters, data usage, NIC picker) stay in the struct so a v1 config round-trips
losslessly.

## How it sits on the taskbar — and why (Windows 11)

There is **no supported way to embed a visible widget *into* the Win11 taskbar**, and this was proven,
not assumed:

- **Deskbands** (the old toolbar API) were removed in Windows 11.
- **Reparenting** a window into `Shell_TrayWnd` (`SetParent`) *does* embed it — but it stays invisible.
  Enumerating the taskbar's children shows a `Windows.UI.Composition.DesktopWindowContentBridge`
  surface spanning the whole bar; it's composited over GDI child windows regardless of z-order, so an
  embedded pill is buried under it.

So the only option — the same one every other Win11 taskbar widget uses — is a **top-level
always-on-top overlay** (`WS_POPUP`, no-activate, **owned by `Shell_TrayWnd`** so it rides the
taskbar's z-band). Its one weakness (getting covered when you click the taskbar) is fixed with
**`SetWinEventHook`** (`EVENT_SYSTEM_FOREGROUND` global + `EVENT_OBJECT_REORDER` scoped to the
taskbar thread, 16 ms debounce) that re-raises the pill the instant the taskbar comes forward — no
flicker, no polling.

Corollary: popup menus clip to the *monitor*, not the work area, and the taskbar band would hide
any rows underneath it — so `show_menu` passes `TPMPARAMS.rcExclude` = the taskbar strip with
`TPM_VERTICAL`.

Windows 10 (1809+) is supported by the same overlay approach; a vertical taskbar is not handled.

## Threading & window plumbing

One UI thread owns all windows, timers (1 s render / 15 s recompute), hooks, and GDI+. Network work
(update check/download, IP geolocation) runs on `std::thread::spawn` and posts results back with
`PostMessageW`, passing `Box::into_raw` payloads in `lparam`. The wndproc trampoline
(`ui/window.rs`) stores the handler pointer from `WM_NCCREATE`'s `lpCreateParams` in
`GWLP_USERDATA` and routes to a `WindowHandler` trait.

A hidden **top-level** (not message-only) window receives the broadcasts that drive resilience:
`WM_POWERBROADCAST` (resume), `WM_TIMECHANGE`, `WM_SETTINGCHANGE` (theme follow), and
`TaskbarCreated` (Explorer restart → re-add tray icon, rebuild pill).

The settings dialog is native Win32 children (combo/edit/checkbox, owner-drawn buttons) themed via
`WM_CTLCOLOR*` + `SetWindowTheme("DarkMode_CFD")`, run in a nested modal loop; full RTL via
`WS_EX_LAYOUTRTL`.

## Localization

UI strings live in a generated static table keyed by language (`src/i18n/data.rs`) — binary-searched
`t(key)`, `{0}` substitution, English fallback. OS language detected via
`GetUserDefaultUILanguage() & 0x3FF`. Arabic and Urdu flip the whole UI to right-to-left; numerals
stay Western.

## Toasts without a package identity

`SetCurrentProcessExplicitAppUserModelID("DynamicEG.PrayerTray")` plus a Start-Menu shortcut
carrying `PKEY_AppUserModel_ID` (created via `IShellLinkW`/`IPropertyStore`) — the same recipe and
the same AUMID as v1.x, so upgraders don't get duplicate shortcuts. Tray balloons are the fallback.

## Releases & migration from v1.x

Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`: the tag version is patched into
`Cargo.toml` + `prayertray.rc`, tests run, and a **draft** release is created for manual inspection
before publishing. The exe is uploaded as `PrayerTray-win-x64.exe` **plus** the two legacy v1.x
asset names (`-standalone`, `-needs-dotnet8`) — the v1.x in-app updater looks for those exact names
and swaps the exe in place, migrating users onto the native build automatically. Config path, AUMID,
mutex, and Run-key are unchanged, so nothing else needs migrating. Dev builds are version `0.0.0`
and never self-update; set `PRAYERTRAY_DEV_MUTEX=1` to run one beside an installed release.

## Files (`rust/src/`)

| File | Role |
|------|------|
| `calc/praytimes.rs` | Offline astronomical calculation (incl. high-latitude rules) |
| `calc/hijri.rs` + `calc/umalqura_data.rs` | Umm al-Qura Hijri conversion (generated .NET table) |
| `calc/events.rs` | Islamic special days + next-major-event lookup |
| `config.rs` | serde config load/save/sanitize, v1-compatible schema |
| `datetime.rs` | Civil-date ↔ Rata Die helpers (no chrono) |
| `i18n/` | Language runtime + generated string catalog |
| `native/taskbar.rs` | Taskbar find/geometry, fullscreen detect, DPI |
| `native/displays.rs` | Monitor enumeration + CCD friendly names |
| `native/startup.rs` | HKCU Run key + StartupApproved handling |
| `native/time.rs` | Local time + DST-aware UTC offset |
| `ui/gdip.rs` | RAII GDI+ wrappers — the only unsafe-heavy drawing zone |
| `ui/window.rs` | Window-class/wndproc trampoline plumbing |
| `ui/widget.rs` | The overlay pill — owned by the taskbar, hooks, DPI, RTL |
| `ui/popup.rs` | Today's-times popup (pin, drag, saved position) |
| `ui/settings.rs` + `ui/controls.rs` | Settings dialog + themed native controls |
| `ui/icon.rs` | Tray icon, tooltip, balloon fallback |
| `ui/theme.rs` | Palettes (Dark/Light/Midnight/Slate/Warm) + Auto-follow |
| `services/audio.rs` | Azan/reminder playback via MCI + synthesized tones |
| `services/toast.rs` | Action Center toasts + AUMID shortcut |
| `services/location.rs` | WinRT geolocation → IP fallback + map-link parsing |
| `services/update.rs` | GitHub release check + in-place exe swap |
| `services/http.rs` | WinHTTP GET/download (no TLS crate needed) |
| `app.rs` | Orchestrator: pill + tray + popup + timers + notification engine |
| `main.rs` | Entry point: single-instance mutex, panic log, `.old` cleanup |
