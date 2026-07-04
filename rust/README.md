# PrayerTray — native rewrite (Rust)

Native Win32 rewrite of PrayerTray: one small exe, no .NET runtime, same features,
same `%APPDATA%\PrayerTray\config.json`. Replaced the C# build at v2.0.0; the C# tree
lives on in `../legacy-dotnet/`.

## Build

```
cargo build --release        # rust/ — needs MSVC Build Tools + Windows SDK
cargo test                   # calc fixtures (exact-match vs the C# engine), config, i18n
```

Dev builds are version `0.0.0` (never self-update) and can run beside an installed
release: set `PRAYERTRAY_DEV_MUTEX=1`.

## Regenerating ported data

- `tools/convert_strings.py` — rebuilds `src/i18n/data.rs` from `../legacy-dotnet/I18n/Strings.cs`.
- `tools/genfix/` — rebuilds calc test fixtures + the Umm al-Qura table from the real
  .NET implementations (see its README).

## Deferred to v2.x

Net speed / ping meters, data-usage tracking, and the NIC picker. Their config fields
still round-trip so nothing is lost for users migrating from v1.
