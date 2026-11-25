# Plan: Phase 4 (Player) and Phase 5 (Settings & polish)

Audit date: 2026-11-25. Status verified against the tree at `origin/main` commit
`98dbefc`. The two deferred items (Add Series wizard — Stitch screen TBD;
AnimeCube playback `resolve()` — new endpoint unknown) are explicitly **out of
scope** for this plan, as requested.

## Phase 4 — Player

### What exists

| Capability | Status | Evidence |
|---|---|---|
| Local file playback | ✅ done | `app/watch/page.tsx` — `<video>` + `convertFileSrc`, path persisted in `episodes.file_path` (migration 0005), "Open in my player" hand-off |
| Watch progress save/resume | ⚠️ 60% | `lib/ipc/watch-progress.ts` + Rust commands + migration 0006; resume on `loadedmetadata`, throttled writes (5 s), force writes on pause/seek/end. **No auto-mark-watched at 90%, no next-episode autoplay** |
| Streaming playback | ⚠️ 60% | `app/stream/page.tsx` + `components/player/player-surface.tsx` (react-player v3, loading overlay, copyable error panel); Rust `resolve_playable` resolves source pages at play time, URLs never persisted. **No header injection / HLS manifest proxy** |
| Settings surface | ✅ done | `PlayerConfig { engine, volume, external_player, remember_position, auto_play_next }`, whitelist `auto|mpv|html|external`, all exposed in `settings-panel.tsx` — but `engine` is currently **cosmetic** |
| Decision record | ✅ done | `docs/decisions/player.md` — webview engine (react-player online, `<video>` local), external hand-off, no libmpv; limitations recorded |
| Notifications / single instance / window state | ✅ done | `tauri-plugin-notification` used by the download queue; `tauri-plugin-single-instance` focuses the window; `tauri-plugin-window-state` registered |

### What is missing

1. **`PlayerEngine` abstraction** (`AGENTS.md §8`) — no `src/lib/player/engine.ts`, no adapters. The `player.engine` setting does nothing.
2. **Reusable player components** (`AGENTS.md §9`) — `components/player/` contains only `player-surface.tsx`; missing `VideoSurface`, `ControlBar`, `SeekBar`, `TrackMenu`, `MiniPlayer`, `PlayerOverlay`.
3. **Controls UX** — keyboard shortcuts, speed control, audio/subtitle track menus, fullscreen overlay that keeps titlebar/controls visible. Theater Mode (Stitch screen `07d31346f286421cb5a599a166531695`) not implemented.
4. **Resume/watched completion** — `watch_progress.completed` exists but nothing sets it at ~90%; `auto_play_next` config flag is unused.
5. **Source choice** — an episode with both a downloaded file and a stream always plays one path; no "choose local vs stream" UI and no "prefer local" logic.
6. **Header-bearing HLS** — `ResolvedMedia` has no headers field; the §8 loopback proxy (token, `Range`, manifest rewrite, `Referer`/`User-Agent`) is not implemented.
7. **Capability probe** — `auto` does not probe; no visible non-blocking notice when falling back.
8. **Mini-player** — no cross-route persistent player (zustand player store absent).

**Estimated completion: ~40%.**

### Implementation plan (in order)

1. **Pull the Stitch Theater Mode Player screen first** (`AGENTS.md §5`): fetch code + screenshot for `07d31346f286421cb5a599a166531695`, extract any new tokens into `docs/design/tokens.md` + `app/globals.css`, then build the UI from tokens with shadcn/Tailwind — never paste Stitch HTML. If MCP is unavailable, ask for an HTML+PNG export before starting UI work.
2. **`src/lib/player/engine.ts`** — define `PlayerEngine`, `PlayableSource`, `Track`, `PlayerEventMap`, `Unsub`. UI talks only to this interface.
3. **Adapters** in `src/lib/player/adapters/`: `HtmlVideoEngine` (wraps `<video>` + react-player for HLS), `ExternalPlayerEngine` (uses the existing `open_in_external_player` command), and a documented stub for `MpvEngine` if/when libmpv is revisited. Keep the shipped webview path as the default; the decision record stays valid.
4. **Rust player service** (`src-tauri/src/player/`): capability probe command, `player://state` events, and the loopback stream proxy (random port, per-session token, loopback-only bind, `Range` support, HLS manifest/segments rewritten through the proxy, headers injected). Add headers to `ResolvedMedia` only if a real source needs them.
5. **Wire config**: `player.engine = auto|html|external` selects the adapter; `auto` probes and falls back with a visible, non-blocking toast. Volume, remember-position, auto-play-next are honoured.
6. **Build the shared player components** and rewire `/watch` + `/stream` to a `usePlayer` hook → IPC → engine. Add keyboard shortcuts, speed, track menus, fullscreen handling, and `MiniPlayer` backed by a small zustand store.
7. **Watched + next episode**: set `completed = 1` at ≥90% (one write), auto-advance when `auto_play_next` is on, and prefer the local file when both sources exist.
8. **Gates + update** `docs/decisions/player.md`, `docs/PROGRESS.md`, `AGENTS.md` §4/§8 if the chosen engine changes.

**Exit criteria:** a downloaded MKV/MP4 and a remote HLS stream both play in-app, seek/fullscreen/track/speed controls work, progress resumes across restarts, "watch to 90% → next episode" works, mini-player survives route changes, and the engine setting changes behaviour. All gates green; commit + push.

## Phase 5 — Settings & polish

### What exists

| Capability | Status | Evidence |
|---|---|---|
| Settings page + config | ✅ done | `app/settings/page.tsx` (19 lines) + `settings-panel.tsx` with ~18 rows; config JSON atomic writes, 0600/0700, versioned |
| yt-dlp updater | ✅ done | `downloader/updater.rs` + `update_ytdlp` command + Settings/Downloads UI |
| Single instance / window state | ✅ done | plugins registered and wired |
| Notifications | ⚠️ 50% | downloads only (queue completion/failure); no other lifecycle notifications |
| About + disclaimer | ✅ done | `app/about/page.tsx` 69 lines, 6 sections; root `DISCLAIMER.md` carried forward |
| Packaging | ⚠️ 25% | `release.yml` covers Windows/macOS/Linux and `externalBin` is configured, but no local `.deb`/AppImage/NSIS build has been verified and icon/binary naming per target is untested |
| Third-party licences | ✅ done | `dstauri/THIRD_PARTY.md` |

### What is missing

1. **Stitch Settings parity pass** — the panel is functionally complete but has not been compared with Stitch screen `de1b2c77759d46e1803c376f3c339eb6`.
2. **Notification coverage** — series import finished, playback errors, watch progress milestones; honour a settings toggle.
3. **First-run checks** — actionable errors for unwritable download dir and missing YouTube key are incomplete; missing sidecars already handled.
4. **Packaging verification** — icons, `externalBin` target-triple suffixes on all platforms, `.deb`/AppImage/NSIS/DMG builds in a dry run, updater artefacts if wanted.
5. **App self-updater** — no `tauri-plugin-updater`; decide whether releases should auto-update (not required by AGENTS.md but implied by "GitHub release workflow").
6. **Final docs pass** — `PROGRESS.md`, `AGENTS.md` phase checkboxes, release notes, `.github/workflows/ci.yml` final green run.

**Estimated completion: ~70%.**

### Implementation plan (in order)

1. Pull the Stitch Settings screen and do a parity pass; keep the existing functional rows, adjust layout/tokens only.
2. Add notification events for import completion, watch milestones and fatal playback errors, with a Settings toggle stored in config.
3. Tighten first-run checks (download dir writable, sidecars executable, YouTube key present when searching) into friendly, copyable errors.
4. Verify packaging for every target in `release.yml`: bundle icons, confirm sidecar naming, and build at least a Linux `.deb`/AppImage locally (or explain in the report why the environment cannot).
5. Optional: add `tauri-plugin-updater` + signing keys **only if user approves** (needs secrets).
6. Final gate sweep (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bun run typecheck`, `bun run lint`, `bun run build`), update `PROGRESS.md`, commit + push.

## Working agreement

- No implementation until the user approves this plan.
- The two deferred items stay deferred: Add Series wizard (needs a Stitch screen) and AnimeCube playback resolution (endpoint unknown).
- Commit + push at the end of each phase; keep untracked files unless asked otherwise.
- UI work begins by pulling the design from Stitch; if MCP is unavailable, ask for HTML + PNG exports rather than improvising.
