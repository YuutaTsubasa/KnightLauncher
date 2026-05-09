# KnightLauncher

A dual-screen Windows game launcher for handhelds. Pulls your Steam library,
EmuDeck ROMs, and RPCS3 installs into one grid, with achievement progress for
each source.

[![Latest release](https://img.shields.io/github/v/release/YuutaTsubasa/KnightLauncher)](https://github.com/YuutaTsubasa/KnightLauncher/releases)
[![CI](https://github.com/YuutaTsubasa/KnightLauncher/actions/workflows/ci.yml/badge.svg)](https://github.com/YuutaTsubasa/KnightLauncher/actions)

Built with [Tauri 2](https://tauri.app/) + [Svelte 5](https://svelte.dev/) +
Rust. Single binary, runs in the WebView2 runtime that ships with Windows 11.

## Features

- **One library, three sources** — auto-detects installed Steam games, scans
  EmuDeck ROM folders, and pulls PS3 titles from your RPCS3 install.
- **Achievement progress per source** — Steam (public profile XML),
  RetroAchievements (REST API), PS3 trophies (parsed from local
  `TROPCONF.SFM` + `TROPUSR.DAT`).
- **Auto-refresh on game exit** — watches the launched process and re-syncs
  achievements when the game closes.
- **Dual monitor layout** — splits library grid and game detail across two
  displays for handhelds with a primary and secondary screen.
- **Controller navigation** — gamepad-driven selection, launch, and editor
  flow. Status pills show clock, battery, and network at a glance.
- **Artwork pipeline** — fetches covers / heroes / logos from SteamGridDB or
  Google Image Search, normalizes everything to WebP, and resizes per asset
  type (cover ≤ 1024×1536, hero ≤ 1920×1080, logo ≤ 512×512, badge ≤ 256×256).
- **Library backup** — every save first stashes the previous `library.json`
  / `settings.json` to a sibling `.bak`, so a misclicked merge or unlink can
  be recovered manually.

## Supported sources

| Source | Scan | Launch | Achievements |
|---|---|---|---|
| Steam | `appmanifest_*.acf` parser | `steam://rungameid/<id>` | Public profile XML |
| EmuDeck ROMs | system folders + extension match (33 systems) | EmuDeck `tools/launchers/<emu>.ps1` | RetroAchievements API |
| RPCS3 (PS3) | `dev_hdd0/game/*/USRDIR/EBOOT.BIN` | `rpcs3.ps1 <eboot>` | Local TROPCONF.SFM + TROPUSR.DAT |
| Plain executable | `Add executable` button | spawned with custom args + cwd | (none) |

The EmuDeck launcher is invoked with the right `-L <core>.dll` for RetroArch
systems, falling back to the standalone emulator (Dolphin, melonDS,
DuckStation, RPCS3, etc.) when one is available.

## Install

Pre-built Windows installers are attached to each tagged GitHub release:

- <https://github.com/YuutaTsubasa/KnightLauncher/releases/latest>

Tags push to the `windows-release.yml` workflow which builds the MSI/NSIS
bundle on `windows-latest`.

## Build from source

Requires:

- Node 24
- Rust stable
- The standard Tauri 2 host requirements (WebView2 on Windows; `webkit2gtk-4.1`
  + `gtk-3` + `libsoup-3` on Linux for development builds)

```sh
npm ci
npm run tauri:dev      # dev mode with hot reload
npm run tauri:build    # release build
```

Front-end-only iteration (no Rust rebuild) runs on `npm run dev`.

## Project layout

```
src/                   Svelte 5 frontend
  App.svelte           main shell, library grid, editor, status bar
  lib/
    libraryStore.ts    writable stores + actions (effectiveAchievements, scans, links)
    tauri.ts           thin invoke() wrappers
    platforms.ts       PLATFORMS table + canonicalPlatformId / resolvePlatform
    types.ts           Game / Library / RetroAchievementsLink types
    PlatformBadge.svelte

src-tauri/src/
  lib.rs               Tauri lifecycle, window orchestration, library/settings IO
  steam.rs             VDF parsers, scan, achievement scrape
  emudeck.rs           EMU_SYSTEMS table, ROM scan, launcher dispatch
  ps3.rs               PARAM.SFO + TROPCONF.SFM + TROPUSR.DAT parsers, trophy linking
  ra.rs                RetroAchievements REST API + game/variant linking
  artwork.rs           ArtworkKind, WebP conversion, download_to
  logger.rs            file-backed logger ([APPDATA]/logs/knightlauncher.log)

references/            golden samples used by unit tests (TROPUSR/, _SYMBOLS_/)
.github/workflows/
  ci.yml               npm run check + cargo test on push/PR
  windows-release.yml  Windows installer build on tag push
```

## Development workflow

```sh
npm run check                      # svelte-check (TypeScript + Svelte)
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Both run on every push via `ci.yml`.

Parser tests live next to the parsers under each module's `mod tests`. The
golden fixtures are under `references/TROPUSR/` and are loaded with
`include_bytes!` / `include_str!` so the tests are self-contained.

## Versioning + release

Three files carry the version string and must stay in sync:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

The HTTP user agent picks up `CARGO_PKG_VERSION` automatically. Tagging
`v<version>` triggers `windows-release.yml`, which builds and attaches the
Windows bundle to a new GitHub release.

## Data locations

- Library + settings: `%APPDATA%\com.knightlauncher.app\library.json` (and
  `settings.json`) plus matching `.bak` from the previous save.
- Game artwork cache: `%APPDATA%\com.knightlauncher.app\artwork\<game-id>\`.
- Achievement icon caches:
  - RetroAchievements: `%APPDATA%\com.knightlauncher.app\retroachievements\<ra-game-id>\`
  - Steam: `%APPDATA%\com.knightlauncher.app\steam_achievements\<app-id>\`
  - PS3 trophies: `%LOCALAPPDATA%\com.knightlauncher.app\cache\ps3_trophies\<NPWR>\`
- Logs: `%APPDATA%\com.knightlauncher.app\logs\knightlauncher.log` (rotates to
  `.log.old` once it passes 1 MiB).

## Acknowledgements

- [Tauri](https://tauri.app/) for the Rust + WebView shell.
- [SteamGridDB](https://www.steamgriddb.com/) for box art and logos.
- [RetroAchievements](https://retroachievements.org/) for the achievement
  database and API.
- [lucide](https://lucide.dev/) for the icon set.
- The [hedge-dev/UnleashedRecomp](https://github.com/hedge-dev/UnleashedRecomp)
  team for documenting `ACH-DATA` cleanly enough that integration was a
  weekend conversation.
