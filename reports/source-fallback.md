# Streamplay replacement-source research (NOT shipped in 1.1.2)

Date: 2026-11-27 · Branch: `1.1.2` · Working dir: repository root (`sources.py`, `script.py`, `urls.py`, `db.py`, `donghua-cli`).

Scope: **no Streamplay cracking/decryption/embedding bypass was attempted.** The Streamplay page was only fetched once to record its HTTP status, then handed to `yt-dlp` to confirm it is unusable. The goal was to find a *different* source for the same series/episode.

---

## 1. Sample series / episode (real legacy catalog entry)

Chosen from the registered PostgreSQL catalog exposed by `donghua-cli`:

```bash
donghua-cli --list | grep -i 'Senior Brother'
```

Excerpt (columns trimmed):

```
3617 │ My Senior Brother Is Too Steady Season 2 │ my_senior_brother_is_too_steady_season_2 │ 155 │ 1 │ …
1    │ Senior Brother Is Too Steady             │ senior_brother                          │ 1   │ 1 │ …
```

Legacy DB row (read-only query; the CLI's Python has `psycopg2`, the system `python` does not):

```bash
PYTHONPATH=. /home/alaricsenpai/.local/share/donghua-stream/cli/bin/python -c "
import db
c=db.get_connection()
with c, c.cursor() as cur:
    cur.execute(\"SELECT e.id,e.video_id,e.url,e.episode_number,e.title FROM episodes e WHERE e.series_id=3617 ORDER BY e.id LIMIT 3\")
    for r in cur.fetchall(): print(r)
c.close()"
```

Output:

```
(35912, 'dhs-680c8fbd76c408df', 'https://donghuastream.org/my-senior-brother-is-too-steady-season-2-episode-1-14-multiple-subtitles/', 1, 'My Senior Brother Is Too Steady Season 2 Episode 1 [14] Multiple Subtitles')
(35913, 'dhs-9bb4f8390874c238', 'https://donghuastream.org/my-senior-brother-is-too-steady-season-2-episode-2-15-multiple-subtitles/', 2, 'My Senior Brother Is Too Steady Season 2 Episode 2 [15] Multiple Subtitles')
(35914, 'dhs-66743b89d35b0ffe', 'https://donghuastream.org/my-senior-brother-is-too-steady-season-2-episode-3-16-multiple-subtitles/', 3, 'My Senior Brother Is Too Steady Season 2 Episode 3 [16] Multiple Subtitles')
```

There are **no** literal `streamplay` URLs in the DB (`SELECT … WHERE url ILIKE '%streamplay%'` → `count: 0`); the Streamplay host appears at **resolve time** in the DonghuaStream episode page.

### Primary link resolves to Streamplay

Live episode page:

```bash
curl -4 -m 15 -sS -A 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36' \
  'https://donghuastream.org/my-senior-brother-is-too-steady-season-2-episode-1-14-multiple-subtitles/' \
  -o /tmp/dhs_ep1.html -w 'http=%{http_code} size=%{size_download}\n'
# http=200 size=245307
grep -oE '<iframe[^>]+data-litespeed-src="[^"]+"' /tmp/dhs_ep1.html | head -1
```

```
<iframe data-lazyloaded="1" src="about:blank" data-litespeed-src="//play.streamplay.co.in/embed/c095b606-6da2-4772-8fff-83409809148a" frameborder="0" allowFullScreen="true" webkitallowfullscreen="true" mozallowfullscreen="true" width="640" height="320">
```

Legacy parser (same result for episodes 1, 2, 3):

```bash
PYTHONPATH=. /home/alaricsenpai/.local/share/donghua-stream/cli/bin/python -c "
import sources
d=sources.SOURCES['donghuastream']
print(d.resolve('https://donghuastream.org/my-senior-brother-is-too-steady-season-2-episode-1-14-multiple-subtitles/', refresh=True)['url'])"
```

```
//play.streamplay.co.in/embed/c095b606-6da2-4772-8fff-83409809148a
```

The absolute form used by the shipped app is `https://play.streamplay.co.in/embed/c095b606-6da2-4772-8fff-83409809148a`.

Streamplay page status and yt-dlp verdict (no unpacking/decryption attempted):

```bash
curl -4 -m 15 -sS -o /dev/null -w 'http=%{http_code} type=%{content_type}\n' \
  'https://play.streamplay.co.in/embed/c095b606-6da2-4772-8fff-83409809148a'
# http=200 type=text/html; charset=UTF-8

yt-dlp --simulate --no-warnings --print '%(id)s | %(title)s' \
  'https://play.streamplay.co.in/embed/c095b606-6da2-4772-8fff-83409809148a'
# ERROR: Unsupported URL: https://play.streamplay.co.in/embed/c095b606-6da2-4772-8fff-83409809148a
# exit=1
```

This matches `dstauri/src-tauri/src/sources/mod.rs:87-89`, which already blocks Streamplay with an actionable message in 1.1.2.

---

## 2. Alternatives probed for that SAME series / episode

Sources available in legacy code: `sources.py:962-971` registers only **AnimeCube** and **DonghuaStream**; YouTube is a separate opt-in source (`youtube_api.py`); the shipped Rust app additionally has **animekhor.org** (`dstauri/src-tauri/src/sources/animekhor/`). All were checked.

### 2a. DonghuaStream (same site, other episode page) — partial fallback

The same series page mixes hosts. Sampled episode pages via the legacy resolver:

| DHS page | Resolved player |
|---|---|
| S2E1 [14], E2 [15], E3 [16], E4 [17], E5 [18], E10 [23], E20 [33] | `//play.streamplay.co.in/embed/…` (Streamplay) |
| S2E90 [93], S2E100 [113], S2E105 [118], S2E120 [133], S2E121 [134], S2E123 [136] | Dailymotion |

Example (different episode, **not** the targeted #14), from the legacy resolver and yt-dlp:

```bash
PYTHONPATH=. /home/alaricsenpai/.local/share/donghua-stream/cli/bin/python -c "
import sources
print(sources.SOURCES['donghuastream'].resolve('https://donghuastream.org/my-senior-brother-is-too-steady-season-2-episode-100-113-multiple-subtitles/', refresh=True)['url'])"
# https://www.dailymotion.com/video/k4Sbv6kxJJvQ1vE75Ug

yt-dlp --simulate --no-warnings --print '%(id)s | %(title)s | %(duration)s' \
  'https://www.dailymotion.com/video/k4Sbv6kxJJvQ1vE75Ug'
# k4Sbv6kxJJvQ1vE75Ug | brother 113 (2) | 932   (exit=0)
```

Finding: the site itself supplies a working Dailymotion fallback for *some* episodes of this series, but **not for the sampled Streamplay episode #14**. Its page contains exactly one iframe (Streamplay) and no `video`/`source` tag or direct `.mp4`/`.m3u8` URL.

### 2b. AnimeCube — same series found, targeted episode absent

The legacy parser currently fails against the live site:

```bash
PYTHONPATH=. /home/alaricsenpai/.local/share/donghua-stream/cli/bin/python -c "
import sources
print(sources.SOURCES['animecube'].search('My Senior Brother Is Too Steady'))"
# SourceError: AnimeCube's home page did not contain its catalog. The site layout may have changed.
```

Evidence of site redesign (live, 2026-11-27):

```bash
curl -4 -m 25 -sS -o /tmp/ac_home.html 'https://animecube.live/'
grep -c 'fallbackData' /tmp/ac_home.html   # 0  (legacy parser looks for this)
grep -oE 'href="/(title|watch)/[^"]+"' /tmp/ac_home.html | head -3
# href="/title/234tnymnv6"
# href="/watch/8qytrb4"
# …
curl -4 -m 25 -sS -o /dev/null -w 'http=%{http_code}\n' \
  'https://animecube.live/anime/my-senior-brother-is-too-steady'
# http=404   (legacy slug route is gone)
```

The new public JSON APIs work:

```bash
curl -4 -m 20 -sS 'https://animecube.live/api/catalog' -o /tmp/ac_catalog.json
# http=200, application/json, 489339 bytes
curl -4 -m 20 -sS -G --data-urlencode 'q=My Senior Brother is Too Steady' \
  'https://animecube.live/api/search' | python -m json.tool | head
# http=200; first item: animeCode=8yfpj96rg3, title="Big Brother",
# aliases=["Shixiong A Shixiong","Senior Brother, Senior Brother",
#          "My Senior Brother is a Bit Too Cautious",
#          "Wo Shixiong Shizai Tai Wenjian Le",
#          "My Senior Brother is Too Steady","MSBTS"]
```

So the **same series is present under alias "Big Brother"** (`animeCode=8yfpj96rg3`). Its catalog entry says `availableEpisodeCount: 70`, `latestEpisodeDisplay: 169`, `primaryWatchEpisodeCode: nvdt26g`.

Episode list is in the Next.js flight payload of a watch page (or `/api/watch/<code>/context`):

```bash
curl -4 -m 25 -sS 'https://animecube.live/watch/vjqc644' -o /tmp/ac_watch.html
# decode self.__next_f.push([1,"…"]) chunks, then:
# 70 episodes, all numbered 100..169.  No episode numbered 14.
curl -4 -m 20 -sS -H 'Accept: application/json' \
  'https://animecube.live/api/watch/at7ye8w/context' -o /tmp/ac_ctx.json
# http=200; currentEpisodeCode=at7ye8w = "Episode 113" (same series, different episode)
curl -4 -m 20 -sS -H 'Accept: application/json' \
  'https://animecube.live/api/watch/at7ye8w/downloads'
# {"success":true,"data":{"downloads":[],"capability":{"accessState":"unavailable",…}}}
```

Result for targeted episode #14: **none found on AnimeCube** (current catalogue starts at #100). Playback sources are no longer exposed in the static page/legacy API; the player fetches an Anex ticket client-side (`/api/anex/sessions/complete`, `/api/watch/<code>/downloads`), so no direct media URL was obtained even for #113 without a browser session. This is a **parser/API migration issue**, not an unavailable series.

### 2c. animekhor.org — unreachable from this environment

```bash
curl -4 -m 15 -sS -L -A 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36' \
  -o /tmp/ak.html -w 'http=%{http_code}\n' 'https://animekhor.org/?s=Big+Brother'
# http=403, 3180 bytes, <title>Bot Verification</title>, captcha markup
```

Same result for `https://animekhor.org/page/1/?s=My+Senior+Brother+Is+Too+Steady` (403, Bot Verification). No candidate URL obtained. Recorded as unreachable rather than guessed.

### 2d. YouTube — **working replacement found**

Two usable paths; no third-party site access from the frontend is required (Rust/legacy CLI does the search).

**(i) YouTube Data API v3 via legacy `youtube_api.py`** (an API key is configured locally; value not printed):

```bash
PYTHONPATH=. /home/alaricsenpai/.local/share/donghua-stream/cli/bin/python -c "
import youtube_api
c=youtube_api.YouTubeClient(timeout=12)
print('configured:', c.configured())
for r in c.search('Big Brother S2 EP14 YOUKU ANIMATION', max_results=8, refresh=True):
    print(r.get('title'), '|', r.get('duration'), '|', r.get('url'))"
```

Result (first 4):

```
【Big Brother】S2 | EP14-20 FULL | Chinese Ancient Anime | YOUKU ANIMATION | 6805 | https://www.youtube.com/watch?v=xSSZ5DD-Osw
【Big Brother S2】EP14 | Chinese Ancient Anime | YOUKU ANIMATION | 1288 | https://www.youtube.com/watch?v=ZqO6DJl53Sg
【Big Brother S2】EP14-39 FULL | Chinese Ancient Anime | YOUKU ANIMATION | 24544 | https://www.youtube.com/watch?v=ghURLLelHrM
【Big Brother S2】EP15 | Chinese Ancient Anime | YOUKU ANIMATION | 1286 | https://www.youtube.com/watch?v=JzqUT6TZe-A
```

One `search.list` + one `videos.list` call; results cached on disk as the legacy client does.

**(ii) `yt-dlp` without an API key** (scraped YouTube search):

```bash
yt-dlp --flat-playlist --no-warnings --playlist-end 8 \
  --print '%(id)s\t%(title)s\t%(duration)s\t%(channel)s' \
  'ytsearch8:My Senior Brother Is Too Steady episode 14 donghua'
```

Same exact candidate appeared: `ZqO6DJl53Sg | 【Big Brother S2】EP14 | 1288 | YOUKU English Animation-Get APP now`.

**yt-dlp acceptance of the candidate page:**

```bash
yt-dlp --simulate --no-warnings \
  --print '%(id)s | %(title)s | %(duration)s | %(availability)s' \
  'https://www.youtube.com/watch?v=ZqO6DJl53Sg'
# ZqO6DJl53Sg | 【Big Brother S2】EP14 | Chinese Ancient Anime | YOUKU ANIMATION | 1287 | public
# exit=0
```

Format probe (from `--print '%(formats)j'`) returned **47 formats**, including plain HTTPS MP4 `134` (360p) and `136` (720p) plus HLS variants `230`/`232`; so the URL is downloadable/playable by yt-dlp and by `HtmlVideoEngine`/mpv.

Caveat: a follow-up `yt-dlp -g -f 18 …` and a repeat `--print` both timed out at 60–90 s (YouTube throttling/challenge after several probes). The two successful simulate/format probes above are the evidence; a direct signed media URL was **not** captured (those URLs expire anyway). No full download was performed, so this is a high-confidence title/number/duration match, not a byte-for-byte verification against the Streamplay video.

### 2e. Other sources in `sources.py`

None. `SOURCES` contains only `animecube` and `donghuastream`; `SOURCE_CHOICES` adds `youtube`. `urls.py` is a legacy fallback list of YouTube URLs, not a resolver. The Streamplay helpers at `sources.py:960-1201` were deliberately not exercised.

---

## 3. Practical matching strategy

1. **Normalize titles.** Lowercase; strip punctuation, `Season N`, `[NN]` bracket suffixes, and "Multiple Subtitles"; compare against both `title` and an alias list.
   - Worked here: DHS `"My Senior Brother Is Too Steady Season 2 Episode 1 [14]"` → AnimeCube `"Big Brother"` via alias `"My Senior Brother is Too Steady"` / `"MSBTS"`; YouTube title uses `"Big Brother S2"`.
2. **Prefer the continuous episode number.** DHS's bracket number (`[14]`, `[113]`) is the continuous series index; the local `episode_number` (1, 100) is season-local. AnimeCube `number` is continuous (100–169). Confirmation: DHS `S2E100 [113]` ↔ AnimeCube `number=113` (`at7ye8w`) ↔ Dailymotion title `"brother 113 (2)"`.
   - Never match on the season-local number alone (DHS E1 → AnimeCube E100 would be wrong).
3. **Guard against compilations.** On YouTube, reject candidates whose duration is much larger than a single episode (~21 min here); accept `EP14` (1287 s), reject `EP14-20 FULL` (6805 s), `EP14-39 FULL` (24544 s), `EP01-90 FULL` (81189 s).
4. **Alias discovery:** use each site's own alias/catalog API (`/api/catalog`, `/api/search?q=`) rather than hard-coded title equality; the legacy slugs/routes are stale.

## 4. Suggested fallback order for a future Rust implementation

1. Re-resolve the *same* episode page and take any non-Streamplay player (`DonghuaStreamSource.resolve` already prefers Dailymotion/direct `.mp4`/`.m3u8`). Some episodes of this very series pass here; ep #14 does not.
2. AnimeCube: alias-normalized title + continuous episode number, then resolve via its **new** API (`/api/watch/<code>/context` + player session), not the legacy `/api/anime-sources-versions` endpoint. Expect no result for low-numbered episodes of this series.
3. animekhor.org: title/episode search — currently blocked by bot verification from this host, so treat as "unknown/unavailable" and move on quickly.
4. YouTube: Data API search (already implemented in `youtube_api.rs`) or `ytsearch` as last resort, with duration sanity check and user-visible notice that the replacement is a different uploader.
5. If none match, keep the existing actionable error ("choose another source / try later"); do not fall back to Streamplay extraction.

## 5. Unknowns / limitations

- No Streamplay internals were inspected or bypassed; why some DHS episodes use Streamplay and others Dailymotion is unknown (likely a per-upload choice by the site).
- AnimeCube's current playback source/ticket flow was not reverse-engineered; no direct AnimeCube media URL is claimed.
- animekhor.org may work from a different network/browser; only the 403 Bot Verification result is recorded here.
- YouTube candidate `ZqO6DJl53Sg` was not fully downloaded; metadata + format availability were verified, bytes were not compared.
- Live sites change quickly; all statuses/timestamps are from 2026-11-27 on this machine.
- No repository files other than this report were modified; nothing here is part of 1.1.2.
