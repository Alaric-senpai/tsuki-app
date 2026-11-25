# Third-party components

The app itself is MIT-free-form personal software; it depends on the following
third-party components. Binaries that are **bundled** are fetched by
`dstauri/scripts/fetch-sidecars.sh` and are not committed to the repository.

## Bundled sidecars

| Component | Version pin | License | Why it is bundled |
|---|---|---|---|
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | latest release at fetch time (PyInstaller standalone, no Python required) | Unlicense | Downloads and merges episode streams. It changes often, so the app prefers a copy in `<app_data>/bin` over the bundled one. |
| [FFmpeg](https://ffmpeg.org/) | Linux: johnvansickle static build 7.0.2 · Windows: gyan.dev release-essentials · macOS: Homebrew binary at fetch time | LGPL-2.1+ / GPL-2.0+ depending on the build | Muxing/merging separate video and audio streams, subtitle remuxing. The Linux static build is GPL; it is invoked as a **separate process** (`--ffmpeg-location`), never linked, so it does not affect the licence of the app's own code. |

The sidecars are spawned as child processes. Nothing in this repository links
against FFmpeg or yt-dlp code.

## Rust crates

Managed through `dstauri/src-tauri/Cargo.toml`; licences are those of the crates
themselves (predominantly MIT/Apache-2.0). Run `cargo license` for a full list.

Notable ones:

- `tauri` / `tauri-build` / plugins — MIT/Apache-2.0
- `sqlx` (SQLite) — MIT/Apache-2.0
- `reqwest` (rustls) — MIT/Apache-2.0
- `regex`, `serde`, `tokio`, `thiserror` — MIT/Apache-2.0

## npm packages

Managed through `dstauri/package.json`; predominantly MIT. Notable:

- `next`, `react`, `react-dom` — MIT
- `@tauri-apps/*` — MIT/Apache-2.0
- `@tanstack/react-query`, `zustand`, `zod`, `react-hook-form` — MIT
- `react-player`, `hls.js` — MIT
- `tailwindcss`, `shadcn`-generated components, `lucide-react` — MIT

## External services

- **YouTube Data API v3** — used only when the user supplies their own API key in
  Settings. Subject to Google's terms and quota policy.
- **DonghuaStream / AnimeCube** — public websites scraped by the app at the user's
  request. No content is redistributed.

## Bundled fonts

The app bundles latin subsets of three variable fonts locally so it never fetches
from Google Fonts at runtime. All three are licensed under the SIL Open Font
License 1.1; the full license texts ship beside the font files:

- **Geist** — `public/fonts/geist-latin.woff2` (`geist-OFL.txt`)
- **Plus Jakarta Sans** — `public/fonts/plus-jakarta-sans-latin.woff2` (`plusjakartasans-OFL.txt`)
- **JetBrains Mono** — `public/fonts/jetbrains-mono-latin.woff2` (`jetbrainsmono-OFL.txt`)
