# Plan: Phase 2 (Downloader) and Phase 3 (Sources & Explore)

Written after the Phase 0/1 audit. Every item below was verified against the
code in this tree, not inferred from `AGENTS.md`.

## Phase 0/1 status: complete except one item

Everything in `AGENTS.md` §12 Phase 0 and Phase 1 is in place — scaffold, static
export, Stitch tokens, `AppShell`/`Sidebar`/top bar, explicit CSP, capabilities,
SQLite + migrations `0001`–`0006`, legacy JSON import, config with atomic writes,
watch progress (queries + IPC + `/watch` resume), vitest harness, CI gates,
`docs/decisions/player.md`.

**Still open: `tauri-specta` typed bindings.** There is no `specta` dependency and
no generated `types/`. Types are hand-mirrored in `lib/ipc/*.ts`.

Do it **first in Phase 2**, not last: Phase 3 adds a large number of new payload
types (AnimeCube series/sources, YouTube videos, wizard steps). Deriving them
from day one is cheaper than retrofitting them afterwards.

---

## Phase 2 — Downloader

### What already works

`downloader/ytdlp.rs` (process driver, 10 transport retries, 30 s socket timeout,
file-size verification), `downloader/queue.rs` (concurrent FIFO, default 3,
`enqueue`/`cancel`/`remove`/`clear_finished`/`set_concurrency`, per-video retry
budget, retry noise to console only), `downloader/archive.rs` (`downloaded.txt`),
`downloader/quality.rs` (presets 360p–1080p + best, containers, merge policy),
events (`download://progress`, `download://console`), desktop notifications,
startup resume of episodes left `queued`, per-series folder + quality override,
browser cookies, bandwidth limit, subtitle languages, Downloads page with queue
items, telemetry, console drawer and per-episode retry.

### What is missing

| # | Gap | Evidence |
|---|---|---|
| 1 | **Sidecar bundling.** `yt-dlp`/`ffmpeg` are not shipped. No `externalBin` in `tauri.conf.json`, no `src-tauri/binaries/`. `YtDlp::locate` looks in `<app_data>/bin` then `PATH`. | `grep externalBin` → nothing |
| 2 | **Pause / resume.** No pause anywhere: not in `queue.rs`, not in `commands/downloads.rs`, not in the Downloads UI. `resume_downloads` only re-enqueues rows left `queued` in the DB at startup. | `grep -rn pause src-tauri/src` → no hits |
| 3 | **Priority reorder.** Queue is strictly FIFO with no ordering field or move command. | `queue.rs` has no `reorder`/`priority` |
| 4 | **Disk-usage scan.** No command exists at all, although §2 requires "disk-usage scan". | `grep disk_usage` → no hits |
| 5 | **yt-dlp updater.** No `update_ytdlp`. `YtDlp::locate` already prefers `<app_data>/bin/yt-dlp`, so the override hook exists but nothing writes to it. | no updater module |
| 6 | **Page size debt.** `app/downloads/page.tsx` is 413 lines (rule: ~150, no inline business logic). | `wc -l` |

### Implementation plan (in order)

1. **tauri-specta** (closes the last Phase 0 item)
   - Add `specta` + `tauri-specta` to `Cargo.toml`; derive `specta::Type` on every
     command payload/return type (`db/models.rs`, `config.rs`, `downloader/`,
     `sources/`, `error.rs`, `commands/*`).
   - Add a `bindings` module behind a `--features bindings` (or a small
     `cargo run --bin export-bindings`) that writes `dstauri/lib/ipc/bindings.ts`.
   - Replace hand-written interfaces in `lib/ipc/*.ts` with imports from the
     generated file; keep the wrapper functions (they add fallbacks and error
     mapping) but take their types from the binding.
   - Gate: `tsc --noEmit` must pass with zero `any`.

2. **Bundle the sidecars** (answers the open decision "bundle ffmpeg or PATH?")
   - Decision: **bundle both.** §1 says self-contained; a fresh machine without
     `yt-dlp`/`ffmpeg` currently cannot download anything.
   - `tauri.conf.json`: `bundle.externalBin = ["binaries/yt-dlp", "binaries/ffmpeg"]`.
   - `scripts/fetch-sidecars.sh` downloads the pinned release for the host target
     triple into `src-tauri/binaries/<name>-<triple>` (gitignored).
   - `YtDlp::locate` order becomes: app-data override → bundled resource dir →
     `PATH`. Same for ffmpeg; pass `--ffmpeg-location` to yt-dlp so the merge
     postprocessor cannot silently fall back to a missing binary.
   - First-run check surfaces a friendly, copyable error if a binary is missing
     or not executable.
   - `THIRD_PARTY.md` entry for yt-dlp (Unlicense) and ffmpeg (LGPL/GPL build).

3. **Pause / resume**
   - Queue actor gets a `paused: AtomicBool`. While paused it keeps the running
     job (do not kill it) and refuses to start new ones; `pause_all`/`resume_all`
     commands + `download://state` event so the UI reflects it.
   - Per-job pause: `cancel` the process with a flag that marks the row
     `paused` instead of `cancelled`; on resume re-enqueue the same episode and
     let yt-dlp continue from the `.part` files (`--continue` is already on).
   - Download page: pause/resume button per active item and a queue-wide toggle.

4. **Priority reorder**
   - Give queued jobs an explicit `position` (an `AtomicU64` counter is enough
     for FIFO; reorder rewrites the pending vector).
   - `move_in_queue(episode_id, direction)` command + arrow buttons on
     `DownloadQueueItem` (disabled for running jobs), emitting a fresh snapshot.

5. **Disk-usage scan**
   - `scan_disk_usage()` in Rust: walk each series `save_dir`, sum file sizes,
     report per-series totals + the free space of the containing filesystem.
   - Show in Settings (library section) and as a "N GB on disk" line in Library.
   - Pure size-formatting helper already exists in `lib/utils.ts` (`formatBytes`).

6. **yt-dlp updater**
   - `update_ytdlp()` downloads the latest release binary into `<app_data>/bin`,
     chmod 0755, and records the version in config meta. Refuse to swap while a
     download is running. Settings row: "Check for yt-dlp updates".

7. **Split `app/downloads/page.tsx`** into `features/downloads/` containers
   (queue list, status table, actions) so the page composes only.

**Exit criteria:** fresh machine with no yt-dlp/ffmpeg downloads, merges and plays
an episode; pause/resume survives an app restart; reorder is reflected in the
running order; Settings shows disk usage; all gates green; commit + push.

---

## Phase 3 — Sources & Explore

### What already works

DonghuaStream: catalog with pagination (`/anime/?status=&type=&order=` and
`/anime/pagg/{n}/`), search (`/?s=`, `/page/{n}/?s=`), filters (genres, status,
kind, order), series details (episodes, recommendations, metadata), on-disk
response cache, and `import_source_series` which supports a **new** series
(`series_name`) or an **existing** one (`target_series_id`), with an optional
immediate download and per-episode warnings. Explore page: source dropdown,
filter bar, poster grid, series drawer, pagination, add-to-library dialog.
Settings already stores YouTube API key / region / language / safe search.

### What is missing

| # | Gap | Evidence |
|---|---|---|
| 1 | **AnimeCube scraper** — entirely absent. Legacy `AnimeCube` in `sources.py` (~330 lines) parses Next.js RSC payloads and *decrypts* the playback-source blob (`_decrypt_payload`, `_find_anime_object`, `_pick_source`, `versions`, `details`, `recommendations`, `resolve`). | no `sources/animecube/` |
| 2 | **YouTube client** — absent. Legacy `youtube_api.py` (495 lines): `search.list` + one `videos.list`, disk cache with TTL, `discover()`, normalization helpers. Only the config block and a disabled dropdown entry exist. | no `youtube.rs` |
| 3 | **`/show` route is a stub** — 31 lines of placeholder copy, although `explore_series` already returns a complete `SourceSeries` (metadata, episodes, recommendations). | `app/show/page.tsx` |
| 4 | **Add Series wizard** — missing; only `add-to-library-dialog.tsx`. `AGENTS.md` §5 marks the Stitch screen **TBD**. | no wizard component |
| 5 | **Source tabs** — Explore uses a dropdown; Stitch defines separate "Explore YouTube" / "Explore Donghua" screens. | `source-dropdown.tsx` |
| 6 | **Search-mode pagination unverified** — `catalog_url` emits `/page/{n}/?s=`, never tested against the live site. | `browse.rs:534` |
| 7 | `ensure_source()` rejects everything except `donghuastream`, so each new source needs its branch. | `commands/explore.rs:74` |

### Implementation plan (in order)

1. **YouTube first** — self-contained, no scraping fragility, easiest to test.
   - `src-tauri/src/youtube.rs`: `YouTubeClient` with `search()` (one
     `search.list` + one `videos.list`), `discover()` for trending, ISO-8601
     duration parsing, view/age formatting, `best_thumbnail`.
   - Disk cache keyed by endpoint + params, TTL from `config.youtube.cache_minutes`
     (default 20), stored under `<app_data>/cache/youtube/`.
   - Errors: `YouTubeNotConfigured` when the key is empty, quota/403 mapped to a
     friendly message that names the key setting.
   - **Nothing calls the API until the user submits a search** — no prefetch on
     Explore mount (legacy behaviour, test it).
   - Commands: `youtube_search`, `youtube_discover`, `youtube_video`.
   - Tests: duration/views/age formatting, cache hit/miss, error mapping.

2. **AnimeCube**
   - `src-tauri/src/sources/animecube/{mod.rs,browse.rs,payload.rs}`.
   - `payload.rs`: RSC text extraction, `json_after`, anime object lookup,
     `decrypt_payload`, source picking — pure functions with fixtures captured
     from real responses, unit-tested exactly like the legacy tests.
   - `browse.rs`: `search`, `catalog`, `details` (episodes + recommendations +
     version/tab selection), `resolve` → `ResolvedMedia`.
   - Wire `ensure_source("animecube")`, flip `available: true`, and let the
     existing Explore UI work unchanged.
   - **Risk:** the decryption is site-specific and may already be stale. Validate
     against the live site before building UI on it; if it is broken, ship the
     source as `available: false` with an honest note rather than a half-working
     scraper.

3. **`/show` page** (DonghuaStream + AnimeCube, same component)
   - `features/show/`: hero (poster, title, metadata chips, genres), synopsis,
     episode list with select-all / latest / none, recommendations grid.
   - Actions: "Add to library" (new or existing series) and "Add and download",
     both through `import_source_series`; toast + link to the series page.

4. **Add Series wizard (3 steps)** — **needs a design decision first.**
   `AGENTS.md` lists the Stitch screen as TBD. Either pull a new Stitch screen or
   approve composing the wizard from the existing Series Details + Library
   patterns. Steps: (1) source + search/pick, (2) episode selection + folder +
   quality, (3) review + add/download. Build `common/stepper.tsx` +
   `common/wizard-shell.tsx` so it is reusable.

5. **Source tabs** — decide dropdown vs tabs. Recommendation: tabs, to match the
   two Stitch Explore screens; it is a presentational refactor of
   `app/explore/page.tsx` + `source-dropdown.tsx` into `SourceTabs`, with the
   per-source state kept in the URL (`?source=`).

6. **Verify search pagination** against the live site; add a fixture and a test
   for `catalog_url` + `has_next_page` in search mode.

**Exit criteria:** Explore searches and browses all three sources, `/show` is
fully wired, the wizard adds a series to the library, and nothing calls the
YouTube API before a search is submitted. All gates green; commit + push.

---

## Working agreement

- Commit and push **after every phase** (`AGENTS.md` §11 style, one concern per
  commit).
- Update this file and `docs/PROGRESS.md` at the end of each phase.
- Anything that turns out to be blocked (Stitch design, live-site decryption,
  missing API key) gets recorded here rather than worked around silently.
