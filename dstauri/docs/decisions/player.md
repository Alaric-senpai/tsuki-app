# Decision: embedded player engine

**Status:** accepted (pragmatic, post-hoc spike). Supersedes the open question in `AGENTS.md` §14 and completes Phase 0b.

## Context

`AGENTS.md` §8 called for a spike to prove an embedded engine that plays local MKV/MP4, remote HLS with custom headers, seeks reliably, fullscreens, and works on Wayland and X11 — without depending on a system player. The preferred path was `tauri-plugin-libmpv`; the required abstraction is `PlayerEngine` in `src/lib/player/engine.ts`.

This decision records what actually shipped and why, so future work does not re-litigate it.

## What shipped

The app is **self-contained** (no system player required) using two layers:

1. **Online streams (`/stream`)** — `react-player` v3.4.0 in `components/player/player-surface.tsx`.
   - A Tauri command `resolve_playable(page_url)` (Rust `src/commands/player.rs`) resolves a source episode page to a playable URL at play time through the existing `sources::resolve_for_download` resolver. Resolved URLs are **never persisted** (they expire).
   - `PlayerSurface` shows a loading overlay until `onReady` and a copyable error panel on real `onError`. `react-player`'s static `canPlay()` is deliberately **not** used as a gate — its HTML fallback (a plain `<video>`) still handles hosts it does not recognise (e.g. Dailymotion), and treating `canPlay === false` as fatal produced blank screens.
   - HLS is handled by `react-player`'s `hls-video-element` player.

2. **Local files (`/watch`)** — a plain `<video controls playsInline>` element whose `src` is `convertFileSrc(path)` (Tauri asset protocol, scope `$HOME/**`, `$VIDEO/**`, `$DOWNLOADS/**` in `tauri.conf.json`).
   - This deliberately bypasses `react-player`. On Linux, WebKitGTK/GStreamer's MSE/HLS behaviour is flaky and `react-player`'s player-selection/Suspense layer added a failure mode for `asset://` URLs; a native `<video>` with a clear `onError` toast is more predictable for already-downloaded MP4/MKV.
   - A finished download persists its absolute path in `episodes.file_path` (migration `0005_episode_file_path.sql`), so `/watch` keeps working after a restart or after the in-memory queue row is cleared.
   - **"Open in my player"** hands the raw path to the user's own player via the `open_in_external_player` command (custom command string, falling back to `tauri-plugin-opener`). This is the supported escape hatch for codecs the webview cannot decode.

## Why not libmpv / tauri-plugin-video

- `tauri-plugin-libmpv` documents Windows as the tested platform and Linux window embedding as not working; `tauri-plugin-video` (GStreamer) was judged immature for the required header/seek behaviour.
- A dedicated own-libmpv integration or child-process embedding would add a heavy LGPL/GPL sidecar and X11-only `--wid` embedding — disproportionate for a v1 where the webview plus an external-player hand-off covers the cases.
- No `PlayerEngine` adapter layer was built: the UI talks to `react-player`/`<video>` directly through `PlayerSurface` and `app/watch/page.tsx`. The §8 interface remains a **future refinement** if a second embedded engine is ever needed; it is not required to meet the "no system player dependency" goal today.

## Known limitations (accepted)

- **Custom `Referer`/`User-Agent` headers for remote HLS are not injected.** `resolve_playable` returns the bare URL; `sources::ResolvedMedia` has no headers field. Streams that require headers will fail and surface the copyable error panel. §8's loopback stream proxy (header injection + HLS manifest rewrite) is **not implemented** — deferred until a real source needs it.
- **Linux codec coverage depends on the user's GStreamer plugins.** MKV/HEVC or soft-sub tracks that WebKitGTK cannot decode will show a playback error; the "Open in my player" button is the remedy. This is why the external hand-off is a first-class action, not an afterthought.
- **No `watch_progress` resume yet** — `watch_progress` persistence is added in Phase 1 (migration `0006`), with player wiring to follow.

## Consequences

- Player UI stayed small and reviewable (important for the Termux workflow).
- The `docs/decisions/player.md` open question in `AGENTS.md` §14 is now answered: engine = webview (`react-player` online + `<video>` local), with external hand-off; no libmpv.
- If header-bearing HLS or reliable MKV/HEVC playback becomes a requirement, implement the §8 loopback proxy or revisit libmpv behind a `PlayerEngine` adapter at that point.
