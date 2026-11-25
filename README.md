# Tsuki

**A cinematic desktop hub for streaming and downloading the shows you follow.**

Tsuki is the successor to Donghua Stream. The PyQt6/PostgreSQL desktop app is
preserved in [`docs/legacy/README.md`](docs/legacy/README.md); this repository
ships a self-contained Tauri v2 application with a Rust backend, SQLite catalog,
and a static Next.js SPA.

> [!IMPORTANT]
> Tsuki is a personal passion project, not a product. It is not for sale or
> commercial use, carries no warranty, and is not associated with or endorsed
> by any content owner. Only download content you have the right to access.
> See [DISCLAIMER.md](DISCLAIMER.md) for the full notice.

## Features

- **Library** — series and episodes in an embedded SQLite database, with
  status tracking, per-series folders, quality overrides, archive sync, and a
  disk-usage scanner.
- **Explore** — DonghuaStream and animekhor.org catalogs with search, genres,
  details, and episode lists. YouTube is an opt-in source and never calls its
  API until you submit a search. AnimeCube is temporarily disconnected.
- **Downloads** — concurrent queue (default 3, configurable up to 64) with
  pause/resume, priority reordering, retry budget, subtitles, bandwidth limit,
  browser-cookie import, and a live console. `yt-dlp` and `ffmpeg` are bundled.
- **Player** — downloaded files play in-app from disk; remote streams play via
  `react-player`. Resume position, watched state at 90%, mini-player, keyboard
  shortcuts, track/speed menus, and an external-player hand-off are included.
- **Self-contained** — no Python runtime, PostgreSQL server, or system media
  player is required. Fonts are bundled with the app.

## Install

Download the installer for your platform from the
[GitHub Releases](https://github.com/Alaric-senpai/tsuki-app/releases) page:

| Platform | Asset |
| --- | --- |
| Linux (Debian/Ubuntu) | `.deb` |
| Linux (Fedora/RHEL) | `.rpm` |
| Linux (portable) | `.AppImage` |
| Windows | `.msi` / `.exe` |
| macOS | `.dmg` |

The app also checks GitHub for newer releases from Settings → App updates.
Installers are opened with your desktop's default handler; always review the
file before installing.

## Build from source

Prerequisites: [Bun](https://bun.sh), the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/), and Rust
stable. On Debian/Ubuntu:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf
```

Then:

```bash
bun install --frozen-lockfile
bun run sidecars          # download yt-dlp + ffmpeg into src-tauri/binaries/
bun run tauri dev         # development build
```

`sidecars` fetches host-specific binaries into `src-tauri/binaries/` (gitignored).
They are declared as Tauri `externalBin` resources and bundled into release
builds.

### Quality gates

```bash
# Frontend (from dstauri/)
bun run typecheck
bun run lint
bun run test
bun run build             # static export must succeed

# Rust (from dstauri/src-tauri/)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

To produce local packages (`src-tauri/target/release/bundle/`):

```bash
bun run tauri build --bundles appimage,deb,rpm
```

Additional tooling: Fedora/RHEL packaging needs `rpmbuild`; AppImage needs
`appimagetool`. Windows and macOS installers are produced by the release
workflow matrix and cannot be cross-built from Linux.

## Configuration

Settings live in a JSON file in the OS config directory (Linux:
`~/.config/com.alaricsenpai.tsuki/config.json`). The catalog (`tsuki.db`), caches, and
downloaded sidecar overrides live in the OS data directory.

An existing Donghua Stream config at `~/.config/donghua-stream/config.json` is
imported on first launch; the legacy PostgreSQL catalog can be staged with
`tools/export_legacy.py` and imported from Settings → Legacy data. No
PostgreSQL client ships with Tsuki.

The YouTube API key, player engine, background-download notifications, and
concurrent download limit are all configurable in Settings. YouTube and
background notifications are opt-in and disabled by default.

## Repository layout

- `dstauri/app`, `dstauri/components`, `dstauri/hooks`, `dstauri/lib` — Next.js SPA.
- `dstauri/src-tauri/src` — Rust commands, SQLite, downloader, sources, player glue.
- `dstauri/src-tauri/migrations` — append-only SQLite migrations.
- `dstauri/src-tauri/binaries` — fetched sidecars (gitignored).
- `docs/design/tokens.md` — Stitch-derived design tokens.
- `docs/plans` — implementation plans and release notes.

## License and credits

Tsuki's own code is a personal project. Third-party binaries, fonts, and
licences are listed in [`dstauri/THIRD_PARTY.md`](dstauri/THIRD_PARTY.md).
