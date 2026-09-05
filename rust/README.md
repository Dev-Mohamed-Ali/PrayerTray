# PrayerTray — native rewrite (Rust)

Native Win32 rewrite of PrayerTray: one small exe, no .NET runtime, same features,
same `%APPDATA%\PrayerTray\config.json`. Replaced the C# build at v2.0.0; the C# tree
was removed afterwards and lives on in git history at tag `v1.14.0`.

## Build

```
cargo build --release        # rust/ — needs MSVC Build Tools + Windows SDK
cargo test                   # calc fixtures (exact-match vs the C# engine), config, i18n
```

Dev builds are version `0.0.0` (never self-update) and can run beside an installed
release: set `PRAYERTRAY_DEV_MUTEX=1`.

## Ported data

`src/i18n/data.rs`, `src/calc/umalqura_data.rs`, and `tests/data/*.json` were generated from the C#
tree at port time. The generators went with it — edit these by hand now. The fixtures are frozen
goldens: they still gate `cargo test`, but a change to them can no longer be checked against the
original engine (recover it from tag `v1.14.0` if that is ever needed).
