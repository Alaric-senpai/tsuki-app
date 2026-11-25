# AGENTS.md — Donghua Stream: PyQt6 → Tauri + Next.js migration

This file is the source of truth for any AI agent (or human) working in this repo.
Read it fully before writing code. If something here conflicts with a request, ask before deviating.

---

## 1. Mission

Port **Donghua Stream**, a Python/PyQt6 desktop catalog and yt-dlp download manager for donghua, to a
**Tauri v2 + Rust + Next.js (TypeScript) + shadcn/ui + Tailwind** desktop app.

The finished app must be:

1. **Feature-complete** against the legacy app (library, explore, series detail, add wizard, downloads, settings).
2. **Self-contained.** No PostgreSQL, no system media player, no Python runtime required.
3. **A streaming app, not only a downloader.** Remote sources and downloaded files both play inside the app.
4. **Built from reusable components**, visually faithful to the Stitch design.

The legacy project is a personal, non-commercial passion project. Carry its `DISCLAIMER.md` forward
and keep the notice visible in the About dialog.

---

## 2. Legacy reference

The legacy Python repo (`Alaric-senpai/Donghua-stream`) must be available at `./legacy/`
(submodule or plain clone). **If it is missing, stop and ask for it.** Never port from memory or guess behavior.
In this checkout the legacy app lives at the repository root (the Tauri app is in `dstauri/`), so use
the root files as the reference and `../` from the Tauri app.

Always read the legacy file before porting it, and port its tests too (`legacy/tests/`, stdlib `unittest`).

| Legacy | Responsibility | Becomes |
|---|---|---|
| `app_config.py` | JSON config, 0700 dir / 0600 file, atomic save | `src-tauri/src/config.rs` |
| `db.py` | PostgreSQL catalog, auto schema, sync | `src-tauri/src/db/` (**SQLite**, see §7) |
| `downloader.py` | yt-dlp options, retries, quality, subtitles, archive, events | `src-tauri/src/downloader/` |
| `media_ids.py` | Stable per-source IDs, `downloaded.txt` archive tags | `src-tauri/src/media_ids.rs` |
| `sources.py` | AnimeCube / DonghuaStream search, details, recs, playback resolution | `src-tauri/src/sources/` |
| `youtube_api.py` | YouTube Data API v3 client + disk cache | `src-tauri/src/youtube.rs` |
| `urls.py` | Fallback catalog, legacy compat | seed data / dropped if unused |
| `script.py`, `gui_entry.py` | CLI and launcher | **Not ported** (no CLI in v1) |
| `gui/pages/*` | Home, Explore, Library, Series detail, Downloads, Settings | `src/app/**` routes + `src/components/features/**` |
| `gui/components/*`, `gui/dialogs/*` | UI primitives, wizard, credentials, URL import | `src/components/**` |
| `gui/infrastructure/*` | db_worker, download_queue, thumbnails, size_scanner | Rust services + tokio tasks |
| `gui/theme.py` | Design tokens + stylesheet | Tailwind theme + CSS variables, **sourced from Stitch** |
| `oldscript.py` | Dead code | Ignore |

### Behavior that must be preserved exactly

- An episode is **complete only after** every stream is saved, the merge postprocessor ran, and the final
  file exists on disk with **nonzero size**. Multi-stream downloads show partial progress until then.
- **Failures are reported only after the retry budget is exhausted.** Retry noise goes to the console, never the error banner.
- yt-dlp gets 10 transport/fragment retries and a 30 s socket timeout; per-video retry budget with backoff.
- Episode states: `ready`, `queued`, `downloaded`, `failed`. Per-episode retry.
- Concurrent queue (configurable, default 3) with FIFO ordering, priority reorder, pause/resume, stop-current.
- Quality presets 360p–1080p + Best Available; MP4/MKV/WebM; SRT subtitles in configurable languages.
- Browser-cookie import, bandwidth limit, per-series folder + quality override, disk-usage scan.
- `downloaded.txt` archive survives renames and re-syncs.
- Source episode pages resolve to a direct playable URL **at download/play time**; the library stores the stable page URL for dedup.
- YouTube search: `search.list` (100 units) + one `videos.list`; results cached on disk (default 20 min); nothing calls the API until the user submits a search.
- AnimeCube and DonghuaStream search/trending need no account or API key.
- Errors are friendly **and copyable**.

---

## 3. Scaffold status (what exists vs. what is missing)

The project was generated with `create-tauri-ui` (Next.js template). Per the template's docs it provides:
shadcn/ui + Radix, Tailwind, Lucide icons, dark/light mode, native-looking window controls via
`tauri-controls`, an About dialog, `tauri-plugin-app` / `-os` / `-shell`, and a GitHub release workflow.

**Do not assume. Open `package.json`, `src-tauri/Cargo.toml`, `next.config.*` and `tauri.conf.json` and
verify versions first.** The template originally targeted older Next/Tailwind majors; this project runs a newer
Next.js, so check Tailwind's config style (v3 `tailwind.config.ts` vs v4 CSS-first `@theme`) and follow what is actually installed.
The template README says `shadcn-ui@latest`; the current CLI is `shadcn@latest`.

### Missing — add during Phase 0

**Rust (`src-tauri/Cargo.toml`)**

| Need | Crate / plugin |
|---|---|
| Embedded database | `sqlx` (features: `sqlite`, `runtime-tokio`, `migrate`) — **not** `tauri-plugin-sql` |
| HTTP | `reqwest` (rustls) |
| HTML scraping | `regex` (chosen over `scraper`: the legacy parsers are regex-based and port 1:1) |
| Async / errors | `tokio`, `thiserror`, `anyhow` |
| Serialization | `serde`, `serde_json` |
| Typed IPC bindings | `tauri-specta` + `specta` (or `ts-rs`) |
| Logging | `tauri-plugin-log`, `tracing` |
| Dialogs / fs / opener | `tauri-plugin-dialog`, `-fs`, `-opener` |
| Clipboard (copyable errors) | `tauri-plugin-clipboard-manager` |
| Notifications | `tauri-plugin-notification` |
| Window state | `tauri-plugin-window-state` |
| Single instance | `tauri-plugin-single-instance` |
| Sidecars (yt-dlp, ffmpeg) | `tokio::process` (Rust owns the child process). `tauri-plugin-shell` is **not** in `Cargo.toml`; add it only if the frontend ever needs to spawn something itself. Bundled via `externalBin`; `bun run sidecars` (= `scripts/fetch-sidecars.sh`) puts the host build in `src-tauri/binaries/`, which is gitignored. |
| Player | see §8 |

**Frontend (`package.json`, use `bun`)**

`@tanstack/react-query`, `zustand`, `zod`, `react-hook-form` + `@hookform/resolvers`, `sonner` (toasts),
`hls.js` (player fallback), `date-fns`, `vitest`, `@testing-library/react`, `eslint` + `prettier`.

**Config / infra**

- `capabilities/*.json` entries for every plugin used (Tauri v2 is deny-by-default).
- A real CSP in `tauri.conf.json` (media, connect, img sources for posters and the local stream proxy).
- `externalBin` sidecars for `yt-dlp` and `ffmpeg` (target-triple suffixed filenames).
- Window `transparent` only if the chosen mpv embedding requires it (§8).
- CI: add `cargo clippy`, `cargo test`, `tsc --noEmit`, `bun run lint`, `bun run build` to the existing workflow.

---

## 4. Hard constraints

1. **Next.js is a static SPA inside Tauri.** `output: 'export'`, `images: { unoptimized: true }`.
   **No** API routes, server actions, `getServerSideProps`, middleware, `next/image` optimization, or any Node runtime.
   Every page that touches Tauri is a client component (`'use client'`). Dynamic segments need
   `generateStaticParams` or, preferably, query params (`/series?id=…`) / client routing.
2. **All I/O lives in Rust.** Network, DB, filesystem, process spawning: Rust commands only.
   The frontend never uses `fetch` for third-party sites (CORS, headers, cookies) and never touches SQLite directly.
3. **No PostgreSQL, no Python, no system player dependency** in the shipped app.
4. **No secrets in the repo.** API keys (YouTube, Stitch) come from user config or env, never committed.
5. **Linux is the dev platform; keep Windows/macOS viable.** No Linux-only assumptions in app logic.
6. The user also develops on a phone via Termux. Native builds don't work there, so keep TS/Rust
   changes small, self-contained and reviewable as plain diffs.

---

## 5. Design: Stitch is the source of truth for UI

The visual design is in **Google Stitch**. When building or changing any UI, **pull the design from Stitch first**.
Never invent layouts, colors, spacing, or copy that Stitch already defines.

**Workflow (every UI task):**

1. Use the **Stitch MCP server** (discover its actual tool names at runtime; typically project/screen listing,
   fetching a screen's HTML/code, and fetching a screen image). Authentication is via the user's Stitch API key
   or OAuth, supplied through environment/MCP config, never committed.
2. Locate the project (`STITCH_PROJECT_ID`: ask the user if not set below), list screens, and map the screen to its route (table below).
3. Fetch the screen's **code and screenshot**, plus the project's `DESIGN.md` if present.
4. **First time only:** extract tokens (colors, radii, spacing, type scale, shadows, motion) into
   CSS variables + the Tailwind theme, and record them in `docs/design/tokens.md`. After that, always use tokens, never raw hex.
5. **Rebuild with shadcn/ui + Tailwind** and the shared components in §9. Do **not** paste Stitch's raw HTML into pages.
6. Compare the result against the screenshot (layout, hierarchy, spacing, states) before calling it done.
7. **Fallback:** if MCP is unavailable, ask the user to export HTML + PNG (+ `DESIGN.md`) into `design/stitch/`.
   Work from those. If neither exists, **stop and ask**. Do not improvise a design.

Design must include real states: loading (skeletons), empty, error (copyable), hover quick-actions, disabled, dark/light.

**Screen → route map** (Stitch project `9300912284796071368`, "Donghua Streaming Hub"):

| Legacy page | Route | Stitch screen |
|---|---|---|
| Home (hero, filters, poster/list) | `/` | Home — `a3e51831b68e445da73597fc41147c82` |
| Explore (per-source tabs) | `/explore` | Explore YouTube — `4a108cf00d12455d99fea88b0fd108c3`; Explore Donghua — `57156e2a84884c4ab9300141798198e7` |
| Source show details | `/show?source=&id=` | no dedicated screen — compose from Series Details + poster grid |
| Library | `/library` | Library — `d237fc2b7c3443c88ab48257116a0d56` |
| Series detail | `/series?id=` | Series Details — `131de03dffa747069507f4abebf5e61f` |
| Add series wizard (3 steps) | dialog | TBD |
| Downloads center | `/downloads` | Download Center — `f38bbe1dacb349af9134c633f604d4d7` |
| Player | `/watch` + mini-player | Theater Mode Player — `07d31346f286421cb5a599a166531695` |
| Settings | `/settings` | Settings — `de1b2c77759d46e1803c376f3c339eb6` |
| About | `/about` | no screen — built from `DISCLAIMER.md` |

---

## 6. Architecture

```
src-tauri/
  migrations/                 # sqlx SQL migrations (0001_init.sql, …)
  src/
    main.rs / lib.rs          # builder, plugin + state registration
    commands/                 # thin #[tauri::command] handlers, one file per domain
    config.rs                 # JSON config, atomic write, schema versioning
    db/                       # pool, models, queries (library, episodes, progress)
    downloader/               # queue.rs (actor), ytdlp.rs, events.rs, archive.rs
    sources/                  # animecube.rs, donghuastream.rs, resolver.rs
    youtube.rs                # API client + disk cache
    media_ids.rs
    player/                   # engine glue, stream proxy (see §8)
    error.rs                  # AppError (serde-serializable, user-friendly message + detail)
src/
  app/                        # Next app router (routes only; no logic)
  components/
    ui/                       # shadcn primitives (CLI-managed)
    common/                   # generic reusable pieces
    features/<domain>/        # library, explore, downloads, series, settings, wizard
    player/                   # player UI, engine-agnostic
    layout/                   # shell, sidebar, titlebar
  hooks/                      # useSeries, useDownloadQueue, usePlayer, …
  lib/
    ipc/                      # typed wrappers around invoke/listen, one file per domain
    player/                   # engine interface + adapters
    utils.ts                  # cn(), formatters
  stores/                     # zustand (UI/player/session state only)
  types/                      # generated bindings live here (do not hand-edit)
```

**IPC rules**

- Request/response → `#[tauri::command]`. Streaming (progress, console lines, sync status, player state) → Tauri **events**,
  namespaced like `download://progress`, `download://console`, `player://state`.
- Generate TS types from Rust so both sides never drift. Don't hand-write duplicate types.
  **Decision: `ts-rs`, not `tauri-specta`.** `tauri-specta` is still at `2.0.0-rc` and would add an RC dependency
  to the whole command surface; `ts-rs` is stable, and `#[derive(TS)] #[ts(export, export_to = "../../lib/ipc/bindings/")]`
  emits the `.ts` files during `cargo test`. Applied at the **end of Phase 2**, once that phase's new payload types exist.
- **Components never call `invoke` directly.** They use hooks → `lib/ipc/*` → `invoke`. Server state goes through TanStack Query; zustand holds only UI-local state.
- Rust errors serialize to `{ code, message, detail }`. The UI shows `message` and offers **Copy details**.
- Long work runs on tokio tasks; never block a command handler.

---

## 7. Database: in-app SQLite (replaces PostgreSQL)

- Engine: **SQLite via `sqlx`**, owned by Rust. File: `<app_data_dir>/tsuki.db`. Created and migrated automatically on first launch.
  The first-run PostgreSQL credentials prompt **no longer exists**.
- Pragmas on connect: `journal_mode=WAL`, `foreign_keys=ON`, `busy_timeout`.
- Schema is versioned with `sqlx::migrate!` in `src-tauri/migrations/`. Never edit an applied migration; add a new one.
- **Read `legacy/db.py` and mirror its tables, columns and sync semantics** (series, episodes, status values, quality
  overrides, save folders). Translate types carefully: Postgres `SERIAL`/`TIMESTAMPTZ`/`BOOLEAN`/enums → SQLite
  `INTEGER PRIMARY KEY`, ISO-8601 `TEXT`, `INTEGER 0/1`, `TEXT CHECK(...)`.
- New tables for the player: `watch_progress (episode_id, position_sec, duration_sec, updated_at, completed)`.
- Settings stay in the **JSON config file** (as in legacy), not in the DB. Drop the legacy `app_settings` import.
- **Legacy data import (optional, never a runtime dependency):** provide `tools/export_legacy.py` that dumps the old
  Postgres catalog to JSON, and an `import_legacy_json` command that loads it. The shipped app must not link any Postgres client.
- Query code lives in `db/`; commands call it. No SQL in command handlers or the frontend.

---

## 8. Media player (self-contained, mpv-first, external supported)

**Goals:** play **downloaded files** and **remote streams** inside the app, with no dependence on a system player.
Let the user optionally hand off to an external player (mpv, VLC, or a custom command).

### Why this needs care

Linux Tauri uses **WebKitGTK**, whose `<video>`/MSE/HLS support depends on the user's GStreamer plugins and is
known to be flaky (HLS stalls, seek breakage, missing codecs, MKV/HEVC gaps). **Do not make HTML5 `<video>` the only engine on Linux.**
The Tauri libmpv plugin ecosystem is also young: `tauri-plugin-libmpv` documents Windows as the tested platform and
Linux window embedding as not working (at the time of writing). **Verify current status before choosing.**

### Required abstraction (build this first)

`src/lib/player/engine.ts` defines a `PlayerEngine` interface. UI components talk **only** to this interface, never to a concrete engine.

```ts
interface PlayerEngine {
  load(src: PlayableSource, opts?: { startAt?: number }): Promise<void>;
  play(): Promise<void>; pause(): Promise<void>;
  seek(seconds: number): Promise<void>;
  setVolume(v: number): void; setRate(r: number): void;
  audioTracks(): Track[]; subtitleTracks(): Track[];
  selectAudio(id: string): void; selectSubtitle(id: string | null): void;
  on<E extends PlayerEvent>(e: E, cb: (p: PlayerEventMap[E]) => void): Unsub;
  dispose(): Promise<void>;
}
// PlayableSource = { kind: 'file', path } | { kind: 'stream', url, headers?, subtitles? }
```

Adapters (`src/lib/player/adapters/`):

| Adapter | Role |
|---|---|
| `MpvEngine` | **Preferred embedded engine** (libmpv driven from Rust; state via `player://state` events) |
| `HtmlVideoEngine` | Fallback using `<video>` + `hls.js`; use for mp4/h264/AAC and where the webview supports it |
| `ExternalPlayerEngine` | Spawns the user's chosen player. For mpv, use `--input-ipc-server` to sync progress back |

Engine selection is a setting (`player.engine = auto | mpv | html | external`); `auto` picks the best working engine at startup
via a capability probe and falls back gracefully with a visible, non-blocking notice.

### Phase 0b: player spike (do before building player UI)

Prove, on the dev machine, an embedded engine that meets **all** of these. Record the outcome in `docs/decisions/player.md`.

1. Plays a local MKV (H.264/HEVC, soft subs) and MP4 smoothly at 1080p.
2. Plays a remote HLS stream with **custom headers** (`Referer`, `User-Agent`).
3. Seeks repeatedly without freezing; pause/resume work.
4. Fullscreen works; the player UI (controls, titlebar) overlays correctly.
5. Works on the dev compositor (check **Wayland and X11**).

Candidates to evaluate, in order (stop at the first that passes; do not guess, test):

1. `tauri-plugin-libmpv` (check the latest version's Linux status).
2. `tauri-plugin-video` (GStreamer default, optional `mpv-runtime` feature, lists Linux support): evaluate maturity.
3. Own Rust integration with the `libmpv2` crate (render API / native child surface) behind `MpvEngine`.
4. mpv as a child process (`--wid` embedding is X11-only; not viable on Wayland. Prefer IPC-controlled windowed mpv as the "external" path instead).
5. `HtmlVideoEngine` only, **plus** an in-app prompt to install required GStreamer packages (last resort).

### Streaming remote sources

- The `sources` module resolves an episode page → `{ url, headers, subtitles[], isHls }` at play time. Never persist resolved URLs (they expire).
- For `HtmlVideoEngine`, route playback through a **loopback stream proxy** in Rust: `127.0.0.1:<random port>`,
  per-session token, injects required headers, supports `Range`, and rewrites HLS manifests so segments also go through the proxy.
  Bind to loopback only; reject requests without the token.
- For `MpvEngine`, pass headers directly via mpv options (`http-header-fields`, `referrer`, `user-agent`); no proxy needed.
- Downloaded files: play from disk path. For the webview engine serve through Tauri's asset protocol
  (`convertFileSrc`) with the scope **restricted to the user's download directories**.
- Player UX: resume from `watch_progress`, auto-mark watched at ~90%, next-episode, audio/subtitle track menus, speed,
  keyboard shortcuts, mini-player that persists across route changes. Both sources of an episode (downloaded vs. stream)
  are offered; prefer the local file when present.

---

## 9. Reusable components (non-negotiable)

If a UI pattern appears twice, it becomes a component. Pages only **compose**.

- A page file (`app/**/page.tsx`) stays under ~150 lines, with no inline business logic, no raw `invoke`, no inline SQL/API shapes.
- Build presentational components (props in, JSX out) separate from container hooks (data in, actions out).
- Props are fully typed; variants use `cva`; classes are merged with `cn()`; **no hardcoded colors or spacing**, only theme tokens.
- Do not edit `components/ui/*` by hand. Re-generate with the shadcn CLI; wrap or extend in `components/common/`.
- Every component handles loading, empty and error states, and is keyboard-accessible with correct ARIA.

Expected shared set (create once, reuse everywhere):

`PosterCard`, `PosterGrid` / `PosterList`, `HeroSpotlight`, `StatusBadge`, `StatusFilterBar`, `SourceTabs`,
`EpisodeRow`, `EpisodeList` (All/None/Latest selection), `EpisodeStatusTabs`, `SyncProgressCard`, `QualitySelect`,
`FolderPicker`, `DownloadQueueItem`, `TelemetryCard`, `ConsoleView` (colorized, copyable), `ErrorPanel` (copyable),
`EmptyState`, `SkeletonGrid`, `Stepper` / `WizardShell`, `SettingsSection`, `ConfirmDialog`, `AppShell`, `Sidebar`, `Titlebar`,
and under `components/player/`: `VideoSurface`, `ControlBar`, `SeekBar`, `TrackMenu`, `MiniPlayer`, `PlayerOverlay`.

---

## 10. Config, paths, and bundled binaries

- Config: JSON in the OS config dir via Tauri's path API (Linux: `~/.config/<identifier>/config.json`), atomic write (temp file + rename),
  perms `0600` file / `0700` dir on Unix, `version` field with migrations. Keep legacy v2 keys **minus** the `database` block; add a `player` block.
- App data (`tsuki.db`, caches, resolved metadata): `app_data_dir`. Cache dir: YouTube responses and remote thumbnails, separate subfolders as in legacy.
- **Self-contained means bundled:** ship `yt-dlp` and `ffmpeg` as sidecars (`externalBin`), plus libmpv if the chosen engine needs it.
  `yt-dlp` breaks often; implement an in-app updater that stores a newer binary in app data and prefers it over the bundled one.
- Mind licensing when bundling (libmpv/ffmpeg are LGPL/GPL depending on build; link dynamically, document in `THIRD_PARTY.md`).
- First-run checks show actionable errors (missing binary, unwritable download dir), never silent failure.

---

## 11. Coding standards and quality gates

**Rust:** `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`. No `unwrap()`/`expect()` outside tests; use `AppError`.
Parsers (scrapers, yt-dlp progress, media IDs, archive tags) are pure functions with unit tests, with fixtures copied from real responses.

**TypeScript:** `strict` on, no `any`, no unused exports. `bun run lint`, `tsc --noEmit`, `vitest` for hooks and logic-bearing components.

**Before declaring any task done**, run: `cargo clippy`, `cargo test`, `tsc --noEmit`, `bun run lint`, `bun run build` (static export must succeed).
For UI tasks also compare against the Stitch screenshot. If the environment can't run a command (e.g. no desktop libs), say so explicitly. Never claim it passed.

**Commits:** small, one concern each, conventional style (`feat(downloads): …`).

**Tests to port from legacy:** config normalization/permissions, YouTube parsing/errors/caching, downloader retry semantics
(no failure event until budget exhausted), media IDs/archive tags, and "no API call on Explore load".

---

## 12. Phases (work in order; mark done in `docs/PROGRESS.md`)

**Phase 0: Foundations.** Verify scaffold versions · add missing deps (§3) · static export + CSP + capabilities ·
Stitch token extraction → theme · `AppShell`/`Sidebar`/`Titlebar` · typed IPC bindings (deferred to end of Phase 2) · CI gates.
**Phase 0b: Player spike (§8).** Decision recorded before any player UI.
**Phase 1: Data.** `config.rs` · SQLite + migrations · legacy JSON import · Library + Home (read-only, real data).
**Phase 2: Downloader.** *(in progress)* yt-dlp process driver · concurrent queue (configurable, default 3, FIFO) with cancel/retry ·
retry semantics · archive · events · Downloads page · **bundled `yt-dlp` + `ffmpeg` sidecars** (`externalBin` + `scripts/fetch-sidecars.sh`).
Still open: pause/resume, priority reorder, disk-usage scan, yt-dlp updater, typed IPC bindings.
**Phase 3: Sources & Explore.** `youtube.rs` + cache · AnimeCube scraper (DonghuaStream resolver already ported in Phase 2) ·
Explore tabs · show details · Add Series (existing/new) · Add wizard.
**Phase 4: Player.** `PlayerEngine` + adapters · local playback · streaming (+ proxy) · controls UI · resume/watched · external-player handoff.
**Phase 5: Settings & polish.** Settings page · notifications · single-instance/window-state · yt-dlp updater · packaging (`.deb`, AppImage, etc.) · About + disclaimer.

Each phase ends with a working, demoable app, not a half-wired branch.

---

## 13. Agent working rules

- **Read before you port.** Open the legacy file and its tests first. Match behavior, not just names.
- **Don't expand scope.** Only what the phase and the Stitch design call for. Note ideas in `docs/IDEAS.md`.
- **Ask when blocked** on a missing design, missing legacy file, or an ambiguous requirement. Don't fabricate.
- **Never** commit secrets, hand-edit generated bindings, hand-edit `components/ui/*`, run `git push --force`, or delete the legacy folder.
- Keep this file current. If a decision changes (player engine, schema, layout), update AGENTS.md in the same commit.
- End every session by updating `docs/PROGRESS.md`: what's done, what's next, known issues, and open questions.

---

## 14. Open decisions (resolve with the user, then record here)

- [x] `STITCH_PROJECT_ID` and the screen → route map (§5) — project `9300912284796071368`; map filled in above.
- [ ] Embedded player engine result of the spike (§8)
- [x] Legacy data import in v1 — implemented as staged JSON (`tools/export_legacy.py` → `import_legacy_json`), runs on startup when `meta.legacy_data_imported` is unset. No PostgreSQL client ships in the app.
- [x] Bundle `ffmpeg` (size) or require it on PATH? — **bundle both `yt-dlp` and `ffmpeg`.** `tauri.conf.json` declares them as `externalBin` under the private names `binaries/tsuki-yt-dlp` / `binaries/tsuki-ffmpeg` (Linux packages install these into `/usr/bin`; plain names collide with Fedora's `yt-dlp`/`ffmpeg` packages). `scripts/fetch-sidecars.sh` fetches the host build into `src-tauri/binaries/` (gitignored), and `YtDlp::locate` prefers `<app_data>/bin` → prefixed sidecar next to the executable → legacy/plain name → `PATH`. `--ffmpeg-location` is passed explicitly so the merge step never silently falls back. ~120 MB per platform; `THIRD_PARTY.md` records licences.
- [x] Release targets — macOS/darwin jobs are skipped in `.github/workflows/release.yml`; only Windows and Linux are built.
- [x] Target platforms for release builds — Windows (`nsis`) and Linux (`appimage`, `deb`, `rpm`); macOS/darwin jobs are skipped in release CI.