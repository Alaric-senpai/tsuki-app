# Design: Unified Source Registry

**Status:** proposed — design for a **future release (post-1.1.3)**. Nothing in this document ships in 1.1.3.
**Branch context:** `1.1.3` (research/planning only; no source, workflow, version or changelog changes).
**Authority:** this document proposes an *additive* layer. It does not amend `AGENTS.md`; when implemented, the
relevant decisions must be recorded in `AGENTS.md` §5/§6/§12 and `docs/PROGRESS.md` in the same change set.

---

## 1. Problem and goal

Today every source is a separate catalog. `commands/explore.rs` exposes a source dropdown from a hand-written
`source_infos()` list, gates every command through `ensure_source()`, and dispatches per source with an
`if/else` chain. Each source module (`sources/donghuastream/`, `sources/animekhor/`, `sources/animecube/`)
returns the shared shapes in `sources/browse.rs` (`SourceCard`, `SourceSeries`, `SourceEpisode`,
`CatalogPage`) but owns its own identity, titles, episode numbering, aliases and cache namespace.
The library stores exactly one `series.source_key` / `series.source_url` and one page URL per episode.

The shipped source set at the time of writing is **donghuastream, animekhor, animecube** plus the **six sources
added in 1.1.3**. With 9+ sources, a user must search each tab, recognise that "Big Brother", "My Senior Brother
Is Too Steady" and "MSBTS" are one show, and manually reconcile episode numbers. There is no single "what's new",
no "everything I follow in one place", and no way to see that one episode is available from several sources.

**Goal:** an **opt-in** registry that aggregates all sources into a single personalized registry that *looks* like
one catalog, while keeping every source-specific view intact and never treating any source as the source of truth.

### Invariants (non-negotiable, inherited from `AGENTS.md`)

1. All I/O in Rust; the frontend only calls commands/hooks; SQLite owned by Rust (`tsuki.db`).
2. Only **stable page URLs** are persisted. Resolved media URLs expire and are never stored (§8).
3. Download semantics are untouched: explicit user action, local file is preferred for playback, archive survives
   renames, failures surface only after retries (§2).
4. Next.js stays a static SPA; every registry page is a client component; typed bindings via `ts-rs`.
5. No YouTube anywhere in the registry (§5/§14 and §15 below). No secrets, no cloud, no account.
6. No new UI design invented here: when UI lands, use the Stitch screens/tokens (§5) and shared components (§9).

---

## 2. Architecture overview

```
 per-source adapters                    registry core (pure functions)              publication
 ┌──────────────────────┐        ┌──────────────────────────────────────┐     ┌─────────────────┐
 │ donghuastream        │        │ normalize → identity → numbering     │     │ SQLite tables   │
 │ animekhor            │  ───▶  │ → merge → health/rank → feeds        │ ──▶ │ registry://*    │
 │ animecube (disabled) │        │          (no I/O, unit-testable)     │     │ events          │
 │ + six 1.1.3 sources  │        └──────────────────────────────────────┘     └─────────────────┘
 └──────────────────────┘                       ▲       ▲
          │ SourceRecord                       │       │
          └── disk cache (sources/cache.rs) ───┘       └── on-device signals
                                                          (library, watch_progress, pins)
```

New module tree (all under `src-tauri/src/`), added **without changing existing source modules**:

```
registry/
  mod.rs          # orchestration + public API; the only I/O-bearing module
  descriptors.rs  # SourceDescriptor, capabilities, trust tiers, health state machine
  normalize.rs    # pure: title/alias/number/label normalization
  identity.rs     # pure: blocking keys, pairwise score, clustering, vetoes
  numbering.rs    # pure: continuous vs season-local mapping, ranges, conflicts
  merge.rs        # pure: field selection, episode union, provenance
  ranking.rs      # pure: health score, candidate score, FYP/new-episode/calendar scores
  feeds.rs        # pure: materialize feed entries from scored inputs
  backfill.rs     # pure-ish: build initial registry rows from library rows + cached explore JSON
  types.rs        # serde/ts-rs types shared with the frontend
```

Rule: **only `registry/mod.rs` and the adapter boundary touch network, disk and SQLite.** Everything else is a
pure function taking values and returning values, so it can be tested without a webview, network or DB.

### Adapter contract (replaces the `ensure_source` if-chain for registry ingestion)

A single trait (conceptually; Rust name to be chosen at implementation time) implemented once per source:

- `key() -> SourceId`, `descriptor() -> SourceDescriptor` (label, capabilities, trust tier).
- `search(query, filters) -> Vec<SourceCard>` and `detail(id) -> SourceSeries` — the existing types in
  `sources/browse.rs` are the contract; no source-specific shape enters the registry.
- `aliases(id) -> Vec<AliasRecord>` — optional; sources without an alias API return empty.
- `health_probe() -> ProbeResult` — cheap `HEAD`/small `GET`, no auth, no user identifiers.
- `resolver_status() -> bool` — whether `sources::resolve_for_download` can resolve this source today
  (today: donghuastream/animekhor `true`, animecube `false`, streamplay blocked).

The six 1.1.3 sources are **data, not match arms**: each gets a descriptor + adapter; their concrete keys, hosts,
capabilities and alias endpoints must be filled in from the actual 1.1.3 source modules at R0 (§14). The registry
core never names a source.

---

## 3. Data structures

### 3.1 Core types (Rust, serialized camelCase, `ts-rs` exports into `lib/ipc/bindings/`)

```
SourceId            = newtype String (stable lowercase key, e.g. "donghuastream")
CanonicalId         = newtype String (opaque ULID, generated once, never derived from a title)
EpisodeKey          = { canonical_id, season: Option<u32>, continuous: i64, part: Option<u32> }
SourceRef           = { source: SourceId, source_series_id: String, source_url: String }

SourceCapabilities  = { browse, search, detail, episode_list, resolve_media,
                        aliases, dates, thumbnails, genres, requires_auth, region_locked }
TrustTier           = Verified | Community | Experimental | Disabled
SourceHealth        = { state: Healthy|Degraded|Unavailable|Unknown,
                        success_rate_24h, resolver_success_rate, last_success_at,
                        last_failure_at, consecutive_failures, median_latency_ms,
                        probed_at }

AliasRecord         = { value, normalized, kind: Primary|Alias|Romanization|Abbreviation,
                        source: SourceId, confidence, first_seen, last_seen }
FieldValue<T>       = { value: T, source: SourceId, confidence, observed_at }
MergedMetadata      = { title, description, status, kind, studio, network, released,
                        duration, genres: Vec<String>,
                        provenance: per-field FieldValue metadata }   // never a bare string

NumberMapping       = { source, raw_label, season: Option<u32>, continuous: i64,
                        method: Bracket|Label|SiteApi|Inferred|UserAsserted,
                        confidence }
EpisodeCandidate    = { source, source_episode_id, page_url, title, number_text,
                        aired_at: Option<String>, confidence, verified_resolver,
                        captured_at }
CanonicalEpisode    = { key: EpisodeKey, candidates: Vec<EpisodeCandidate>,
                        preferred_candidate: Option<(SourceId, source_episode_id)>,
                        conflicts: Vec<NumberConflict>, updated_at }
CanonicalSeries     = { id: CanonicalId, display_title: FieldValue<String>,
                        aliases: Vec<AliasRecord>,
                        links: Vec<SourceLink>, episodes: Vec<CanonicalEpisode>,
                        metadata: MergedMetadata, poster: FieldValue<String>,
                        pinned_source: Option<SourceId>,
                        created_at, updated_at, merge_version }
SourceLink          = { source, source_series_id, source_url, confidence,
                        last_seen, status, primary_numbering }
RegistryEntry       = CanonicalSeries + derived: best candidate per episode,
                        health badges, freshness badges   // the UI-facing shape

IdentityLink        = { canonical_id, other_canonical_id, score, signals,
                        decision: AutoMerged|Review|UserMerged|UserSplit,
                        decided_by: Rust|User, decided_at }
ReviewItem          = { left, right, score, reason, suggested_canonical_id }
FeedItem            = { kind: Fyp|NewEpisode|Calendar, canonical_id,
                        episode_key: Option<EpisodeKey>, score, reason,
                        computed_at, confidence }
CalendarEntry       = { canonical_id, expected_date, window_days,
                        basis: Inferred|Announced, confidence, observations }
```

`RegistryEntry`, `FeedItem`, `CalendarEntry`, `ReviewItem` and `SourceHealth` derive `TS` and are emitted during
`cargo test` (the existing `ts-rs` setup). Components consume them through `hooks/useRegistry*` →
`lib/ipc/registry.ts` → `invoke`; no page talks to Rust directly.

### 3.2 SQLite additions (new migrations only — never edit `0001`–`0006`)

`0007_registry.sql` (schema; disabled feature, tables empty):

- `registry_sources(key TEXT PK, label, capabilities TEXT JSON, trust_tier, enabled INTEGER, probe_interval_sec, last_probe_at, health TEXT JSON)`
- `registry_entries(canonical_id TEXT PK, display_title, title_key, fingerprint, status, metadata TEXT JSON, poster TEXT, pinned_source TEXT, created_at, updated_at, merge_version INTEGER)`
- `registry_aliases(canonical_id, source, value, normalized, kind, confidence, first_seen, last_seen, PRIMARY KEY(canonical_id, source, normalized))`
- `registry_source_links(canonical_id, source, source_series_id, source_url, confidence, last_seen, status, numbering, PRIMARY KEY(canonical_id, source, source_series_id))`
- `registry_episodes(key TEXT PK, canonical_id, season INTEGER, continuous INTEGER, part INTEGER, created_at, updated_at)`
- `registry_episode_candidates(key, source, source_episode_id, page_url, title, number_text, aired_at, confidence, verified_resolver, captured_at, PRIMARY KEY(key, source, source_episode_id))`
- `registry_number_mappings(canonical_id, source, raw_label, season, continuous, method, confidence, UNIQUE(canonical_id, source, raw_label))`
- `registry_identity_links(a TEXT, b TEXT, score REAL, signals TEXT, decision, decided_by, decided_at, PRIMARY KEY(a, b))`
- `registry_health_events(source, observed_at, kind, detail)` — rolling observations used to compute health
- `registry_feed_items(kind, canonical_id, episode_key, score REAL, reason TEXT, computed_at, confidence, PRIMARY KEY(kind, canonical_id, episode_key))`
- `registry_state(id INTEGER PK CHECK(id=1), enabled, backfilled_at, last_full_merge_at, last_feed_at, schema_version)`

Indexes: `registry_entries(title_key)`, `registry_aliases(normalized)`, `registry_source_links(source, source_series_id)`,
`registry_source_links(source_url)`, `registry_episode_candidates(page_url)`, `registry_feed_items(kind, score DESC)`.
FTS5 over title/aliases is optional at R2; the pure in-memory search path is the tested default and must stay.

**Settings stay in the JSON config**, not SQLite (`AGENTS.md` §7): add a `registry` section to `Config` and bump
`CONFIG_VERSION` from 3 to 4 via the existing `normalize()` coercion path. DB `registry_state` holds only runtime
state (timestamps, health, computed feeds), never user preferences.

---

## 4. Canonical identity and dedup

### 4.1 Normalization (pure, deterministic, Unicode-aware)

`normalize_title(raw) -> NormalizedTitle { key, tokens, season: Option<u32>, numbers: Vec<i64>, flags }`

Ordered steps: Unicode NFKC → lowercase → strip HTML entities and URL components → remove noise:
quality/resolution tags (`1080p`, `720p`, `WEB-DL`), bracketed numbering (`[113]` — **retained as a continuous
number signal, removed from the title key**), `Multiple Subtitles`, `FULL`, `EP\d+`, punctuation, and trailing
season/language suffixes. Collapse whitespace; produce a sorted token multiset key so word order does not matter.
Preserve CJK tokens (do not transliterate heuristically); romanization matches only through an explicit alias
record, never through guessed transliteration.

`normalize_number(raw) -> Vec<NumberSignal>` must handle: plain `12`, `Episode 12`, `S2E14`, continuous bracket
`[113]`, ranges `539to540` / `EP14-20` / `E01–90`, and season-local labels. Ranges produce two bounds and a
`is_compilation` flag (see §8.3). **Never** treat a season-local number as continuous without a mapping.

### 4.2 Candidate generation (blocking, avoids O(n²))

For a new `SourceRecord`, generate blocking keys: (a) normalized primary title key; (b) each alias key;
(c) sorted token-prefix keys (`first 3 tokens`, `last 3 tokens`); (d) `studio|released_year` when both exist;
(e) stable page URL / `media_ids` digest for exact re-identification. Only entries sharing ≥1 key enter pairwise
scoring. A new source's catalog of ~10k items therefore compares hundreds of candidates, not all pairs.

### 4.3 Pairwise identity score

Signals in `[0,1]`, weights summing to 1:

| Signal | Weight | Rule |
|---|---|---|
| title token similarity (Jaccard + token-ordered bigram) | 0.40 | `>=0.95` near-exact, `0.70–0.95` fuzzy |
| alias / alternative-title exact key match | 0.25 | any shared alias key, or `value` equals other primary |
| continuous-number overlap with a valid mapping | 0.15 | requires `NumberMapping` on both sides |
| studio / network / release-year agreement | 0.08 | present on both sides only, else neutral (0.5) |
| poster perceptual similarity | 0.05 | optional; neutral when no image or no image crate |
| source trust agreement | 0.07 | same trust tier; disagreement is *not* a veto |

`score = Σ wᵢ sᵢ`. Decision tiers:

- `>= 0.92` → **AutoMerged** (only if hard vetoes pass).
- `0.75–0.92` → **Review** item; no automatic merge, shown in a review queue.
- `< 0.75` → distinct.

**Hard vetoes** (never auto-merge, even at score 1.0):

1. Two different `continuous` episode ranges with no compatible `NumberMapping` and no alias bridge.
2. A prior `UserSplit` link between the same two identities.
3. One side `Disabled`/unverified with fewer than 3 populated metadata fields and no shared alias.
4. Conflicting `released` years > 2 apart when both are present.
5. Any source on the blocklist (Streamplay, and animecube's broken resolver) contributing a *playback* claim —
   metadata claims from a disabled source are still allowed but never become preferred.

### 4.4 Clustering and audit

Build candidate pairs from blocking, score them, then union clusters with a disjoint-set structure. A `UserSplit`
link is a permanent edge-cut (not just a low score) so re-crawls cannot re-merge what a user separated. Every
decision writes a `registry_identity_links` row with `score`, `signals` and `decided_by`, so merges are explainable
in the UI ("matched on alias MSBTS + 41 shared episode numbers") and reversible. Ties break by `(trust_tier,
source_key, source_series_id)` — deterministic across runs and input order.

Worked example from `reports/source-fallback.md`: DHS `"My Senior Brother Is Too Steady Season 2 Episode 1 [14]"`
→ key `my senior brother is too steady`, continuous `14`; AnimeCube `"Big Brother"` with alias
`"My Senior Brother is Too Steady"` / `"MSBTS"` → alias bridge to the same cluster; episode `113` corroborated by
DHS `[113]`, AnimeCube `number=113` and Dailymotion title `"brother 113 (2)"`. Episode `1` (season-local) must
**not** be matched to AnimeCube `100`.

---

## 5. Episode numbering and list merging

- Each source's `SourceEpisode` contributes an `EpisodeCandidate` with its raw `number_text` and a
  `NumberMapping` to a canonical `continuous` number. Mapping methods, in descending confidence:
  `UserAsserted` (1.0) > `SiteApi` (0.9) > `Bracket` (0.85, DHS `[113]`) > `Label` (0.7) > `Inferred`
  (0.5, arithmetic offset from a corroborated episode).
- `season` is stored separately and defaults to `None` (continuous numbering). A season-local `S2E1` maps to
  `(season=2, continuous=14)` only when a bracket/site mapping supplies `14`.
- Union episodes by `EpisodeKey`. Multiple candidates for the same key are kept — never discarded; the UI offers
  them as source choices. Cross-source corroboration raises the key's confidence (`1 - Π(1-cᵢ)`).
- `NumberConflict` is emitted when two sources claim different continuous numbers for the same page URL, or the
  same number for clearly different page URLs. Conflicts are **surfaced**, never auto-resolved; the user can pin
  a mapping (`UserAsserted`, confidence 1.0).
- Range/compilation labels (`EP14-20 FULL`) are not single episodes: they become candidates with `is_compilation`
  and are excluded from episode-key union and from download suggestions (matching the report's duration guard).
- `episode_count` displayed in the UI is the number of canonical keys; each source's own count stays on its
  `SourceLink` so the UI can explain "12 of 14 episodes available here".

---

## 6. Metadata merge with confidence

Every display field is a `FieldValue<T>` with provenance. Selection rule, applied per field independently:

1. Highest `confidence` wins.
2. Ties → higher `TrustTier` → higher source `health` → more recent `observed_at` → lexicographic `SourceId`.
3. `pinned_source` overrides **display** fields only (never playback/download choice).
4. If all candidates are below `0.5`, keep the previous value and mark it `stale` rather than overwrite with a
   guess.

Field confidence formula:

```
field_confidence = 0.40 * trust_weight + 0.30 * health
                 + 0.20 * corroboration + 0.10 * recency
trust_weight: Verified 1.0 | Community 0.7 | Experimental 0.4 | Disabled 0.2
corroboration: 0 for a single source, →1 as independent sources agree
recency: 1 at capture time, decays linearly to 0 at 30 days
```

Poster selection additionally prefers landscape artwork with adequate resolution, deduplicates by perceptual hash
when available, and caches the chosen image locally through the existing banner cache path (`library.rs`
`fetch_remote_banner`) rather than hotlinking a source host. User-pinned artwork is stored as `UserAsserted` and
never overwritten.

---

## 7. Source health and capability scoring

`SourceHealth.state` is updated by a single background tokio task from `registry_health_events`:

```
health_score H = 0.40 * success_rate_24h
               + 0.30 * resolver_success_rate
               + 0.20 * freshness_of_last_success
               + 0.10 * latency_score
latency_score = clamp(1 - median_latency_ms / 2000, 0, 1)
freshness_of_last_success = 1 at probe time → 0 at 7 days

Healthy   : H >= 0.80 and consecutive_failures < 3
Degraded  : 0.50 <= H < 0.80  (or one resolver failure)
Unavailable: H < 0.50 or >= 3 consecutive hard failures
Unknown   : no observation yet — never ranked above Degraded, never blocked
```

Hard failures (DNS/TLS/HTTP 5xx/parser shape change) increment `consecutive_failures`; a soft miss (empty search
result) does not. Probes use exponential backoff with jitter (1 min → 30 min cap) and `network::ensure_online()`;
while offline, health is frozen and rows are marked `unknown/offline`, never failed.

**Capability gating:** `SourceCapabilities` decides what a source may contribute. A source without `dates` cannot
feed the calendar; without `episode_list` cannot feed new-episode detection; without `resolve_media` cannot offer
download/stream actions. This is how animecube can contribute metadata today while being unable to provide
playback — the same principle applies to any of the six new sources with partial adapters.

---

## 8. Ranking and personalized feeds

All signals come from **on-device data only**: `series`, `episodes`, `watch_progress`, download history, explicit
user pins/follows, and per-source browse counts. No account, no upload, no third-party analytics.

### 8.1 Preferred episode candidate

```
C = 0.30 * health + 0.25 * trust_weight + 0.20 * verified_resolver
  + 0.15 * number_mapping_confidence + 0.10 * freshness
  - 0.20 if no local file here but another candidate has one
  - 0.30 if requires_auth/region_locked and not configured
  - 0.50 if resolver is blocked (Streamplay, broken animecube resolver)
```

`preferred_candidate` is **derived, recomputed each publication**, never frozen into a row — so a source coming
back online automatically becomes eligible again. A locally downloaded file wins whenever one exists
(`AGENTS.md` §8: prefer the local file); a `player.prefer_local` toggle, if added later, may only make that
preference explicit — it must not invert it.

### 8.2 FYP ("For you") score per canonical series

```
F = 0.30 * genre_affinity       (genres of completed/watched series)
  + 0.20 * studio_affinity
  + 0.15 * pinned/followed boost
  + 0.15 * completion_affinity  (watch_progress similarity of prior series)
  + 0.10 * latest_episode_freshness
  + 0.10 * health of best candidate
  - 0.20 if the only candidates are from Experimental/unverified sources
```

Exclusions: series already in the library with no unwatched episodes; series with no playable candidate;
`personalization` opt-in off (then FYP is replaced by a deterministic "Trending" list from source health +
episode freshness, no user signals). Diversity caps: at most 2 items per source and 3 per genre in the top 20;
tie-break by `canonical_id` for stable ordering.

### 8.3 New-episode feed

Entries: canonical episodes with `aired_at` (or first `captured_at` when the source has no dates) inside the
configured window, where the series is followed/pinned or in the library, and the episode is not watched.
`score = recency_decay(7d) * health * trust * (1 + followed_boost)`, deduplicated by `EpisodeKey` — one row even
when three sources carry the episode. Flat `is_compilation` candidates are excluded. Entries with only a
`captured_at` estimate are labelled "detected" rather than "aired".

### 8.4 Release calendar

Infer cadence from `aired_at` gaps: require **>= 3 observations**; take the median interval and classify as
`weekly` (5–9 days), `biweekly`, `monthly` (25–35 days) or `irregular`. Produce `CalendarEntry { expected_date,
window_days = clamp(interval/4, 1, 7), basis: Inferred|Announced, confidence }` for the next 30 days. Never
fabricate a date from fewer than 3 observations; show "expected" with a confidence badge, never "confirmed".
Announced dates from a source's own schedule field override inference and carry `basis: Announced`.

Confidence badges for all feeds: `High >= 0.85`, `Medium 0.65–0.85`, `Low < 0.65`, with a provenance tooltip
("from AnimeKhor, 2 days ago").

---

## 9. Freshness and caching

Reuse `sources/cache.rs` (SHA-1 keyed JSON files, atomic writes, cache failures never break Explore) with new
namespaces:

| Namespace | TTL | Notes |
|---|---|---|
| `registry-search` | `explore.cache_minutes` (default 20) | same knob as Explore |
| `registry-detail` | 6 h | episode lists + aliases |
| `registry-aliases` | 24 h | sources with alias APIs |
| `registry-health` | 5 min | probe result cache |
| `registry-feed` | 30 min | materialized feed recomputation |
| `registry-calendar` | 12 h | cadence is slow-moving |

`refresh: true` bypasses the cache (existing command contract). Extend cached envelopes with `etag` /
`last_modified` when the source offers them; keep the expiry check on file mtime as today. Freshness tiers:
`fresh` (within TTL) / `stale` (TTL–7 d, usable with "last checked" badge) / `expired` (> 7 d, excluded from
feeds and candidate preference but still visible in library and series detail). A single bounded tokio task
(max concurrency 3, per-source rate limit) coalesces refetches; every fetch is guarded by `network::ensure_online()`;
exponential backoff on failure. No network call happens when `registry.enabled == false`.

---

## 10. Opt-in settings and privacy boundaries

New `registry` block in `config.json` (coerced by `normalize()` like `youtube`/`explore`/`player`):

```
enabled                    = false        # master opt-in; false ⇒ byte-for-byte current behavior
enabled_sources            = []           # empty ⇒ all verified shipped sources; explicit list once edited
mode                       = "preview"    # preview | unified  (see rollout R2/R4)
auto_merge                 = true
allow_cross_source_episodes= true
personalization            = false        # FYP requires this; calendar/new-episodes do not
telemetry_upload           = false        # reserved, always false in this release; no UI to turn on
feed_refresh_minutes       = 30
calendar_window_days       = 30
max_feed_items             = 40
confidence.auto_merge      = 0.92
confidence.review          = 0.75
```

Privacy boundaries:

- **Local-only.** No account, no login, no cloud sync, no cross-device identity, no analytics/telemetry upload.
  `telemetry_upload` exists only so future config round-trips do not break; it is hard-rejected if true in this
  release.
- All personalization inputs stay in the existing SQLite file and config; they are never sent to a source. Probes
  and scrapes use the existing desktop UA only (no user ID, no cookies beyond the existing optional yt-dlp
  browser-cookie import, which the registry never reads).
- Turning any source off excludes its rows from merge **and** feeds immediately; existing library entries and
  downloads are untouched.
- Unverified `Experimental` sources can appear in catalogs but get no FYP boost, no download affordance and no
  calendar authority until they have a green health probe and a passing resolver smoke test.
- Errors remain friendly and copyable (`AppError { code, message, detail }`); no source key, URL or secret appears
  in a user-facing message without the user choosing "Copy details".

---

## 11. Migration path from the source-specific catalog

**M0 — schema only.** Add `0007_registry.sql` and config `registry` block; feature off; no queries run; no network.
Zero behavior change, independently revertible.

**M1 — local backfill, no network.** On first enable, build canonical entries from `series` + `episodes` +
`watch_progress` + already-cached Explore JSON:

- one `CanonicalSeries` per library row, seeded with `source_key`/`source_url` as its only `SourceLink`;
- alias from `series.name` (`Primary`), no guessed aliases;
- episodes from `episodes` rows, `NumberMapping` from `episode_number` (`method: Label`) and from titles when a
  bracket number is present (`method: Bracket`);
- `registry_state.backfilled_at` stamped; config `meta`-style timestamp written via the existing config store.

Library routes keep reading the original tables; the registry is a shadow copy. Backfill is deterministic and
re-runnable (upsert by `source_url`).

**M2 — preview.** `mode = preview` adds an "All sources" view that reads registry tables only, with source badges
and provenance. "Add to library" routes through the existing `import_source_series` command with the candidate's
`source` + `id` — no new download code path. Review queue from `ReviewItem` available; `UserSplit`/`UserMerged`
decisions stored and honoured on re-crawl.

**M3 — link, don't replace.** Add nullable `series.canonical_id` (new migration `0008`) so a library row can point
at a canonical entry. **Never overwrite** `source_key` or `source_url`: they remain authoritative for update
checks, re-sync and downloads. Library gains an optional "grouped by show" view; per-source tabs stay.

**M4 — unified default for new installs.** Existing installs keep their setting and get a one-time prompt;
fresh installs default `enabled = true, mode = unified` in a later release, never in the release that introduces
the code. Rollback is `enabled = false` — an instant UI flip with no destructive migration. Registry tables can be
dropped without touching library tables.

Optional `export_registry_json` / `import_registry_json` commands support backup and fixture-based tests; they
never contact the network.

---

## 12. IPC and UI integration points

- Commands (thin handlers in `commands/registry.rs`): `registry_status`, `registry_list`, `registry_get`,
  `registry_search`, `registry_review_list`, `registry_decide`, `registry_feeds`, `registry_sources`,
  `registry_set_enabled_sources`, `registry_refresh`. Long work runs on tokio tasks; progress goes to
  `registry://sync` events. No SQL in handlers.
- Hooks: `useRegistryList`, `useRegistryEntry`, `useRegistryFeeds`, `useRegistryReview` (TanStack Query for server
  state; zustand only for UI-local selections). Components compose existing shared pieces: `PosterGrid`,
  `PosterCard`, `SourceTabs`, `StatusBadge`, `ErrorPanel`, `EmptyState`, `SkeletonGrid`, `ConfirmDialog`,
  `SettingsSection`, `EpisodeList`. No new design language; Stitch screenshot comparison still required (§5).
- Pages stay < 150 lines: `/unified` (catalog), `/unified/item?id=` (merged detail), `/feeds` (FYP/new/calendar
  tabs), and a Settings section for opt-ins. Query params, not dynamic segments, per the static-export rule.

---

## 13. Testability

### Pure-function modules with fixture-backed unit tests

| Module | Tests |
|---|---|
| `normalize` | NFKC, bracket/`[113]` stripping, range detection, `[^]` noise, CJK preservation, idempotence |
| `identity` | alias-bridge merge ("Big Brother" ↔ "My Senior Brother Is Too Steady"), veto on conflicting continuous ranges, `UserSplit` permanence, deterministic tie-break, score symmetry |
| `numbering` | `S2E1` vs `[14]`, `539to540`, `EP14-20 FULL`, mapping confidence order, conflict emission |
| `merge` | field selection by confidence, provenance retained, episode union never drops a candidate, order-independence |
| `ranking` | health thresholds, candidate score with blocked resolver penalty, diversity caps, calendar with <3 observations produces no entry |
| `feeds` | dedupe by `EpisodeKey`, compilation exclusion, opt-in off ⇒ no user signals used |
| `health` | consecutive-failure state machine, offline freeze, backoff bounds |

Fixtures live under `src-tauri/tests/fixtures/registry/` (no network in tests): existing
`donghuastream_series.html`, a DHS episode page with a bracket number, AnimeCube `/api/catalog` + `/api/search`
JSON showing the alias case, an AnimeKhor series page, and one minimal `source_<key>_series.json` per 1.1.3
source with the canonical `SourceSeries` shape. Golden files hold expected `RegistryEntry` JSON and expected feed
order; ranking changes update goldens deliberately.

Invariants to assert beyond examples: normalization idempotent; pairwise score symmetric; clustering transitive
except explicit veto; merge commutative/associative w.r.t. input order; no episode lost from any source; disabled
sources absent from all outputs; zero network calls when the feature is off or offline.

Quality gates before any phase lands: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` (CI uses the bounded `--lib` form noted in `docs/PROGRESS.md`), `bun run typecheck`,
`bun run lint`, `bun run build` (static export), plus Stitch screenshot comparison for UI phases.

---

## 14. Phased rollout

| Phase | Scope | Exit criteria |
|---|---|---|
| **R0** | Types, pure modules + tests, `0007` schema, config block, adapter trait. Feature off. | All pure tests + goldens green; no behavior change; AGENTS/PROGRESS updated |
| **R1** | Adapters for donghuastream/animekhor/animecube + the six 1.1.3 sources; M1 local backfill; identity/merge publication; review queue IPC. No feeds, no new UI. | Backfill reproducible on a real library; alias-bridged merge and conflict fixtures pass; six 1.1.3 descriptor keys/capabilities filled in from actual modules |
| **R2** | `mode=preview`: "All sources" catalog, merged detail with provenance, manual merge/split, source badges. | Stitch comparison done; pages <150 lines; all states (loading/empty/error/stale) present |
| **R3** | Health monitoring, capability scoring, FYP/new-episode/calendar behind `personalization` opt-in. | Health state machine tested; opt-in off uses zero user signals; diversity caps verified |
| **R4** | `mode=unified` for fresh installs; grouped library view via `series.canonical_id`; docs updated. | Old installs keep prior setting; rollback verified; all gates green |

Rules: no phase starts until the previous one's tests pass on real fixtures; no source enters FYP before its
adapter passes the contract test and a resolver smoke test; each phase ends demoable, with feature flags allowing
partial work to ship dark.

---

## 15. Explicit non-goals

- **No YouTube.** No Data API, no `ytsearch`, no YouTube candidates in identity, feeds or fallback. The research in
  `reports/source-fallback.md` remains research-only; this decision follows `AGENTS.md` §5/§14.
- **No automatic download from unverified sources.** Downloads still require the existing explicit enqueue action.
  A source gets download/stream affordances only with a working `resolve_for_download` path and green health.
  Streamplay stays blocked; animecube's broken resolver stays non-preferred.
- **No system/account dependency:** no login, no sync, no cloud, no telemetry upload, no new secrets.
- **Not a replacement for `sources::resolve_for_download`.** Resolved media URLs are never persisted; the registry
  stores stable page URLs and `media_ids` digests only.
- **No downloader changes:** completion, retries, archive, queue and container semantics stay exactly as in §2.
- **No external metadata providers** (TMDB/AniList/MyAnimeList) in this release; external IDs only if a user enters
  them manually, and then only as `UserAsserted` aliases.
- **No invented release dates:** calendar entries are "expected" inferences with confidence, never promises.
- **Source-specific catalogs are not deleted.** The unified view is additive; per-source tabs and `/item?source=&id=`
  routes remain.
- **No new visual design** in this spec — UI work follows Stitch screens/tokens and the shared component set.

---

## 16. Items to settle at implementation time (recorded, not blocking)

- Concrete keys, hosts, capability sets and alias endpoints of the six 1.1.3 sources (read the actual modules; do
  not guess). Registry code must still treat them as descriptors, not match arms.
- Whether to include a perceptual-hash crate for poster similarity (§4.3/§6) or keep that signal neutral initially —
  default proposal: neutral at R1, optional at R3.
- FTS5 vs in-memory token search for `registry_search` (proposal: in-memory at R2, FTS5 only if profiling demands).
- Whether `pinned_source` overrides playback choice: proposal — **metadata display only**; playback still follows
  local-file-first + candidate ranking, unless the user explicitly picks a source in the episode row.
- Cadence classification thresholds may need tuning against real 1.1.3 fixture dates; keep them in one pure
  constants module so a single test update covers it.
