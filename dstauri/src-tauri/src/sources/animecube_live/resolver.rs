//! Resolve an animecube.live episode page to a direct playable media URL.
//!
//! Evidence captured 2026-11-28: `/watch/<code>` HTML contains no direct media
//! URL; `/api/anime-sources-versions` answers HTTP 410 and
//! `/api/watch/<code>/sources` answers HTTP 403 for anonymous requests, while
//! `/api/download/ticket` answers HTTP 401. The resolver therefore only
//! returns a target when the page itself exposes one (old embeds, mirrors or
//! future layout changes); otherwise it fails early with a copyable, friendly
//! error instead of handing a raw page to yt-dlp (which reports
//! `Unsupported URL`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::Regex;

use crate::error::AppError;
use crate::media_ids;
use crate::sources::{ResolvedMedia, absolute_media_url, fetch_text, is_hls_url};

use super::SOURCE_KEY;

fn re(pattern: &str) -> Option<Regex> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Regex>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = match map.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(cached) = map.get(pattern) {
        return cached.clone();
    }
    let compiled = Regex::new(pattern).ok();
    map.insert(pattern.to_string(), compiled.clone());
    compiled
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&#x27;", "'")
        .replace("&#039;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// `geo.dailymotion.com/player/<player>.html?video=<id>`,
/// `dailymotion.com/embed/video/<id>`, `dai.ly/<id>` and plain
/// `dailymotion.com/video/<id>` all become one canonical, yt-dlp-friendly URL.
pub fn dailymotion_direct(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    let video = re(r#"(?i)[?&]video=([0-9A-Za-z]+)"#)
        .and_then(|regex| regex.captures(trimmed))
        .map(|caps| caps[1].to_string())
        .or_else(|| {
            re(r"(?i)(?:dailymotion\.com/(?:embed/)?video/|dai\.ly/)([0-9A-Za-z]+)")
                .and_then(|regex| regex.captures(trimmed))
                .map(|caps| caps[1].to_string())
        })?;
    if !trimmed.to_ascii_lowercase().contains("dailymotion")
        && !trimmed.to_ascii_lowercase().contains("dai.ly")
    {
        return None;
    }
    Some((
        format!("https://www.dailymotion.com/video/{video}"),
        "dailymotion".to_string(),
    ))
}

/// Candidate media URLs in document order: embeds, `<video>/<source>` tags,
/// lazy-loaded attributes and JSON `url` fields from the page's RSC payload.
pub fn media_candidates(html: &str) -> Vec<String> {
    let Some(attr) = re(r#"(?is)\b(?:src|data-src|data-video|data-url|value)\s*=\s*"([^"]+)""#)
    else {
        return Vec::new();
    };
    let Some(json_url) = re(r#"(?i)\\?"url\\?"\s*:\s*\\?"(https?:[^"\\]+)""#) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for caps in attr.captures_iter(html) {
        let value = decode_entities(&caps[1]).trim().to_string();
        if value.is_empty() || value == "about:blank" || value.starts_with("data:") {
            continue;
        }
        if !found.contains(&value) {
            found.push(value);
        }
    }
    for caps in json_url.captures_iter(html) {
        let value = decode_entities(&caps[1]).replace("\\/", "/");
        if !found.contains(&value) {
            found.push(value);
        }
    }
    found
}

fn page_title(html: &str) -> String {
    let Some(title) = re(r"(?is)<title[^>]*>(.*?)</title>") else {
        return String::new();
    };
    title
        .captures(html)
        .map(|caps| {
            let raw = decode_entities(&caps[1]);
            raw.split(" | ").next().unwrap_or(&raw).trim().to_string()
        })
        .unwrap_or_default()
}

fn protected_error(page_url: &str) -> AppError {
    AppError::source_with(
        "AnimeCube.live does not expose a playable URL in its episode page. Playback sources are fetched from a protected API that needs a signed-in session; choose another source or sign in on the site first.",
        format!("requested page: {page_url}"),
    )
}

/// Pure parser: episode page HTML → resolved media.
pub fn resolved_from_html(html: &str, page_url: &str) -> Result<ResolvedMedia, AppError> {
    for candidate in media_candidates(html) {
        let url = absolute_media_url(&candidate);
        let (platform, direct) = if let Some((direct, platform)) = dailymotion_direct(&url) {
            (platform, direct)
        } else {
            let lower = url.to_ascii_lowercase();
            let playable = lower.contains(".m3u8")
                || lower.contains(".mp4")
                || lower.contains(".webm")
                || lower.contains(".mkv");
            if !playable {
                continue;
            }
            (String::new(), url)
        };
        let title = {
            let found = page_title(html);
            if found.is_empty() {
                "AnimeCube episode".to_string()
            } else {
                found
            }
        };
        return Ok(ResolvedMedia {
            is_hls: is_hls_url(&direct),
            url: direct,
            title,
            source: SOURCE_KEY.to_string(),
            video_id: media_ids::extract_video_id(page_url),
            platform,
        });
    }
    Err(protected_error(page_url))
}

/// Fetch the episode page at play/download time, then resolve it.
pub async fn resolve(page_url: &str) -> Result<ResolvedMedia, AppError> {
    let html = fetch_text(page_url).await?;
    resolved_from_html(&html, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `/watch/ckvqqzn` page captured 2026-11-28 (player section). It
    /// carries episode metadata but no direct media URL.
    const LIVE_PAGE: &str = include_str!("../../../tests/fixtures/animecube_live_episode.html");

    #[test]
    fn dailymotion_embeds_are_normalised() {
        assert_eq!(
            dailymotion_direct(
                "https://geo.dailymotion.com/player/x110cm.html?video=k7upvQNbFinl9LMYn4i"
            ),
            Some((
                "https://www.dailymotion.com/video/k7upvQNbFinl9LMYn4i".to_string(),
                "dailymotion".to_string()
            ))
        );
        assert_eq!(
            dailymotion_direct("https://www.dailymotion.com/embed/video/x8abc12?autoplay=1"),
            Some((
                "https://www.dailymotion.com/video/x8abc12".to_string(),
                "dailymotion".to_string()
            ))
        );
        assert_eq!(
            dailymotion_direct("https://dai.ly/x8abc12"),
            Some((
                "https://www.dailymotion.com/video/x8abc12".to_string(),
                "dailymotion".to_string()
            ))
        );
        assert!(dailymotion_direct("https://example.com/video/x8abc12").is_none());
    }

    #[test]
    fn an_old_or_future_embed_still_resolves() {
        let page = r#"
            <html><head><title>Renegade Immortal Episode 1 | Anime Cube</title></head>
            <body><iframe src="https://geo.dailymotion.com/player/x110cm.html?video=k7upvQNbFinl9LMYn4i"></iframe></body>
        </html>"#;
        let media = resolved_from_html(page, "https://animecube.live/watch/ckvqqzn")
            .expect("the Dailymotion embed resolves");
        assert_eq!(
            media.url,
            "https://www.dailymotion.com/video/k7upvQNbFinl9LMYn4i"
        );
        assert_eq!(media.platform, "dailymotion");
        assert_eq!(media.source, SOURCE_KEY);
        assert!(media.video_id.starts_with("ac-"));
        assert!(!media.is_hls);
    }

    #[test]
    fn hls_sources_are_flagged() {
        let page = r#"<h1>Ep</h1><video src="https://cdn.example/master.m3u8"></video>"#;
        let media = resolved_from_html(page, "https://animecube.live/watch/abc")
            .expect("the HLS source resolves");
        assert!(media.is_hls);
        assert_eq!(media.url, "https://cdn.example/master.m3u8");
    }

    #[test]
    fn the_live_protected_page_fails_with_a_copyable_error() {
        let page = format!("https://{}/watch/ckvqqzn", "animecube.live");
        let error =
            resolved_from_html(LIVE_PAGE, &page).expect_err("no direct URL on the live page");
        assert!(error.message().contains("protected API"));
        assert!(error.detail().contains(&page));
    }
}
