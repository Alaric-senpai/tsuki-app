# Decision research: streaming playback for 1.1.4

**Status:** research only — nothing in this document ships in 1.1.3.
**Branch at time of research:** `1.1.3` · **Date:** 2026-11-28 · **Platform:** Fedora 44 Linux, Wayland session.
**Scope:** why playback fails for both local files and remote streams; what to prove and decide in 1.1.4.

This document deliberately separates **verified findings** (read from source or observed by running a
command) from **hypotheses** (plausible but not proven here). No secrets, API keys, or tokens are
recorded. No source, config, workflow, version, changelog, or other documentation was changed.

---

## 1. Bottom line

There is no single "streaming is broken" bug. The current playback path contains **four independent,
verified blockers**; any one of them is sufficient to make a given episode fail:

| ID | Verified blocker | Effect |
|---|---|---|
| V1 | The dev machine's config has `player.engine = "mpv"`, but `MpvEngine` always throws and `createEngine()` silently swaps in `ExternalPlayerEngine` | `/watch` hands the file to another app; the in-app `<video>` stays blank. No "engine changed" notice reaches the UI. |
| V2 | The play-time resolver returns **Dailymotion page URLs**, and `react-player` v3 has no Dailymotion player; its fallback is a plain `<video src="https://www.dailymotion.com/video/…">` | `/stream` gets an HTML document where it expects media → media error / "Playback unavailable". |
| V3 | No header injection and no loopback proxy exist anywhere in the playback path | Any stream needing `Referer`/`User-Agent` fails; `HtmlVideoEngine` only logs a console warning. |
| V4 | AnimeCube and Streamplay URLs are intentionally rejected by `resolve_for_download` | Those episodes never reach a player at all; they surface a friendly error. |

On top of that, HLS in the mini-player has no HLS implementation (V5), and the existing
`docs/decisions/player.md` "decision" records none of the §8 acceptance tests (V6) — it is a
post-hoc rationale, not evidence of a completed spike.

**Conclusion:** do not try to "fix a codec" first. The codec stack on this machine is present and
works at the GStreamer and mpv level (§4). Fix the **source→engine contract** and run the real §8
spike before choosing an embedded engine.

---

## 2. Inventory of the playback path (as shipped on `1.1.3`)

### 2.1 Frontend

| Step | Location | What it actually does |
|---|---|---|
| Online route | `app/stream/page.tsx:16-25` | Reads `?url=`, calls `resolvePlayable(pageUrl)` via `useAsyncResource`. |
| Online render | `app/stream/page.tsx:64-75` | Renders `<PlayerSurface key={url} src={playable.data.url} />`; no engine preference is consulted. |
| IPC wrapper | `lib/ipc/player.ts:24-29` | `ipcCall("resolve_playable", …)` → `trackedInvoke` (`lib/tauri.ts:6-38`) → Tauri `invoke`. `PlayableSource` has `url/title/source/platform/isHls/direct` — **no headers, no subtitles**. |
| Online player | `components/player/player-surface.tsx:111-124` | `ReactPlayer src={src} controls playsInline …`, `canPlay()` deliberately not used as a gate (`:34-38`). |
| Local route | `app/watch/page.tsx:51-90` | Resolves path from `?path=` or episode id, reads `config.player.engine` (`:66-70`), reads `watch_progress` for resume (`:77-83`). |
| Local render | `app/watch/page.tsx:172-185` | `<VideoSurface source={{kind:"file",…}} preference={engine} startAt=… />`. |
| Shared surface | `components/player/video-surface.tsx:34-41`, `:63-68` | Owns a bare `<video>` (no `controls` attribute), creates engine through `usePlayer`, loads source on engine change, overlays `PlayerControls`. |
| Engine hook | `hooks/use-player.ts:63-106` | `createEngine(preference, video)` in an effect; subscribes to `ready/timeupdate/state/tracks/error`. |
| Engine contract | `lib/player/engine.ts:14-72`, `:74-84` | `PlayableSource` supports `headers`/`subtitles`/`isHls`; `EnginePreference = auto\|mpv\|html\|external`. |
| Engine factory | `lib/player/index.ts:14-34` | `auto`/`html` → `HtmlVideoEngine`; `external` → `ExternalPlayerEngine`; `mpv` → `new MpvEngine()` with `catch → ExternalPlayerEngine()`. |
| HTML adapter | `lib/player/adapters/html-video.ts:81-106` | Local: `video.src = convertFileSrc(path)`. Remote: `video.src = source.url`; headers only produce `console.warn` (`:95-99`). No hls.js. |
| mpv adapter | `lib/player/adapters/mpv.ts:9-18` | Constructor **always throws** `PlayerEngineUnavailable`. |
| External adapter | `lib/player/adapters/external.ts:30-42` | File → `openInExternalPlayer(path)`; stream → throws "External player needs a local file." |
| Mini player | `components/player/mini-player.tsx:17-26`, `:38-39`, `:64` | Global mount in `app/layout.tsx:72`; hardcodes `usePlayer(video, "auto")` → HTML engine; holds a resolved, possibly expiring URL in zustand. |
| Controls | `components/player/player-controls.tsx:74-79`, `:84-132` | Fullscreen via `document.querySelector("[data-player-surface]")` and `requestFullscreen()`; keyboard shortcuts. |
| Settings UI | `components/features/settings/settings-panel.tsx:50-55`, `:454-460` | Offers `Auto`, `mpv`, `HTML video`, `External player` — with no warning that `mpv` degrades to external. |

**Note on stale docs:** the comment in `components/player/player-surface.tsx:10-13` and
`docs/decisions/player.md:11-21` claim both routes use `PlayerSurface`. They do not: `/watch` uses
`VideoSurface`/`usePlayer`.

### 2.2 Rust

| Step | Location | What it actually does |
|---|---|---|
| IPC command | `src-tauri/src/commands/player.rs:34-65` | `resolve_playable` → `sources::resolve_for_download` → returns bare `url`; on `None` returns the input URL unchanged with `direct: true`. |
| Resolved type | `src-tauri/src/sources/mod.rs:33-47` | `ResolvedMedia { url, title, source, video_id, platform, is_hls }` — **no headers/subtitle fields**. |
| Resolver dispatch | `src-tauri/src/sources/mod.rs:86-105` | DonghuaStream → resolver; AnimeCube → **error**; Streamplay → **error**; unknown → `None` (raw URL passed through). |
| Network gate | `src-tauri/src/sources/mod.rs:124-154` | Every `fetch_text` calls `network::ensure_online()`. |
| Connectivity probe | `src-tauri/src/network.rs:20-30`, `:99-167` | Requires HTTP 204 from `google.com/generate_204` (fallback `clients3.google.com`), 4 s timeout, 15 s cache. |
| DonghuaStream parser | `src-tauri/src/sources/donghuastream/mod.rs:169-197`, `:227-244`, `:266-310` | Dailymotion embeds are normalised to `https://www.dailymotion.com/video/<id>`; `pick_direct` prefers Dailymotion **over** a direct `.mp4`/`.m3u8` mirror. |
| Animekhor parser | `src-tauri/src/sources/animekhor/resolver.rs:10-27` | Reuses the DonghuaStream parser, so it also returns Dailymotion page URLs. |
| Command registration | `src-tauri/src/lib.rs:272-341` | `resolve_playable` and `open_in_external_player` are registered. |
| External hand-off | `src-tauri/src/commands/config.rs:43-68` | Empty command → `opener().open_path`; configured command → `std::process::Command` with the path appended. |
| CSP | `src-tauri/tauri.conf.json:24-36` | `media-src` allows `asset:`, `http://asset.localhost`, `blob:`, `https:`, `http://127.0.0.1:*`; `connect-src` allows `ipc:`/`https:`/loopback. |
| Asset protocol | `src-tauri/tauri.conf.json:37-44` | Scope `$HOME/**`, `$VIDEO/**`, `$DOWNLOADS/**`. |
| Capabilities | `src-tauri/capabilities/default.json:8-14` | `core:default`, `opener`, `log`, `dialog`, `notification`; no player-specific permission, no `window:allow-*`. |
| Dependencies | `src-tauri/Cargo.toml:20-55`, `package.json:17-58` | No libmpv/Libmpv2/GStreamer crate, no `hls.js` package; `react-player@^3.4.0` supplies `hls-video-element` transitively. |

**Explicitly absent:** there is no `src-tauri/src/player/` directory, no loopback proxy, no
`player://state` events, and no `ResolvedMedia.headers`. The §8 proxy is documented as future work
in `docs/decisions/player.md:31-35` only.

---

## 3. Verified findings

### V1 — `mpv` preference silently becomes "open in another app"

1. Observed on this machine (config read with a script that prints **only** non-secret fields):
   `player.engine = mpv`, `player.external_player` empty, `settings.global_save_dir = /home/alaricsenpai/Videos`.
2. `Settings` exposes `mpv` as a normal choice (`settings-panel.tsx:50-55`), consistent with AGENTS.md §8
   calling mpv "preferred".
3. `app/watch/page.tsx:66-70` passes that value to `VideoSurface`; `lib/player/index.ts:21-26` tries
   `new MpvEngine()`, which unconditionally throws (`adapters/mpv.ts:12-17`), and **catches the error
   internally**, returning an `ExternalPlayerEngine`. Because `createEngine` no longer throws,
   `usePlayer`'s error/notice branch (`hooks/use-player.ts:68-79`) never runs — there is no UI notice.
4. `ExternalPlayerEngine.load` (`adapters/external.ts:30-42`) calls `openInExternalPlayer(path)`;
   `xdg-mime query default video/mp4` on this machine returns `mpv.desktop`. The file opens in a
   separate mpv window while Tsuki's `<video>` remains empty.
5. For a stream source the same adapter throws "External player needs a local file", which is the
   error the mini-player would report if it shared the preference (it does not; it hardcodes `auto`).

**Impact:** "local streaming never works" is fully explained for the current config, without any
codec failure.

### V2 — Resolver output is not playable by the chosen webview player

1. `pick_direct` (`donghuastream/mod.rs:227-244`) returns Dailymotion first; `dailymotion_direct`
   (`:169-197`) converts an embed into a canonical **HTML page** URL.
2. `react-player` v3's recognisers (`node_modules/react-player/dist/patterns.js`) contain patterns for
   mp4/webm/m3u8/mpd, YouTube, Vimeo, Wistia, Mux, Spotify, Twitch, TikTok — **no Dailymotion**.
   Its fallback (`dist/HtmlPlayer.js`) is literally `React.createElement("video", {…props})`.
3. Runtime check executed here:

   ```
   node --input-type=module -e "import ReactPlayer from 'react-player'; const urls=[…]; for (const u of urls) console.log(ReactPlayer.canPlay(u), u)"
   ```

   Output:

   ```
   false https://www.dailymotion.com/video/x8abc12
   true https://www.youtube.com/watch?v=dQw4w9WgXcQ
   true https://cdn.example/master.m3u8
   true https://cdn.example/video.mp4
   true asset:///home/u/Videos/ep.mp4
   ```

4. `docs/decisions/player.md:17` states that `canPlay()===false` was deliberately not treated as
   fatal because "the HTML fallback still handles hosts it does not recognise (e.g. Dailymotion)".
   That reasoning is wrong for a Dailymotion **page**: the fallback creates a `<video>` pointing at
   an HTML document, not at media. `docs/PROGRESS.md:126` ("Dailymotion/streamplay now render") is
   likewise misleading — rendering the element is not playback.

**Impact:** most DonghuaStream/animekhor episodes cannot play on `/stream`, even though the same URL
works with yt-dlp downloads (the downloader has Dailymotion extraction support; the webview does not).

### V3 — Headers and the loopback proxy do not exist

1. `ResolvedMedia` (`sources/mod.rs:33-47`) and the IPC `PlayableSource` (`lib/ipc/player.ts:4-12`)
   carry no headers.
2. `commands/player.rs:47-55` copies only url/title/source/platform/isHls.
3. `HtmlVideoEngine.load` (`adapters/html-video.ts:92-101`) sets `video.src` directly and warns to
   the console when `source.headers` is non-empty (`:95-99`).
4. `tauri.conf.json:30-32` already permits `http://127.0.0.1:*`, so CSP is not the blocker — but a
   grep across `src-tauri/src`, `app`, `components`, `hooks`, `lib` found no proxy implementation,
   no header injection, no manifest rewrite.
5. A header-gated local HLS test (§4) confirms a plain media request cannot satisfy `Referer`.

**Impact:** header-bearing HLS fails by design. This is a known, accepted limitation in
`docs/decisions/player.md:33`, not a regression.

### V4 — Two whole sources are deliberately unavailable to the player

`resolve_for_download` (`sources/mod.rs:86-105`) returns errors for Streamplay (`:87-92`) and
AnimeCube (`:98-101`). Because `resolve_playable` delegates to it, `/stream` inherits those errors.
This is intentional for downloads; it is a poor fit for a player that could try a direct URL or a
different mirror.

### V5 — HLS is implemented in only one of three player surfaces

- `/stream`: `react-player` lazy-loads `hls-video-element` for `.m3u8` URLs (present in the static
  export: `out/_next/static/chunks/1etp6z7x_015k.js` and `2hgms606txw9y.js` contain HLS parser code).
- `/watch`: local files only, so HLS is not normally reached.
- Mini player: `components/player/mini-player.tsx:21,64` uses the bare `<video>` HTML engine, which
  has no HLS support. Opening HLS from `/stream` in the mini player therefore changes engine semantics.

### V6 — The recorded "player decision" did not run the §8 spike

`docs/decisions/player.md` claims "accepted (pragmatic, post-hoc spike)" and "completes Phase 0b",
but contains no commands, test media, compositor results, or acceptance data. `AGENTS.md` §8 requires
five criteria (local MKV/MP4 1080p, HLS with custom headers, repeat seeks, fullscreen with overlay,
Wayland **and** X11); none is evidenced. `AGENTS.md` §14 still lists the decision as open. Treat the
engine choice as **unsettled**.

### V7 — `removeUnusedCommands` is not stripping player commands (ruled out)

`tauri.conf.json:11` enables `removeUnusedCommands`. Concerns that the dynamic `ipcCall(command, …)`
wrapper hides command names are unfounded for **app** commands: the generated
`target/debug/build/tauri-1babcee64b11648d/out/allowed-commands.json` contains only plugin commands
and `"has_app_acl": false`. `strings src-tauri/target/debug/tsuki | grep -c <name>` found
`resolve_playable` 35 times, `open_in_external_player` 24, `get_config` 13, `download_state` 4,
`save_watch_progress` 49. Caveat: that binary is from 2026-11-25, older than some source edits.

---

## 4. What was actually tested on this Linux machine

### 4.1 Environment (observed)

```
$ echo $XDG_SESSION_TYPE $WAYLAND_DISPLAY $DISPLAY
wayland wayland-0 :0
$ cat /etc/fedora-release
Fedora release 44 (Forty Four)
$ rpm -qa | grep -iE 'webkit2gtk|gstreamer1' | head
webkit2gtk4.1-2.54.2-1.fc44.x86_64
gstreamer1-1.28.1-1.fc44.x86_64  (+ plugins-base/good/bad/libav/openh264/ugly)
$ command -v ffmpeg ffprobe mpv gst-launch-1.0 gst-inspect-1.0
/usr/bin/ffmpeg /usr/bin/ffprobe /usr/bin/mpv /usr/bin/gst-launch-1.0 /usr/bin/gst-inspect-1.0
$ for c in Xvfb xvfb-run weston; do printf '%-10s ' "$c"; command -v "$c" || echo MISSING; done
Xvfb       MISSING
xvfb-run   MISSING
weston     MISSING
```

### 4.2 Static checks (passed)

```
$ cd dstauri && bun run typecheck        # tsc --noEmit
(no output; exit 0)
$ bunx vitest run lib/player/adapters/html-video.test.ts
Test Files 1 passed (1); Tests 2 passed (2)
```

### 4.3 Codec/decoder capability (passed at library level, not in the app)

```
$ ffmpeg -f lavfi -i testsrc=size=1920x1080:rate=24 -t 3 -c:v libx264 -pix_fmt yuv420p … h264-1080p.mp4
$ ffmpeg -f lavfi -i testsrc=size=1920x1080:rate=24 -t 3 -c:v libx265 -pix_fmt yuv420p … hevc-1080p.mkv
$ for f in h264-1080p.mp4 hevc-1080p.mkv; do ffprobe -show_entries stream=codec_name,width,height …; done
h264,1920,1080
hevc,1920,1080

$ timeout 30 mpv --no-config --vo=null --ao=null --frames=24 … h264-1080p.mp4   # exit 0
$ timeout 30 mpv --no-config --vo=null --ao=null --frames=24 … hevc-1080p.mkv   # exit 0

$ timeout 30 gst-launch-1.0 -t playbin uri=file:///…/h264-1080p.mp4
video codec: H.264 (High Profile); Got EOS …; Execution ended after 0:00:03.000; exit 0
$ timeout 30 gst-launch-1.0 -t playbin uri=file:///…/hevc-1080p.mkv
video codec: H.265 (Main Profile); Got EOS …; Execution ended after 0:00:03.000; exit 0

$ gst-inspect-1.0 | grep -iE 'avdec_hevc|hevc.*dec|hlsdemux|matroskademux|qtdemux|playbin'
adaptivedemux2: hlsdemux2, hls: hlsdemux, de265: libde265dec, libav: avdec_h265,
matroska: matroskademux, isomp4: qtdemux, playback: playbin/playbin3
```

`gst-launch` used `xvimagesink0`, i.e. X11 via XWayland, even though the session is Wayland. This
does **not** prove WebKitGTK's `<video>` behaves identically; it proves the underlying decoder stack
is installed and functional here.

### 4.4 Header-gated local HLS (demonstrates V3)

Generated a 6 s HLS playlist with ffmpeg, then served it from a tiny Python HTTP server bound to
`127.0.0.1:18987` that answers 403 unless `Referer: https://cdn.example/` is present:

```
$ curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:18987/master.m3u8
403
$ curl -s -o /dev/null -w '%{http_code}\n' -H 'Referer: https://cdn.example/' …/master.m3u8
200
$ timeout 20 gst-launch-1.0 playbin uri=http://127.0.0.1:18987/master.m3u8
ERROR: from element …/GstSoupHTTPSrc:source: Internal data stream error.
streaming stopped, reason error (-5); pipeline doesn't want to preroll.
```

Interpretation: media stacks cannot magically supply headers. Tsuki needs either a Rust proxy that
injects them (§8) or an engine that accepts header options (mpv).

### 4.5 What could NOT be tested autonomously

- **In-app WebKitGTK playback** on `/stream` or `/watch`: requires driving the real Tauri window and
  reacting to media events. There is no Xvfb/Weston for a headless second session, and opening the
  app would take over the user's desktop; no such GUI automation was run.
- **Repeat seeks, pause/resume, fullscreen overlay, keyboard shortcuts**: interactive; no evidence.
- **Wayland vs X11 comparison in the app**: only one compositor session is live; switching sessions
  or launching nested compositors is not available here (no Xvfb/Weston). `gst-launch` above only
  exercised XWayland.
- **Asset protocol denial for out-of-scope paths**: would require a downloaded file outside
  `$HOME/**`/`$VIDEO/**`/`$DOWNLOADS/**` plus GUI observation. Default config is in scope, so this
  is a conditional hypothesis, not a current failure.
- **Real remote HLS with live/expiring URLs**: no account/keys were used and no third-party stream was
  fetched by this research; the header-gated loopback server is a controlled substitute.
- **Sidecar resolution**: `yt-dlp`/`ffmpeg` are not on the playback path at all — `resolve_playable`
  uses reqwest, not yt-dlp. This candidate is ruled out for streaming (it remains relevant to downloads).

### 4.6 Hypotheses still open (not verified)

| ID | Hypothesis | How to confirm |
|---|---|---|
| H1 | WebKitGTK's `<video>` rejects a codec even though GStreamer playbin can decode it (e.g. HEVC in MKV with soft subs, or MSE/HLS specifics). | Open the same generated MKV/MP4 through `/watch` with engine `html` and capture `MediaError.code`/console on Wayland and X11. |
| H2 | `convertFileSrc` scope denies a save dir outside `$HOME/**`, `$VIDEO/**`, `$DOWNLOADS/**`. | Configure an out-of-scope save dir, download one episode, inspect `asset://` request status. |
| H3 | `network::ensure_online()` (Google `generate_204`, 4 s, 15 s cache) misclassifies a restricted network and blocks every source fetch. | Run `curl -s -o /dev/null -w '%{http_code}' https://www.google.com/generate_204` and compare with the app's network banner; check `detail` in the error panel. |
| H4 | A resolved URL expires while parked in the mini-player (zustand holds it across route changes). | Leave the mini player open past the source's token lifetime, then press Play. |
| H5 | ReactPlayer's lazy player chunks 404 in the packaged static export. | Low probability: HLS parser code is present in `out/_next/static/chunks/1etp6z7x_015k.js`; confirm in DevTools Network with a packaged build. |

---

## 5. Recommended phased plan for 1.1.4

### Phase A — Fix the source→engine contract first (small, reviewable diffs)

1. **Make engine selection honest.** Either remove `mpv` from `ENGINE_OPTIONS` until an embedded mpv
   exists, or keep it but surface `PlayerEngineUnavailable` as a visible, non-blocking notice instead
   of swallowing it in `lib/player/index.ts:21-26`. Add a regression test for `createEngine("mpv")`.
2. **Unify `/stream` onto `VideoSurface`** so one engine path serves local and remote sources; keep
   `PlayerSurface` only if a host-specific player is genuinely required. Never let two surfaces pick
   different HLS behaviour.
3. **Add a play-time resolver distinct from the download resolver.** Return *all* candidate media
   (`{ url, headers, subtitles, isHls, platform, rank }`) instead of one download-optimised URL.
   Rank direct `.m3u8`/`.mp4` above a Dailymotion page when the consumer is the webview; keep the
   current ranking for yt-dlp. Extend `ResolvedMedia` and the IPC `PlayableSource` with
   `headers?: Record<string,string>` and `subtitles?`.
4. **Implement the §8 loopback proxy** (loopback-only, per-session token, `Range`, header injection,
   HLS manifest + segment rewrite) for the HTML engine. CSP already allows `127.0.0.1:*`.
5. **Add an explicit HLS adapter** (`hls.js` on the bare `<video>` in the mini player and HTML engine)
   instead of relying on one library's implicit selection, with a visible notice when unsupported.
6. **Tighten asset scope** to the user's configured download directories (not all of `$HOME`), and
   give a copyable error when a path is outside it.
7. Tests to add: config `mpv` fallback notice; resolver ranking (Dailymotion vs direct HLS fixture);
   proxy token rejection, `Range`, and manifest rewrite; player-surface engine parity.

### Phase B — Run the actual embedded-engine spike (AGENTS.md §8, in order, stop at first pass)

Build a minimal spike app or use the real app with a test route; record commands and outputs in
`docs/decisions/player.md`. Candidates, in order:

1. `tauri-plugin-libmpv` — re-check the **current** release's Linux/Wayland status (docs + a small
   probe app). The old note "Linux window embedding not working" may be stale.
2. `tauri-plugin-video` — GStreamer default with optional `mpv-runtime`; evaluate header/seek support
   and maturity.
3. Own Rust integration with the `libmpv2` crate (render API or native child surface) behind
   `MpvEngine`; pass headers via `http-header-fields`, `referrer`, `user-agent` — no proxy needed.
4. IPC-controlled **windowed** mpv as the external path (`--input-ipc-server` for progress sync).
   Do **not** use `--wid` embedding; it is X11-only and unusable on Wayland.
5. Last resort: keep `HtmlVideoEngine` only, add an in-app GStreamer-plugin install prompt, and keep
   the loopback proxy for headers.

**Acceptance criteria (all must pass before the engine is chosen):**

| # | Criterion | Suggested proof |
|---|---|---|
| 1 | Local MKV (H.264/HEVC, soft subs) and MP4 at 1080p play smoothly | Use generated `hevc-1080p.mkv` / `h264-1080p.mp4` from §4.3; note `playbin`/`mpv` decode already passes. |
| 2 | Remote HLS with custom `Referer`/`User-Agent` plays | Reuse the header-gated loopback server from §4.4; log the exact headers received. |
| 3 | ≥5 repeated seeks and pause/resume without freeze | Scripted seek sequence + screenshot/state log; record time-to-seek. |
| 4 | Fullscreen works and controls/titlebar overlay correctly | Screenshot in fullscreen on the same window. |
| 5 | Works on **Wayland and X11** | Repeat 1–4 in both sessions; if only XWayland is available, say so explicitly. |

### Phase C — Implement the winner

- Wire the chosen engine behind `PlayerEngine` (no UI component may touch `ReactPlayer`/`<video>`
  directly), keep a graceful fallback with a visible notice, and delete the dead `MpvEngine` throw
  stub or replace it with the real adapter.
- Keep the proxy for the HTML path even if mpv wins, so runtime fallback still supports headers.
- Preserve `watch_progress` resume/auto-watched and both source choices (local file preferred).

### Phase D — Verification gates

Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `bun run typecheck`, `bun run lint`,
`bun run build` and re-run the §4 commands plus all five acceptance criteria. Update `docs/PROGRESS.md`.

---

## 6. Decision to record when the spike is done

Record in `docs/decisions/player.md` (and update `AGENTS.md` §8/§14) **with evidence attached**:

1. **Chosen engine** and the exact candidate order that was actually tested (with pass/fail per
   criterion, Wayland and X11 separately). State explicitly if any criterion could not be tested.
2. **Header strategy**: mpv options vs loopback proxy vs both; include the token/`Range`/HLS-rewrite
   design that was verified.
3. **Source ranking for play vs download**: which candidate wins for a Dailymotion page that also
   exposes a direct `.m3u8`, and why.
4. **Meaning of `player.engine = mpv`**: embedded engine, or explicit external hand-off with a
   visible notice. Never a silent swap.
5. **Fallback order** (`auto` → …) and the non-blocking notice shown to the user.
6. **Rejected candidates** with the concrete reason (e.g. "libmpv plugin: window embedding failed on
   Wayland, log at …").

Until that decision is recorded, `AGENTS.md` §14's item "Embedded player engine result of the spike"
must remain **open**.

---

## 7. Non-goals / guardrails

- No Streamplay decryption, embedding bypass, or credential use was attempted; no secrets appear here.
- No changes to 1.1.3 source, config, workflows, versions, changelog, or other docs were made by this
  research task.
- This is a desktop-target plan; keep diffs small and self-contained for the phone/Termux workflow.
