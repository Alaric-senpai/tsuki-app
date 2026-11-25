# Tsuki — Progress

A running log of what is done, what is next, known issues, and open questions.
Update this at the end of every working session (see `AGENTS.md` §13).

---

## Current session — packaging hotfix (v1.1.1-hotfix)

- Fedora RPM install failed: bundled `externalBin` files landed at `/usr/bin/yt-dlp` and
  `/usr/bin/ffmpeg`, colliding with the distro packages. Fixed by shipping them under private
  names (`tsuki-yt-dlp`, `tsuki-ffmpeg`) in `tauri.conf.json` and `scripts/fetch-sidecars.sh`.
- `YtDlp::locate` now prefers the prefixed sidecar, then the legacy/plain name (updater copies,
  older bundles, PATH). Added a unit test for the new precedence.
- macOS/darwin jobs removed from `.github/workflows/release.yml`; remaining Windows + Linux jobs
  no longer need cross-target setup or Apple signing. `tagName`/`releaseName` now follow
  `github.ref_name` so hotfix tags publish their own release.
- Resolution: an "only install if missing" RPM post-install hook cannot work — RPM file lists are
  static and the transaction is rejected before hooks run.

### Verification

- `cargo fmt --all` applied; `cargo test` run after formatting.

---

## Current session — Explore/item, player, downloads, updates & settings

**Status: the 14-task UX batch, the follow-up fix batch, and the 12 runtime-issue fixes are implemented. All gates are green, including `cargo test` (55 passed).**

### Done — 14-task batch
- Explore now shows only the active remote source; cards navigate to a dedicated `/item?source=&id=` screen (hero, metadata, genres, recommendations, episodes `DataTable`) with source-driven pagination.
- Add-to-library dialog: append the selected episodes to an existing library entry or create a new one (the per-series folder is created), with a bottom-right sonner toast on every action.
- Realtime library sync via `library://changed` events; queued episodes are re-enqueued on startup and progress is visible in the tray with completion notifications.
- Two player routes: `/stream` (online, resolved through `resolve_playable` + react-player) and `/watch` (local file via `convertFileSrc`, with an "Open in my player" hand-off).
- Downloads centre: queue/waiting/failed/completed `DataTable`s with row actions, an active-session stats row (speed/ETA/bytes), a console drawer with copy, and global actions with toasts.
- Stream/Watch column on the series and online-item tables, keyed to episode status.
- Settings: update-source picker (`donghuastream`/`animecube`, never YouTube) and a conditional Legacy section that detects a legacy install and lets the user pull/map its catalog into SQLite.
- Home: Updates section for series opted into update checks; SQL statement logging removed from startup logs.

### Done — follow-up fix batch
- Base UI `nativeButton={false}` on every Link-rendered Button (fixes the Downloads/console warning).
- Series detail refreshes the episodes table and summary after a single or bulk delete.
- Batch downloads preserve the user's selection order (FIFO) instead of database order.
- New library entries download and cache the source banner instead of showing the mascot placeholder.
- `/watch` resolves the finished file path from the database, so it works after a restart or after the queue is cleared (migration `0005_episode_file_path.sql`).
- `PlayerSurface` shows loading and copyable error states instead of a blank video box; player components remount when the source changes.
- Download queue is now concurrent and configurable (`concurrent_downloads`, default 3, FIFO-admitted) with `set_download_concurrency`; `AGENTS.md` records the approved override of the old sequential constraint.

### Done — post-restart fixes
- sqlx statement logging is fully off (`log_statements(Off)`) and the `sqlx` target is filtered to `Error`, so startup no longer floods the log with full SQL statements.
- The main window is visible at creation (`tauri.conf.json` `visible: true`) and the `on_page_load` manual `show()` hook was removed — this was causing the GTK `gtk_widget_get_scale_factor` assertion and a GUI that never appeared.

### Done — 12 runtime-issue fixes
- Player no longer hard-errors on `canPlay()===false` (Dailymotion/streamplay now render); Link-rendered Buttons use `nativeButton={false}` on `/item`.
- Concurrent queue uses one `Arc<Semaphore>` with `add_permits`/`forget_permits` so changing the limit mid-batch wakes waiters; finished rows are removed from the live queue.
- yt-dlp runs with `--no-warnings` and fails fast on permanent `Unsupported URL` errors instead of retrying five times.
- Explore pagination uses the site's real `/anime/pagg/{n}/` path; loading skeletons use the poster grid's column classes; the series parser accepts `-partN` episode URLs.
- Startup resume walks queued ids in ASC insertion order (no 50-row cap); update checks de-duplicate by canonical URL, media id and episode number.
- A banner runner (`resync_remote_banners` + Settings "Refresh") caches missing artwork from the source; `/watch` uses a plain `<video>` via `convertFileSrc`; series download actions show toasts.

### Verification — all green
- `cargo clippy --all-targets -- -D warnings`: clean (after a `collapsible_if` → let-chain fix in `downloaded_episode_path`).
- `cargo check`, `cargo test --quiet` (55 passed, 0 failed), `bun run typecheck`, `bun run lint` (0 errors, 4 pre-existing `<img>` warnings), `bun run build` (14/14 static pages): all green.

### Known issues / next
- AnimeCube is still not implemented as a remote source; YouTube search is not wired into Explore.
- `docs/decisions/player.md` (Phase 0b spike) has not been written; playback currently uses react-player with a server-resolved URL.
- Search-mode pagination (`/page/{n}/?s=`) is unverified against the live site; catalog pagination is confirmed working.

---

## Current session — Phase 2 downloader, DonghuaStream first

**Status: episodes download for real. Rust owns yt-dlp, the sequential queue, retries and completion; the UI enqueues and watches events.**

### Done

- **Source resolution ported (`src-tauri/src/sources/`).**
  - `mod.rs` — `ResolvedMedia { url, title, platform, video_id, is_hls, source }`, `source_for_url`,
    `resolve_for_download`, one shared `reqwest` client (rustls, gzip/brotli, browser UA), `fetch_text`, `is_hls_url`.
  - `donghuastream.rs` — port of `DonghuaStreamSource.resolve`: episode-URL normalisation, Dailymotion
    embed → canonical `dailymotion.com/video/<id>`, lazy-load players (`data-litespeed-src` before `src`,
    `about:blank` skipped), direct-file preference, `<h1>`/`<title>` title with HTML-entity decoding,
    HTML comments stripped. Pure parser + unit tests, including a **fixture trimmed from a live episode page**
    (`src-tauri/tests/fixtures/donghuastream_episode.html`, 2026-11-23).
  - Verified against the live site with `curl` and `yt-dlp -F` (formats `hls-380 … hls-1080`), then a real
    `yt-dlp` run using the exact argument list the Rust driver builds.
- **Download engine (`src-tauri/src/downloader/`).**
  - `quality.rs` — 360p–1080p + Best Available format selectors and `merge_container`.
  - `archive.rs` — `downloaded.txt` create/append keyed by the **stable page URL's** video ID, never the
    transient CDN URL.
  - `ytdlp.rs` — `tokio::process` driver: locate `<data_dir>/bin/yt-dlp` then `PATH`; 10 transport + 10 fragment
    retries, 3 extractor/file-access retries, 30 s socket timeout; `--no-skip-unavailable-fragments`,
    `--restrict-filenames`, `--no-simulate`; `%`-escaped safe-title output template; machine-readable
    `--progress-template "download:TSKPROG|…"`; `--print-to-file after_move:filepath`; optional cookies,
    subtitles (SRT) and rate limit. Success requires the reported final file to exist with **nonzero size**.
    Cancellation kills the child; stdout/stderr streamed via `tokio::select!`.
  - `queue.rs` — sequential worker over an unbounded command channel. Resolve → attempt loop with
    `min(30, 2 × attempt)` backoff; **failures surface only after the retry budget is exhausted**; retry noise
    goes to the console. Merge/postprocess failures purge intermediates before retrying. Completion writes the
    archive, sets the episode `downloaded`; cancel resets it to `pending`. Snapshots + cancel flags behind
    `Arc<Mutex<…>>`.
  - `mod.rs` — `normalize_browser` maps the UI's `none/off/false/…` sentinels to `None` so yt-dlp never gets
    `unsupported browser: "none"`.
- **IPC and events.**
  - `commands/downloads.rs`: `enqueue_episode_download`, `cancel_episode_download`, `download_queue`, `download_state`.
  - Job settings read from JSON config (`default_quality`, `default_container`, `sub_langs`, `max_retries`,
    `rate_limit_mb`, `browser_cookies`); save folder from the series row or `global_save_dir`.
  - Events: `download://state` (camelCase `DownloadSnapshot`) and `download://console` (`{ episodeId, line }`).
  - `AppState` now owns the `DownloadManager`; new DB helpers `set_episode_status_by_id`, `episode_job_context`.
  - `error.rs` gained `Source`/`Download` variants (`code` = `source`/`download`) with copyable detail.
- **Frontend wiring (hooks → `lib/ipc/*`, never raw `invoke`).**
  - `lib/ipc/downloads.ts`, `hooks/use-downloads.ts` (initial `download_queue` + both event listeners,
    per-episode console ring buffer of 120 lines).
  - `components/common/console-view.tsx` (colorized, copyable), `components/features/downloads/download-queue-item.tsx`
    (progress, speed/ETA/format telemetry, cancel, copyable failure panel),
    `components/features/downloads/episode-status-list.tsx`.
  - `app/downloads/page.tsx` rewritten: live queue + migrated **Waiting in queue / Needs attention / Completed**
    sections, engine status chip, archive sync, retry action, catalog refresh when a job settles.
  - `app/series/page.tsx` episode row menu gained **Download** (disabled when already downloaded) with an inline
    copyable error panel. No toasts (no Toaster is mounted).
- **Pre-existing lint debt cleared** so the gates are green: `data-table.tsx` de-`any`'d into a generic
  `DataTable<T>` with a structural column type; `series-grid.tsx` `onEdit` typed and actually wired to a hover
  edit button; unescaped quotes and hook deps fixed in `app/series/page.tsx`; a `use-library.ts` return-type bug
  in `remove` fixed.
- **Quality gates (all run, all green).**
  - `cargo fmt` — applied.
  - `cargo clippy --all-targets -- -D warnings` — clean.
  - `cargo test` — **44 passed, 0 failed**.
  - `bun run typecheck` — clean.
  - `bun run lint` — **0 errors**, 11 pre-existing warnings (unused locals, `<img>` advisories).
  - `bun run build` — static export succeeded, 12 pages.

### Not done yet (deliberately deferred)

- Priority reorder, pause/resume and stop-current-without-cancel; queue is sequential with cancel + retry only.
- Per-series quality override (the new schema has no such column); config default is used.
- yt-dlp/ffmpeg are resolved from `<app_data_dir>/bin` then `PATH` — **not bundled sidecars yet**, and the
  in-app yt-dlp updater is not built.
- AnimeCube and YouTube sources; Explore remote search still a placeholder.
- No end-to-end download driven through the running GUI yet — the engine was verified via unit tests, a live
  page fixture, and a real `yt-dlp` invocation with the generated arguments.

---

## Current session — Stitch UI pass, banners and editing

**Status: banners resolve from the legacy cache, series/library/settings are editable through popups, and episode lists use TanStack Table.**

### Done

- **Legacy artwork is resolved and cached.**
  - `tools/export_legacy.py` now exports `posterVideoId` (first episode fallback).
  - New `0002_series_banners.sql` adds `banner_path` and `banner_source`; `0001_init.sql` was not edited.
  - Startup backfills banners from `~/.cache/donghua-stream/thumbs/<video_id>.jpg` into
    `<app_data_dir>/cache/banners/<series_id>.jpg`, even when the legacy catalog import flag is already set.
  - User-chosen (`custom`) banners are never overwritten. Current catalog: **17 of 27 series have cached banners**.
  - Rust serves banners as data URLs generated in-process; no new crates, asset-protocol scope, or file dialog plugin.
- **Home hero is now a cinematic spotlight.**
  - `HeroSpotlight` uses the most recent downloaded episode plus its series details, banner, status, metadata, synopsis and CTAs.
  - Loading, empty and copyable error states are included.
- **Series detail follows the Stitch dossier structure.**
  - `SeriesDetailHero` adds ambient artwork, poster/status/score treatment, native titles, technical pills, synopsis, CTA row, health strip, save-folder line and archive sync.
  - Episode lists now render through a reusable TanStack Table (`@tanstack/react-table` v9) with row action dropdowns.
  - Downloads inherits the same table via `EpisodeList`.
- **Library is paginated and editable.**
  - Default 15 items per page; page-size dropdown offers 15/30/50/All.
  - Filters/search reset pagination.
  - Series cards expose an edit action outside the detail link; `SeriesEditSheet` edits name, description, save folder and banner source through a popup.
  - “Copy from legacy cache” re-runs banner backfill.
- **Settings is no longer directly editable.**
  - Restricted values use dropdowns; text/number fields open popup dialogs via `SettingDialog`.
  - Player, Explore and YouTube sections use validated structured Rust commands.
  - Legacy migration path is display-only with an edit dialog and re-run action.
- **Static SPA constraints preserved.**
  - No page/UI component calls `invoke` directly; IPC stays behind `lib/ipc/*` and hooks.
  - No shadcn/network component installs, no `components/ui/*` edits, no new dependencies.
- **Quality gates rerun after the full pass.**
  - `bun run typecheck` — passed.
  - `bun run lint` — passed.
  - `bun run build` — passed (12 static pages).
  - `cargo fmt` — passed.
  - `cargo clippy --all-targets -- -D warnings` — passed.
  - `cargo test` — pending final Rust-layer report.

---

## Current session — data migration and migrated-data screens

**Status: migrated catalog is live in the app; screens now read it through Rust IPC.**

### Done

- **Background dev server is running.**
  - Launched with `setsid nohup bun run tauri dev > /tmp/opencode/tsuki-tauri-dev-20261122.log 2>&1 < /dev/null &`
  - Verified `curl http://localhost:3000` → `200`.
  - Log shows `main webview finished loading` and live SQLite queries from the pages.
  - Rust rebuilds/restarts have kept `target/debug/tsuki`, `tauri dev`, and `next dev` alive.
- **Legacy data migration verified.**
  - `tools/export_legacy.py` exported the active PostgreSQL catalog to
    `~/.local/share/com.alaricsenpai.tsuki/legacy-export.json`.
  - Startup import created **27 series / 395 episodes** in `tsuki.db`.
  - Statuses: `downloaded 108`, `failed 25`, `pending 190`, `queued 72`.
  - `meta.legacy_data_imported` and `meta.legacy_data_imported_at` are set, so startup skips re-import.
  - Legacy `key` and `video_id` values are preserved so archives and on-disk media still match.
- **Rust read model extended.**
  - `SeriesSummary` now includes `failedEpisodes` and `queuedEpisodes`.
  - `LibraryStats` now includes `queuedCount`.
  - `EpisodeWithSeries` + `recent_episodes(status, limit)` provide cross-library episode lists for Home/Downloads.
  - `legacy_export_path` command exposes the default export path to Settings.
- **Frontend IPC/hooks layer added.**
  - `lib/ipc/{client,library,config,migrate}.ts` wrap `trackedInvoke`; components never call `invoke`.
  - `hooks/use-async-resource.ts` and `hooks/use-library.ts` provide typed loading/error/refresh state.
  - Static browser preview uses explicit empty fallbacks when Tauri IPC is unavailable.
- **Reusable data components added.**
  - `components/common/{data-state,stat-grid,progress-bar}.tsx`.
  - `components/features/library/{series-card,series-grid}.tsx`.
  - `components/features/series/{episode-row,episode-list}.tsx`.
  - `components/features/home/home-sections.tsx`.
  - `components/features/settings/settings-panel.tsx`.
- **Screens wired to migrated data.**
  - Home: real stats, recent downloads, and library posters.
  - Library: searchable/filterable real series grid plus archive sync.
  - Series: query-param detail (`/series?id=…`), real episode list with status filters.
  - Downloads: real queued/failed/downloaded episode lists; clearly labelled as migrated status, not a live queue.
  - Explore: local catalog search from migrated data; remote source tabs explicitly state scrapers are not wired.
  - Settings: real config editor for known settings, config-backed migration status, and re-run import.
- **Shared layout preserved.**
  - All page-level `AppShell` imports removed; only `app/layout.tsx` owns the shell.
- **Quality gates rerun after the changes.**
  - `bun run typecheck` — passed.
  - `bun run lint` — passed.
  - `bun run build` — passed; static export produced 10 routes.
  - `cargo fmt` — passed.
  - `cargo clippy --all-targets -- -D warnings` — passed.
  - `cargo test` intentionally deferred per instruction (“skip unit tests to be done at the end”).

---

## Phase 0 — Foundations

**Status: UI shell/design system and core Rust config/DB foundations complete. Remaining scaffold items are listed below.**

### Done

- Rename to Tsuki (`productName`, bundle id, npm/Cargo package names, window title).
- Stitch tokens extracted into `app/globals.css` and `docs/design/tokens.md`.
- Fonts + metadata in `app/layout.tsx`.
- `AppShell`, `Sidebar`, `TopBar`, and shared common components.
- Static SPA config (`output: 'export'`, unoptimized images); all routes prerender.
- Config layer (`error.rs`, `config.rs`, `state.rs`, config commands) with atomic private writes and legacy config import.

### Not done yet

- Remaining `AGENTS.md` §3 dependencies/plugins: `tauri-specta`, dialog/fs/clipboard/notification/window-state/single-instance, reqwest/scraper, etc.
- Real CSP and capabilities for every plugin; `externalBin` sidecars for `yt-dlp` / `ffmpeg`.
- Generated typed IPC bindings from Rust (current types are hand-maintained in `lib/ipc/*`).
- CI additions for the quality gates.

---

## Phase 0b — Player spike

**Status: not started.**

Needs a decision recorded in `docs/decisions/player.md`: embedded engine that plays local MKV/MP4 and remote HLS with custom headers, seeks cleanly, works fullscreen on Wayland and X11.

---

## Phase 1 — Data

**Status: core complete for the migrated catalog.**

- SQLite via `sqlx` at `<app_data_dir>/tsuki.db`; WAL, foreign keys, busy timeout, migrations in `src-tauri/migrations/`.
- `db/models.rs`, `db/queries.rs`, `media_ids.rs`, archive repair/sync, and library commands.
- Staged legacy import implemented and verified; no PostgreSQL client in the shipped app.
- Library and Home read real migrated data through hooks → `lib/ipc/*` → commands.

---

## Phase 2 — Downloader (in progress)

DonghuaStream downloads work end to end in Rust: resolve → yt-dlp → retry with backoff → verified merged file →
archive + DB status, streamed to the UI over `download://state` / `download://console`. See the current-session
entry above for what is deferred (priority/pause, sidecar bundling, updater).

## Phases 3–5 — Not started

Remote source scrapers/Explore search (AnimeCube, YouTube), player engine/adapters, notifications/packaging polish.
Explore/Show/Watch screens still show honest placeholders where those engines do not exist yet.

---

## Known issues

- `node_modules/next/dist/docs` is absent in this install, so the injected Next.js “read the relevant guide” instruction could not be satisfied literally; static SPA rules from `AGENTS.md` were followed instead.
- Stitch screenshot parity has **not** been re-checked after wiring; only the extracted HTML structure and token system were used.
- Relaunching `bun run tauri dev` after Rust changes is required; the dev process had exited and was restarted from `dstauri/`.
- Downloads now runs a real yt-dlp queue for DonghuaStream. Remote scrapers (AnimeCube/YouTube) and the media
  player are still not wired.
- yt-dlp and ffmpeg must currently be installed on `PATH` (or dropped into `<app_data_dir>/bin`); nothing is
  bundled yet, so a fresh machine without them gets a friendly "yt-dlp was not found" error.
- The download queue is in-memory: jobs queued in a previous app run are not resumed, and episodes left `queued`
  in the DB from the migration are not auto-started.
- `bun run lint` still reports 11 pre-existing warnings (unused locals, `<img>` advisories); zero errors.
- Static `next dev` preview without Tauri shows empty fallbacks by design.
- A harmless Base UI console warning remains for sidebar tooltip triggers rendered as links.
- Rust unit tests exist and run (44 passing, including scrapers, quality, archive, media IDs, config).
  Frontend unit tests and e2e tests are still deferred.
- `dstauri/AGENTS.md` still has older scaffold wording in places; root `AGENTS.md` is the source of truth.

---

## Open questions

- [ ] Embedded player engine result of the spike (§8) — blocks Phase 4 UI.
- [x] Legacy data import in v1 — implemented via staged JSON; startup runs it when the flag is unset.
- [ ] Bundle `ffmpeg` (size) or require it on `PATH`?
- [ ] Target platforms for release builds (Linux only, or + Windows/macOS)?

---

## Repo unification (2026-11)

`dstauri/` used to be a nested git repository, so GitHub never saw the Tauri app
as part of Donghua-stream. It was merged into the base repo as a subtree:

- History: `a5f5544 feat: initial commit` is now an ancestor of `main` via merge
  commit `7fd2a54 chore(repo): bring the Tauri app history into dstauri/`.
- `dstauri/.git` is gone; `git rev-parse --show-toplevel` from `dstauri/` returns
  the repo root.
- Workflows moved to the repo root: `.github/workflows/ci.yml` (fmt/clippy/test +
  typecheck/test/lint/build) and `.github/workflows/release.yml` (now sets
  `working-directory: dstauri` and `projectPath: dstauri`).
- `dstauri/AGENTS.md` is now a pointer to the root `AGENTS.md`, which is the only
  source of truth.

### Phase 0/1 additions in the same session

- `watch_progress`: migration `0006` (adds `idx_watch_progress_updated`), queries
  (`save/get/clear/all_watch_progress`), IPC commands, `lib/ipc/watch-progress.ts`,
  and resume/save wiring in `app/watch/page.tsx` (throttled 5s writes, force write
  on pause/seek/end, auto-complete at 90%).
- CSP replaced `null` with an explicit policy (asset/ipc/loopback/media allowances).
- Added `tauri-plugin-window-state` and `tauri-plugin-single-instance` (the latter
  focuses the existing window on a second launch).
- Frontend tests: vitest + Testing Library, 3 files / 20 tests
  (`lib/utils.test.ts`, `lib/ipc/client.test.ts`, `lib/ipc/watch-progress.test.ts`).
- `docs/decisions/player.md` records the accepted player engine decision.

---

## Phase 2 — complete (this session)

| Item | Status |
|---|---|
| yt-dlp driver, retry budget, archive, events, queue, per-series overrides | already done |
| Bundled `yt-dlp` + `ffmpeg` sidecars (`externalBin`, `scripts/fetch-sidecars.sh`) | done |
| Pause / resume (queue-wide + per job, survives restart via `not_started`) | done |
| Priority reorder (`move_download_in_queue`, arrow controls) | done |
| Disk-usage scan (`scan_disk_usage`, per-series sizes, updater button) | done |
| yt-dlp updater (`update_ytdlp`, stores version in config) | done |
| Split `app/downloads/page.tsx` (448 → 256 lines, composed sections) | done |
| Typed IPC bindings → **ts-rs** (tauri-specta is still 2.0.0-rc) | done |

Rust: 78 unit tests pass; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean.
Frontend: `tsc --noEmit`, `bun run lint` (0 errors, 4 pre-existing `<img>` warnings) and `bun run build` (static export) clean.

`ts-rs` exports into `lib/ipc/bindings/` during `cargo test`. `WatchProgress`,
`SeriesUsage` and `DiskUsage` wrappers consume the generated types.
`DownloadSnapshot.state` stays a hand-written literal union and `AppError` has a
custom `Serialize` impl — both are intentional exceptions documented in
`lib/ipc/bindings/index.ts`.

### Phase 3 next
YouTube client (search + disk cache, no API call on Explore mount) → AnimeCube
scraper (RSC payload + decryption, fixture tests) → `/show` route → Add Series
wizard (needs a Stitch screen decision) → source tabs → search pagination check.

---

## Phase 3 — Sources & Explore (2026-11)

| Item | Status | Notes |
|---|---|---|
| YouTube client | done | `src-tauri/src/youtube.rs`: search.list + one videos.list, disk cache with TTL, pure formatting helpers, friendly errors. No API call until the user searches. |
| YouTube UI | done | `lib/ipc/youtube.ts`, `hooks/use-youtube.ts`, `components/features/explore/youtube-panel.tsx`, `source-tabs.tsx`; Explore page branches on the active source. |
| AnimeCube scraper | done (live remap) | Site changed: catalog marker `sourceKind:"admin_config"`, `animeCode` instead of `slug`, routes `/title/<code>` and `/watch/<code>`. Remapped cards/catalog/anime/episodes (`episodeGroups[].episodes[]`) + wired `explore_*` dispatch. |
| AnimeCube playback resolve | **partial** | Old AES-GCM decrypt kept, but the site no longer exposes `anime-sources-versions` / `sources` in the page. 31 JS chunks had no literal `/api/` playback path. `resolve()` will fail honestly rather than invent a URL. |
| `/show` route | done | Full details page (hero, episodes, recommendations, add-to-library) replacing the phase-3 stub. |
| Source tabs | done | Replaced the dropdown with `SourceTabs` (`role="tablist"`). |
| Search pagination | verified | `catalog_url` emits `/page/{n}/?s=` and `has_next_page` handles `rel=next`; no change needed. |
| Add Series wizard | **blocked** | `AGENTS.md` §5 lists the Stitch screen as TBD. Need a Stitch screen or an approved fallback before building it. |

Gates: `bun run typecheck` clean, `bun run lint` 0 errors (5 pre-existing `<img>` warnings), `bun run build` static export clean. Rust `cargo fmt`/`cargo check` clean; `cargo test --lib` result pending a background run at the time of writing.

Follow-up hotfix (pushed `98dbefc`): the ISO-8601 duration parser spun forever on the `T` separator; 96 tests now finish in ~0.34 s.

---

## Phase 4 — Player (2026-11)

| Item | Status | Notes |
|---|---|---|
| PlayerEngine contract | done | `lib/player/engine.ts`: `PlayableSource`, events, `PlayerStatus`, `EnginePreference`, `PlayerEngineUnavailable`, `formatTime`. |
| Adapters | done | `lib/player/adapters/{html-video,external,mpv}.ts`; `createEngine()` in `lib/player/index.ts` honours `player.engine` and falls back from unavailable mpv to the external adapter. |
| Shared components | done | `components/player/{video-surface,player-controls,mini-player}.tsx`: seek bar, speed, audio/subtitle menus, fullscreen, mini-player action. |
| Local playback | done | `/watch` uses `VideoSurface` with the configured engine, watch-progress resume, throttled saves and 90% auto-watched. |
| Keyboard shortcuts | done | space/k play-pause, ←/→ 10 s, j/l 60 s, f fullscreen, m mute (ignored while typing). |
| Mini player | done | Zustand store + global mount in `app/layout.tsx`; opens from both `/watch` and `/stream`. |
| Remote playback | done (per decision) | `/stream` keeps `PlayerSurface` (react-player) for host compatibility; see `docs/decisions/player.md`. |
| Header-bearing HLS proxy / capability probe | deferred | Explicitly documented in `docs/decisions/player.md`; no source currently needs custom headers. |

Gates: `bun run typecheck` clean, `bun run lint` 0 errors (8 warnings: 5 pre-existing `<img>`, 3 `exhaustive-deps` in `video-surface.tsx`), `bun run build` static export clean.

---

## Phase 5 — Settings, packaging and polish (2026-11)

| Item | Status | Notes |
|---|---|---|
| GitHub repo rename | done | `Donghua-stream` → `tsuki-app`; local `origin` now `https://github.com/Alaric-senpai/tsuki-app.git`. |
| YouTube opt-in | done | `YouTubeConfig.enabled` (default false) gates both the Explore source and the IPC client; a friendly config error is shown when disabled/no key. |
| Background downloads opt-in | done | `settings.background_downloads` (default `0`) + `set_background_downloads`; completion notifications are suppressed unless enabled. |
| App updater | done | `app_updater.rs` checks GitHub releases, picks the matching AppImage/.deb/.rpm asset (Fedora x86_64/aarch64 included), downloads to `<app_data>/updates/` and opens the installer; Settings card exposes Check + Download & install. |
| About screen | done | Live version via `getVersion()`, App/Runtime/Developer info cards, developer section with current priorities, repo link and the carried-forward disclaimer. |
| Clippy cleanup | done | Collapsed-if warnings in `youtube.rs` fixed (clippy 1.98); `cargo clippy --all-targets -- -D warnings` clean. |
| Packaging verification | pending | `release.yml` builds `appimage,deb,rpm`; local build not run here (no desktop packaging toolchain verified). |

Gates: `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test --lib` 99 passed in 1.7 s; `bun run typecheck` clean, `bun run lint` 0 errors (9 warnings: 5 `<img>`, 4 `exhaustive-deps`), `bun run build` static export clean.

Release decision: v1.1.1 (option A) — the remote v1.1.0 tag at legacy commit f900406 is preserved; release.yml now expects v1.1.1.

### Pending for 1.1.2

- Streamplay.co.in embeds are blocked in 1.1.1 with an actionable message while a resolver is developed; yt-dlp cannot extract them and the metadata endpoint returns an empty body. Other sources (DonghuaStream, animekhor, Dailymotion, opt-in YouTube) work.

## v1.1.1 release process

| Item | Status | Notes |
|---|---|---|
| Version surfaces | done | `1.1.1` in `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, `package.json`, sidebar/About UI, and `release.yml`. |
| Zero-warning frontend gates | done | `<Image unoptimized>` + remote patterns replaced all `<img>` warnings; `usePlayer` values stabilized; `bun run lint` has 0 warnings. |
| DonghuaStream pagination 404 | done | `/anime/pagg/{n}/` → `/anime/page/{n}/`, covered by a regression test. |
| Downloader diagnostics | done | Protocol-relative media URLs are absolutized; real yt-dlp `ERROR:` lines surface as the user-facing message; duplicate enqueues are rejected. |
| Mini-player playback error | done | `NotSupportedError` is caught, emitted as an engine error, and toasted instead of producing an unhandled rejection. |
| AnimeCube disconnect | done | Source card marked unavailable; resolver fails with a friendly error instead of handing raw URLs to yt-dlp. |
| animekhor.org source | done | Catalog/details/search, episode parsing, Dailymotion playback resolution, `ak-*` media ids, Explore + settings wiring, fixture tests. |
| Bulk download actions | done | `cancel_all_downloads` and `unqueue_downloads` commands + IPC + UI buttons; queued DB rows reset to `not_started`. |
| READMEs | done | Root README rewritten for Tsuki v1.1.1; legacy README preserved under `docs/legacy/README.md`. |
| Local font bundling | done | Geist / Plus Jakarta Sans / JetBrains Mono woff2 files bundled with OFL licenses; no Google Fonts requests at runtime. |
| Streamplay | shelved | Blocked with an actionable "pending development" error in 1.1.1; resolver planned for 1.1.2. |
| Local packaging build | deferred to CI | Fedora host was missing `rpmbuild` and `dpkg-deb`; per user direction, CI builds the release artifacts and local verification is deferred. Windows NSIS and macOS DMG cannot be cross-built on Linux. Tag pushes do not trigger `ci.yml`; `release.yml` handles tagged releases. |

Tag: `v1.1.1` (remote `v1.1.0` on the legacy commit is preserved).
