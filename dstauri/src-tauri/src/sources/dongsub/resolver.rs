//! Resolve a dongsub.net episode page to a direct playable media URL.
//!
//! The live page embeds an empty `<iframe src="">` and fills it from
//! `<option value="…">` inside `<select id="selectServ">`. The first real
//! option is normally `id="servDefault"` and points at a Dailymotion player
//! (`geo.dailymotion.com/player/<player>.html?video=<id>`).

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

/// Every non-empty server URL the page offers, in document order.
pub fn servers(html: &str) -> Vec<String> {
    let Some(option) = re(r#"(?is)<option\b[^>]*\bvalue\s*=\s*['"]([^'"]+)['"][^>]*>"#) else {
        return Vec::new();
    };
    let Some(iframe) = re(r#"(?is)<iframe\b[^>]*>"#) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for caps in option.captures_iter(html) {
        let value = decode_entities(&caps[1]).trim().to_string();
        if value.is_empty()
            || value.eq_ignore_ascii_case("about:blank")
            || value.starts_with("data:")
        {
            continue;
        }
        if !found.contains(&value) {
            found.push(value);
        }
    }
    for caps in iframe.captures_iter(html) {
        let value = {
            let tag = &caps[0];
            let lazy = re(r#"(?is)\bdata-(?:litespeed-)?src\s*=\s*['"]([^'"]+)['"]"#)
                .and_then(|regex| regex.captures(tag))
                .map(|found| decode_entities(&found[1]).trim().to_string())
                .unwrap_or_default();
            if lazy.is_empty() {
                re(r#"(?is)\bsrc\s*=\s*['"]([^'"]+)['"]"#)
                    .and_then(|regex| regex.captures(tag))
                    .map(|found| decode_entities(&found[1]).trim().to_string())
                    .unwrap_or_default()
            } else {
                lazy
            }
        };
        if value.is_empty()
            || value.eq_ignore_ascii_case("about:blank")
            || value.starts_with("data:")
        {
            continue;
        }
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
            raw.split(" - ").next().unwrap_or(&raw).trim().to_string()
        })
        .unwrap_or_default()
}

/// Pure parser: episode page HTML → resolved media.
pub fn resolved_from_html(html: &str, page_url: &str) -> Result<ResolvedMedia, AppError> {
    for server in servers(html) {
        let url = absolute_media_url(&server);
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
                "Dongsub episode".to_string()
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
        "dongsub.net did not expose a playable video on this episode page.",
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

    const PLAYER: &str = include_str!("../../../tests/fixtures/dongsub_episode_player.html");

    #[test]
    fn a_live_server_option_resolves_to_dailymotion() {
        let media = resolved_from_html(
            PLAYER,
            "https://www.dongsub.net/2026/11/renegade-immortal-episode-168-subtitles.html",
        )
        .expect("the live player resolves");
        assert_eq!(
            media.url,
            "https://www.dailymotion.com/video/k7upvQNbFinl9LMYn4i"
        );
        assert_eq!(media.platform, "dailymotion");
        assert_eq!(media.source, SOURCE_KEY);
        assert!(!media.is_hls);
    }

    #[test]
    fn blank_iframes_and_placeholder_options_are_skipped() {
        let found = servers(PLAYER);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("geo.dailymotion.com/player"));
    }

    #[test]
    fn embed_variants_normalise_to_a_video_url() {
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
    fn a_page_without_a_server_reports_clearly() {
        let error = resolved_from_html("<h1>No player</h1>", "https://www.dongsub.net/x/")
            .expect_err("no player means an error");
        assert!(error.message().contains("playable video"));
    }

    #[test]
    fn hls_mirrors_are_flagged() {
        let page = r#"<select><option value="//cdn.example/master.m3u8">S1</option></select>"#;
        let media =
            resolved_from_html(page, "https://www.dongsub.net/2026/11/ep.html").expect("hls");
        assert!(media.is_hls);
        assert_eq!(media.url, "https://cdn.example/master.m3u8");
        assert!(media.platform.is_empty());
    }
}
