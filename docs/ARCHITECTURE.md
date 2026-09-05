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

The original C# implementation was the porting reference; it was removed once the port settled and
lives on in git history at tag `v1.14.0`.

## Exactness: ported, not re-derived

The port had to produce the *same minutes* users already see, so nothing numeric was re-implemented
from a paper spec:

- `tests/data/reference_times.json` was dumped from the **actual C# `PrayerTimes.cs`**: 1,920 cases
  (8 cities × 5 methods × Asr rules × high-latitude rules, incl. polar summer) plus 406 Hijri cases;
  `cargo test` exact-matches them.
- The Umm al-Qura month table (`src/calc/umalqura_data.rs`) is dumped from .NET's
  `UmAlQuraCalendar` (1318–1500 AH), not computed — a generic tabular algorithm would shift dates.
- C# `Math.Round` is banker's rounding → the Rust side uses `round_ties_even()`.
- `src/i18n/data.rs` was generated from the C# `Strings.cs`. The generators were removed with the
  C# tree, so these files (and the fixtures above) are now hand-maintained frozen goldens.

Config compatibility is a hard contract: `%APPDATA%\PrayerTray\config.json`, PascalCase via serde,
sentinels preserved (`i32::MIN` popup position, `999.0` = system timezone). One exception:
`TrackWorkHours` is dropped on save now that work-hours tracking is gone; unknown keys are ignored
on load, so an older file still opens. The data-usage store
(`%APPDATA%\PrayerTray\usage.json`, `{"yyyy-MM-dd":{"Rx":n,"Tx":n}}`, 90-day retention) is
byte-compatible with the C# build's file too.

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

## Not shouting over a meeting

`MuteWhenBusy` (on by default) silences reminder tones and the azan while the user is busy. Two
probes back it, because either alone has a blind spot:

- `SHQueryUserNotificationState()` — covers full screen / exclusive D3D / presentation mode and
  Focus assist a.k.a. Do not disturb (`QUNS_QUIET_TIME`). It does **not** see a windowed video call,
  which is the case that actually matters.
- The microphone `ConsentStore` (`HKCU\…\CapabilityAccessManager\ConsentStore\microphone`, plus its
  `NonPackaged` subtree): an app holding the mic has `LastUsedTimeStart != 0` and
  `LastUsedTimeStop == 0`. That is how Windows itself drives the "mic in use" tray glyph, and it
  catches windowed Teams/Zoom/Meet. A call outranks every other reason.

`QUNS_NOT_PRESENT` (locked, screensaver) is deliberately *not* busy — that's away-from-desk, and the
azan should still play. An unrecognized future state also falls through to "free": failing open is
better than a permanently silent app.

Only the sound is suppressed; the notification still posts (Windows queues it under DND on its
own). Toasts carry `<audio silent='true'/>` already, but the tray-balloon fallback would otherwise
ding — it gets `NIIF_NOSOUND` on the busy path, so muting the adhan doesn't leave a system chime in
its place. A
swallowed azan is held in `App::muted_azan` and, once the user is free, surfaces as one silent
catch-up line that expires at the next prayer time. Replaying the adhan an hour late would be worse
than staying quiet, so it is never played retroactively.

## Network metering

The optional pill tail (down/up speed, ping, per-day data total) samples once per second on the
existing `TIMER_POS` tick — one `GetIfTable2` snapshot per tick feeds both the speed meters and the
usage accumulator. Byte counters come from `MIB_IF_ROW2` (`InOctets`/`OutOctets`), keyed by the
adapter's braced GUID so it matches the C# `NetInterfaceId`. A shared `delta()` handles the subtle
cases — a counter reset or a re-appearing adapter primes the baseline instead of injecting its
since-boot total as one spike, and a >10 s gap (sleep/resume) re-primes rather than backfilling.
The NIC picker lists only currently-up, non-filter, non-tunnel adapters (raw `GetIfTable2` also
surfaces the NDIS/WFP/QoS filter pseudo-interfaces that `.NET`'s `GetAllNetworkInterfaces` hid).

Ping runs off-thread: a short-lived probe (ICMP `IcmpSendEcho`, or a TCP :443 connect timing) at
most every 3 s writes an `AtomicI32`; the next tick reads it, so the UI never blocks. The tail keeps
a **stable width** via grow-only per-segment slots seeded from worst-case templates and reset only
on a font/DPI-scale change — live values never make the pill jitter.

## Releases

Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`: the tag version is patched into
`Cargo.toml` + `prayertray.rc`, tests run, and a **draft** release is created for manual inspection
before publishing, with `PrayerTray-win-x64.exe` as the single asset. Config path, AUMID, mutex,
and Run-key are unchanged from v1.x, so an existing install's settings carry over. Dev builds are
version `0.0.0` and never self-update; set `PRAYERTRAY_DEV_MUTEX=1` to run one beside an installed
release.

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
| `native/reg.rs` | Registry open/query/enumerate wrappers (RAII key handle) |
| `native/quiet.rs` | Busy detection: shell notification state + mic-in-use probe |
| `native/net.rs` | Adapter byte counters (GetIfTable2), NIC list, ICMP ping |
| `native/time.rs` | Local time + DST-aware UTC offset + monotonic tick |
| `ui/gdip.rs` | RAII GDI+ wrappers — the only unsafe-heavy drawing zone |
| `ui/window.rs` | Window-class/wndproc trampoline plumbing |
| `ui/widget.rs` | The overlay pill — owned by the taskbar, hooks, DPI, RTL |
| `ui/popup.rs` | Today's-times popup (pin, drag, saved position) |
| `ui/usage.rs` | Data-usage dialog (SysListView32 per-day history) |
| `ui/settings.rs` + `ui/controls.rs` | Settings dialog + themed native controls |
| `ui/icon.rs` | Tray icon, tooltip, balloon fallback |
| `ui/theme.rs` | Palettes (Dark/Light/Midnight/Slate/Warm) + Auto-follow |
| `services/net_speed.rs` + `services/latency.rs` + `services/data_usage.rs` | Speed sampler, ping probe, per-day usage store |
| `services/audio.rs` | Azan/reminder playback via MCI + synthesized tones |
| `services/toast.rs` | Action Center toasts + AUMID shortcut |
| `services/location.rs` | WinRT geolocation → IP fallback + map-link parsing |
| `services/update.rs` | GitHub release check + in-place exe swap |
| `services/http.rs` | WinHTTP GET/download (no TLS crate needed) |
| `app.rs` | Orchestrator: pill + tray + popup + timers + notification engine |
| `main.rs` | Entry point: single-instance mutex, panic log, `.old` cleanup |
