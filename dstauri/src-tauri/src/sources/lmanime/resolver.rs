//! Resolve an lmanime.com episode page to a direct playable media URL.
//!
//! Live episode pages put the player in `<div class="player-embed"><iframe
//! src="https://geo.dailymotion.com/player/<player>.html?video=<id>">`, and
//! offer mirrors as `<select class="mirror"><option value="…">`. The Dailymotion
//! player URL is normalised to a canonical `dailymotion.com/video/<id>` URL so
//! yt-dlp's Dailymotion extractor handles it directly.

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
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#8217;", "’")
        .replace("&#187;", "»")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// Dailymotion player/embed URLs → canonical video URL plus platform tag.
pub fn dailymotion_direct(raw: &str) -> Option<(String, String)> {
    let lower = raw.to_ascii_lowercase();
    if !lower.contains("dailymotion") && !lower.contains("dai.ly") {
        return None;
    }
    let video = re(r#"(?i)[?&]video=([0-9A-Za-z]+)"#)
        .and_then(|regex| regex.captures(raw))
        .map(|caps| caps[1].to_string())
        .or_else(|| {
            re(r"(?i)(?:dailymotion\.com/(?:embed/)?video/|dai\.ly/)([0-9A-Za-z]+)")
                .and_then(|regex| regex.captures(raw))
                .map(|caps| caps[1].to_string())
        })?;
    Some((
        format!("https://www.dailymotion.com/video/{video}"),
        "dailymotion".to_string(),
    ))
}

/// Player URLs in document order: iframe embed first, then mirror options.
pub fn players(html: &str) -> Vec<String> {
    let Some(attr) =
        re(r#"(?is)\b(?:data-litespeed-src|data-src|src|value)\s*=\s*['"]([^'"]+)['"]"#)
    else {
        return Vec::new();
    };
    // Only inspect the player embed and the mirror select: the rest of the
    // page is full of ad/script URLs that are not playable media.
    let Some(scopes) = re(r#"(?is)<div\s+class="player-embed".*?</select>"#) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for scope in scopes.captures_iter(html) {
        for caps in attr.captures_iter(&scope[0]) {
            let value = decode_entities(&caps[1]).trim().to_string();
            if value.is_empty()
                || value.eq_ignore_ascii_case("about:blank")
                || value.starts_with("data:")
                || !value.contains("://") && !value.starts_with("//")
            {
                continue;
            }
            if !found.contains(&value) {
                found.push(value);
            }
        }
    }
    if found.is_empty() {
        // Some layouts omit the mirror select; fall back to every iframe.
        if let Some(iframes) = re(r#"(?is)<iframe\b[^>]*>"#) {
            for tag in iframes.find_iter(html) {
                for caps in attr.captures_iter(tag.as_str()) {
                    let value = decode_entities(&caps[1]).trim().to_string();
                    if !value.is_empty()
                        && value != "about:blank"
                        && !value.starts_with("data:")
                        && !found.contains(&value)
                    {
                        found.push(value);
                    }
                }
            }
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
            raw.split(" » ").next().unwrap_or(&raw).trim().to_string()
        })
        .unwrap_or_default()
}

/// Pure parser: episode page HTML → resolved media.
pub fn resolved_from_html(html: &str, page_url: &str) -> Result<ResolvedMedia, AppError> {
    for player in players(html) {
        let url = absolute_media_url(&player);
        let (direct, platform) = if let Some((direct, platform)) = dailymotion_direct(&url) {
            (direct, platform)
        } else {
            let lower = url.to_ascii_lowercase();
            let playable = lower.contains(".m3u8")
                || lower.contains(".mp4")
                || lower.contains(".webm")
                || lower.contains(".mkv");
            if !playable {
                continue;
            }
            (url, String::new())
        };
        let title = {
            let found = page_title(html);
            if found.is_empty() {
                "LMAnime episode".to_string()
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
    Err(AppError::source_with(
        "lmanime.com did not expose a playable video on this episode page.",
        page_url.to_string(),
    ))
}

/// Fetch the episode page at play/download time, then resolve it.
pub async fn resolve(page_url: &str) -> Result<ResolvedMedia, AppError> {
    let html = fetch_text(page_url).await?;
    resolved_from_html(&html, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: &str = include_str!("../../../tests/fixtures/lmanime_episode_player.html");

    #[test]
    fn a_live_dailymotion_embed_resolves_to_a_direct_url() {
        let media = resolved_from_html(
            PLAYER,
            "https://lmanime.com/a-mortals-journey-to-immortality-season-05-episode-23-199/",
        )
        .expect("the live player resolves");
        assert_eq!(
            media.url,
            "https://www.dailymotion.com/video/k3iNx8oYScWEJbNaW10"
        );
        assert_eq!(media.platform, "dailymotion");
        assert_eq!(media.source, SOURCE_KEY);
        assert_eq!(
            media.title,
            "A Mortal’s Journey to Immortality Season 05 Episode 23 (199)"
        );
        assert!(!media.is_hls);
    }

    #[test]
    fn mirror_links_are_not_confused_with_media() {
        let found = players(PLAYER);
        assert!(found[0].contains("geo.dailymotion.com/player"));
        assert!(found.iter().any(|url| url.contains("/v/1/")));
    }

    #[test]
    fn dailymotion_embed_variants_normalise() {
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
    fn a_page_without_a_player_reports_clearly() {
        let error = resolved_from_html("<h1>Nothing</h1>", "https://lmanime.com/ep/")
            .expect_err("no player means an error");
        assert!(error.message().contains("playable video"));
    }

    #[test]
    fn hls_sources_are_flagged() {
        let page =
            r#"<div class="player-embed"><iframe src="//cdn.example/master.m3u8"></iframe></div>"#;
        let media =
            resolved_from_html(page, "https://lmanime.com/ep/").expect("the HLS source resolves");
        assert!(media.is_hls);
        assert_eq!(media.url, "https://cdn.example/master.m3u8");
    }
}
