//! Resolve a wowtpix.com episode page to a playable media URL.
//!
//! The theme exposes a player through `data-video`, a `meta itemprop=embedUrl`,
//! an iframe, or a base64-encoded mirror option. Dailymotion targets are
//! normalised to their canonical `/video/<id>` page, which yt-dlp understands;
//! direct MP4/HLS targets are passed through. Nothing is persisted.

use base64::Engine;
use regex::Regex;
use url::Url;

use crate::error::AppError;
use crate::sources::{ResolvedMedia, absolute_media_url, fetch_text, is_hls_url};

use super::{SOURCE_KEY, is_wowtpix_host};

const NAME: &str = "wowtpix.com";

fn rx(pattern: &str) -> Option<Regex> {
    Regex::new(pattern).ok()
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn strip_comments(html: &str) -> String {
    rx(r"(?s)<!--.*?-->")
        .map(|re| re.replace_all(html, " ").into_owned())
        .unwrap_or_else(|| html.to_string())
}

fn text_of(input: &str) -> String {
    let without_tags = rx(r"(?is)<[^>]+>")
        .map(|re| re.replace_all(input, " ").to_string())
        .unwrap_or_else(|| input.to_string());
    let decoded = decode_entities(&without_tags);
    rx(r"\s+")
        .map(|re| re.replace_all(decoded.trim(), " ").to_string())
        .unwrap_or_else(|| decoded.trim().to_string())
}

fn attr_of(tag: &str, name: &str) -> String {
    let pattern = format!(r#"(?is)\b{}\s*=\s*"([^"]*)""#, regex::escape(name));
    rx(&pattern)
        .and_then(|re| {
            re.captures(tag)
                .map(|caps| decode_entities(&caps[1]).trim().to_string())
        })
        .unwrap_or_default()
}

/// Restrict player discovery to the embed block when the page has one; the
/// theme also carries advertising iframes elsewhere in the document.
fn player_scope(html: &str) -> &str {
    rx(r#"(?is)<div\s+id="embed_holder".*?</div>"#)
        .and_then(|re| re.find(html).map(|found| found.as_str()))
        .unwrap_or(html)
}

/// Turn a Dailymotion embed/player/short URL into the canonical video page.
pub fn dailymotion_direct(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    if host == "dai.ly" {
        let id = parsed.path().trim_matches('/').split('/').next()?;
        if !id.is_empty() && id.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return Some(format!("https://www.dailymotion.com/video/{id}"));
        }
        return None;
    }
    if !host.ends_with("dailymotion.com") {
        return None;
    }
    if let Some((_, id)) = parsed
        .query_pairs()
        .find(|(key, _)| key.eq_ignore_ascii_case("video"))
    {
        let id = id.trim();
        if !id.is_empty() && id.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return Some(format!("https://www.dailymotion.com/video/{id}"));
        }
    }
    let path = rx(r"/(?:embed/)?video/([0-9A-Za-z]+)")?;
    path.captures(parsed.path())
        .map(|caps| format!("https://www.dailymotion.com/video/{}", &caps[1]))
}

fn internal_page(raw: &str) -> bool {
    Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| is_wowtpix_host(&host))
}

/// Base64 mirror values on this theme decode to a full iframe tag.
fn decoded_options(html: &str) -> Vec<String> {
    let Some(option) = rx(r#"(?is)<option\b[^>]*value="([^"]+)"[^>]*>"#) else {
        return Vec::new();
    };
    option
        .captures_iter(html)
        .filter_map(|caps| {
            let value = caps[1].trim();
            if value.len() < 40
                || !value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/' || byte == b'='
                })
            {
                return None;
            }
            base64::engine::general_purpose::STANDARD
                .decode(value)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .filter(|decoded| decoded.contains('<'))
        })
        .collect()
}

fn media_tag_values(fragment: &str, out: &mut Vec<String>) {
    let Some(tags) = rx(r#"(?is)<(?:iframe|video|source)\b[^>]*>"#) else {
        return;
    };
    for tag in tags.find_iter(fragment) {
        let tag = tag.as_str();
        for name in ["data-litespeed-src", "data-src", "src"] {
            let value = attr_of(tag, name);
            if !value.is_empty() && value != "about:blank" {
                out.push(value);
                break;
            }
        }
    }
}

/// Raw candidate URLs in priority order, before normalisation.
pub fn player_candidates(html: &str) -> Vec<String> {
    let html = strip_comments(html);
    let scope = player_scope(&html);
    let mut out = Vec::new();

    // The player script carries the canonical Dailymotion id.
    if let Some(re) = rx(r#"(?is)data-video\s*=\s*"([0-9A-Za-z]+)""#) {
        for caps in re.captures_iter(&html) {
            out.push(format!("https://www.dailymotion.com/video/{}", &caps[1]));
        }
    }
    if let Some(re) = rx(r#"(?is)<meta[^>]*itemprop="embedUrl"[^>]*>"#) {
        for tag in re.find_iter(scope) {
            let value = attr_of(tag.as_str(), "content");
            if !value.is_empty() {
                out.push(value);
            }
        }
    }
    media_tag_values(scope, &mut out);
    for decoded in decoded_options(&html) {
        media_tag_values(&decoded, &mut out);
    }
    if let Some(re) = rx(r#"(?is)<a\b[^>]*href="([^"]+\.(?:mp4|m3u8|webm)[^"]*)"[^>]*>"#) {
        for caps in re.captures_iter(&html) {
            out.push(caps[1].to_string());
        }
    }
    if let Some(re) = rx(r#"(?is)<meta[^>]*itemprop="contentUrl"[^>]*>"#) {
        for tag in re.find_iter(scope) {
            let value = attr_of(tag.as_str(), "content");
            if !value.is_empty() {
                out.push(value);
            }
        }
    }
    out
}

fn normalise(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("about:blank") || raw.starts_with("javascript:") {
        return None;
    }
    if let Some(direct) = dailymotion_direct(raw) {
        return Some(direct);
    }
    let absolute = absolute_media_url(raw);
    let parsed = Url::parse(&absolute).ok()?;
    if internal_page(&absolute) {
        // A `/v/1/` mirror page is not media; resolving it would need a second
        // fetch, so skip it in favour of an already-embedded target.
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    let is_media = path.ends_with(".mp4")
        || path.ends_with(".m3u8")
        || path.ends_with(".webm")
        || path.contains(".m3u8?");
    let known_host = host == "dai.ly"
        || host.ends_with(".dailymotion.com")
        || host == "ok.ru"
        || host.ends_with(".ok.ru")
        || host.ends_with(".odysee.com")
        || host.ends_with(".rumble.com")
        || host.ends_with(".youtube.com")
        || host == "youtu.be";
    if is_media || known_host || path.contains("/embed/") || path.contains("/video/") {
        Some(absolute)
    } else {
        None
    }
}

fn platform_of(url: &str) -> String {
    if dailymotion_direct(url).is_some() {
        return "dailymotion".to_string();
    }
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default()
}

/// Pure resolver: episode HTML → playable target.
pub fn resolved_from_html(html: &str, page_url: &str) -> Result<ResolvedMedia, AppError> {
    let candidates = player_candidates(html);
    let url = candidates
        .iter()
        .find_map(|candidate| normalise(candidate))
        .ok_or_else(|| {
            AppError::source_with(
                format!("{NAME} did not expose a playable video on this episode page."),
                page_url.to_string(),
            )
        })?;
    let title = rx(r#"(?is)<h1[^>]*class="entry-title"[^>]*>(.*?)</h1>"#)
        .and_then(|re| re.captures(html).map(|caps| text_of(&caps[1])))
        .unwrap_or_else(|| format!("{NAME} episode"));
    let platform = platform_of(&url);
    let is_hls = is_hls_url(&url);
    Ok(ResolvedMedia {
        is_hls,
        url,
        title,
        source: SOURCE_KEY.to_string(),
        video_id: crate::media_ids::extract_video_id(page_url),
        platform,
    })
}

/// Fetch the episode page at play/download time, then resolve it.
pub async fn resolve(page_url: &str) -> Result<ResolvedMedia, AppError> {
    if page_url.trim().is_empty() {
        return Err(AppError::source_with(
            format!("{NAME} needs an episode page URL to play this video."),
            page_url.to_string(),
        ));
    }
    let html = fetch_text(page_url).await?;
    resolved_from_html(&html, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPISODE: &str = include_str!("../../../tests/fixtures/wowtpix_episode.html");
    const PAGE: &str = "https://wowtpix.com/renegade-immortal-episode-2/";

    #[test]
    fn dailymotion_embeds_become_canonical_video_urls() {
        assert_eq!(
            dailymotion_direct(
                "https://geo.dailymotion.com/player/xdcdz.html?video=k7ayn9Fhrmec9INb7bs"
            ),
            Some("https://www.dailymotion.com/video/k7ayn9Fhrmec9INb7bs".to_string())
        );
        assert_eq!(
            dailymotion_direct("https://www.dailymotion.com/embed/video/x8abc12?autoplay=1"),
            Some("https://www.dailymotion.com/video/x8abc12".to_string())
        );
        assert_eq!(
            dailymotion_direct("https://dai.ly/x8abc12"),
            Some("https://www.dailymotion.com/video/x8abc12".to_string())
        );
        assert_eq!(dailymotion_direct("https://example.com/video.mp4"), None);
    }

    #[test]
    fn player_page_resolves_with_a_dailymotion_platform_tag() {
        let media = resolved_from_html(EPISODE, PAGE).expect("synthetic episode fixture");
        assert_eq!(
            media.url,
            "https://www.dailymotion.com/video/k7ayn9Fhrmec9INb7bs"
        );
        assert_eq!(media.source, SOURCE_KEY);
        assert_eq!(media.platform, "dailymotion");
        assert!(!media.is_hls);
        assert_eq!(media.title, "Renegade Immortal Episode 2 English Sub");
        assert!(!media.video_id.is_empty());
    }

    #[test]
    fn direct_hls_streams_are_tagged_as_hls() {
        let html =
            r#"<div id="embed_holder"><video src="//cdn.example.com/master.m3u8"></video></div>"#;
        let media = resolved_from_html(html, PAGE).expect("direct HLS");
        assert_eq!(media.url, "https://cdn.example.com/master.m3u8");
        assert!(media.is_hls);
        assert_eq!(media.platform, "cdn.example.com");
    }

    #[test]
    fn a_page_without_a_player_fails_with_a_copyable_error() {
        let error = resolved_from_html("<html><body>no video</body></html>", PAGE).unwrap_err();
        assert!(error.message().contains("did not expose a playable video"));
        assert_eq!(error.detail(), PAGE);
    }

    #[test]
    fn internal_mirror_links_are_not_mistaken_for_media() {
        let html = r#"<div id="embed_holder">
          <option value="https://wowtpix.com/renegade-immortal-episode-2/v/1/">Server 1</option>
          <iframe src="https://ok.ru/videoembed/12345"></iframe>
        </div>"#;
        let media = resolved_from_html(html, PAGE).expect("external player");
        assert_eq!(media.url, "https://ok.ru/videoembed/12345");
        assert_eq!(media.platform, "ok.ru");
    }
}
