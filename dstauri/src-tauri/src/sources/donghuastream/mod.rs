//! DonghuaStream episode resolver.
//!
//! Ported from `DonghuaStreamSource.resolve()` in the legacy `sources.py`.
//! The site embeds one or more players (Dailymotion iframes, direct
//! `<video>`/`<source>` tags); the resolver picks the first usable one and
//! normalises Dailymotion embed URLs into canonical video URLs.
//!
//! Browsing (search, trending, genres, series pages) lives in [`browse`].

pub mod browse;

use std::sync::OnceLock;

use regex::Regex;
use url::Url;

use crate::error::AppError;
use crate::media_ids;

use super::{DONGHUASTREAM_BASE, ResolvedMedia};

pub const SOURCE_KEY: &str = "donghuastream";

fn iframe_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<iframe[^>]*>").expect("the iframe pattern is valid"))
}

fn media_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)<(?:video|source)[^>]*>").expect("the media tag pattern is valid")
    })
}

fn h1_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").expect("the h1 pattern is valid"))
}

fn title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("the title pattern is valid")
    })
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<[^>]+>").expect("the tag pattern is valid"))
}

fn dailymotion_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"/(?:embed/)?video/([0-9A-Za-z]+)").expect("the Dailymotion pattern is valid")
    })
}

fn title_separator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+[|–-]\s+").expect("the title separator pattern is valid"))
}

fn whitespace_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").expect("the whitespace pattern is valid"))
}

fn comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").expect("the comment pattern is valid"))
}

/// Drop HTML comments so commented-out players or markup cannot be picked up.
fn strip_comments(html: &str) -> String {
    comment_re().replace_all(html, " ").into_owned()
}

/// Read one attribute value from a single tag, unescaped and trimmed.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"(?i){}\s*=\s*"([^"]*)""#, regex::escape(name));
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(tag)?;
    Some(
        decode_entities(captures.get(1)?.as_str())
            .trim()
            .to_string(),
    )
}

/// Decode the handful of HTML entities the site actually emits.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find('&') {
        out.push_str(&rest[..position]);
        let tail = &rest[position..];
        match tail.find(';').filter(|end| *end <= 10) {
            Some(end) => {
                let entity = &tail[1..end];
                match entity_char(entity) {
                    Some(character) => {
                        out.push(character);
                        rest = &tail[end + 1..];
                    }
                    None => {
                        out.push('&');
                        rest = &tail[1..];
                    }
                }
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity_char(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let code = if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                let decimal = entity.strip_prefix('#')?;
                decimal.parse::<u32>().ok()?
            };
            char::from_u32(code)
        }
    }
}

/// Strip tags, decode entities and collapse whitespace.
fn text_of(html: &str) -> String {
    let without_tags = tag_re().replace_all(html, " ");
    let decoded = decode_entities(&without_tags);
    whitespace_re()
        .replace_all(&decoded, " ")
        .trim()
        .to_string()
}

/// Normalise an episode URL onto the DonghuaStream host.
pub fn episode_url(raw: &str) -> String {
    let text = raw.trim();
    let Ok(parsed) = Url::parse(text) else {
        return text.to_string();
    };
    let mut path = parsed.path().to_string();
    if path.is_empty() {
        path.push('/');
    }
    format!("{DONGHUASTREAM_BASE}{path}")
}

/// Turn a Dailymotion embed/short link into a canonical video URL.
pub fn dailymotion_direct(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    let host = parsed.host_str()?.to_lowercase();

    if host.ends_with("dailymotion.com") {
        if let Some((_, video)) = parsed.query_pairs().find(|(key, _)| key == "video") {
            let video = video.trim();
            if !video.is_empty() {
                return Some(format!("https://www.dailymotion.com/video/{video}"));
            }
        }
        if let Some(captures) = dailymotion_path_re().captures(parsed.path()) {
            return Some(format!(
                "https://www.dailymotion.com/video/{}",
                &captures[1]
            ));
        }
    }

    if host == "dai.ly" {
        let video = parsed.path().trim_matches('/').split('/').next()?;
        if !video.is_empty() {
            return Some(format!("https://www.dailymotion.com/video/{video}"));
        }
    }

    None
}

/// Every player URL the page embeds, in document order.
pub fn players(html: &str) -> Vec<String> {
    let mut found = Vec::new();

    for tag in iframe_tag_re().find_iter(html).map(|m| m.as_str()) {
        for name in ["data-litespeed-src", "data-src", "src"] {
            let Some(value) = attribute(tag, name) else {
                continue;
            };
            if !value.is_empty() && value != "about:blank" {
                found.push(value);
                break;
            }
        }
    }

    for tag in media_tag_re().find_iter(html).map(|m| m.as_str()) {
        if let Some(value) = attribute(tag, "src")
            && !value.is_empty()
            && value != "about:blank"
        {
            found.push(value);
        }
    }

    found
}

/// Choose the best playable URL: Dailymotion first, then a direct media file,
/// then whatever the page embedded first.
pub fn pick_direct(players: &[String]) -> Option<String> {
    for player in players {
        if let Some(direct) = dailymotion_direct(player) {
            return Some(direct);
        }
    }
    for player in players {
        let path = Url::parse(player)
            .map(|url| url.path().to_lowercase())
            .unwrap_or_default();
        if path.ends_with(".mp4") || path.ends_with(".m3u8") || path.ends_with(".webm") {
            return Some(player.clone());
        }
    }
    players.first().cloned()
}

/// Page title, trimmed of the site's `Title | Site` suffix.
pub fn page_title(html: &str) -> String {
    if let Some(captures) = h1_re().captures(html) {
        let title = text_of(&captures[1]);
        if !title.is_empty() {
            return title;
        }
    }
    if let Some(captures) = title_re().captures(html) {
        let raw = text_of(&captures[1]);
        return title_separator_re()
            .split(&raw)
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
    }
    String::new()
}

/// Pure parser: episode page HTML → resolved media target.
pub fn parse_page(html: &str, page_url: &str) -> Result<ResolvedMedia, AppError> {
    let html = strip_comments(html);
    let html = html.as_str();
    let players = players(html);
    if players.is_empty() {
        return Err(AppError::source_with(
            "DonghuaStream did not expose a playable video on this episode page.",
            page_url.to_string(),
        ));
    }

    let direct = pick_direct(&players).ok_or_else(|| {
        AppError::source_with(
            "DonghuaStream did not expose a playable video on this episode page.",
            page_url.to_string(),
        )
    })?;

    let platform = if dailymotion_direct(&direct).is_some() {
        "dailymotion"
    } else {
        ""
    };

    let title = page_title(html);
    let title = if title.is_empty() {
        "DonghuaStream episode".to_string()
    } else {
        title
    };

    Ok(ResolvedMedia {
        is_hls: super::is_hls_url(&direct),
        url: direct,
        title,
        source: SOURCE_KEY.to_string(),
        video_id: media_ids::extract_video_id(page_url),
        platform: platform.to_string(),
    })
}

/// Fetch and resolve one episode page.
pub async fn resolve(page_url: &str) -> Result<ResolvedMedia, AppError> {
    let url = episode_url(page_url);
    let html = super::fetch_text(&url).await?;
    parse_page(&html, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"
        <html><head><title>Xin Xia Episode 1 | DonghuaStream</title></head>
        <body>
          <h1 class="entry-title">Xin Xia Episode 1</h1>
          <div class="player">
            <iframe data-litespeed-src="about:blank"
                    data-src="https://www.dailymotion.com/embed/video/x8abc12?autoplay=1"></iframe>
          </div>
        </body></html>
    "#;

    #[test]
    fn episode_urls_normalise_onto_the_source_host() {
        assert_eq!(
            episode_url("https://donghuastream.org/xin-xia-episode-1/?x=1"),
            "https://donghuastream.org/xin-xia-episode-1/"
        );
        assert_eq!(
            episode_url("donghuastream.org/xin-xia-episode-1/"),
            "donghuastream.org/xin-xia-episode-1/"
        );
    }

    #[test]
    fn dailymotion_embeds_become_canonical_urls() {
        assert_eq!(
            dailymotion_direct("https://www.dailymotion.com/embed/video/x8abc12?autoplay=1"),
            Some("https://www.dailymotion.com/video/x8abc12".to_string())
        );
        assert_eq!(
            dailymotion_direct("https://www.dailymotion.com/player.html?video=x8abc12"),
            Some("https://www.dailymotion.com/video/x8abc12".to_string())
        );
        assert_eq!(
            dailymotion_direct("https://dai.ly/x8abc12"),
            Some("https://www.dailymotion.com/video/x8abc12".to_string())
        );
        assert_eq!(dailymotion_direct("https://example.com/video.mp4"), None);
    }

    #[test]
    fn players_skip_blank_attributes_and_pick_the_first_real_one() {
        let found = players(PAGE);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("dailymotion.com/embed/video/x8abc12"));
    }

    #[test]
    fn direct_files_are_preferred_over_unknown_players() {
        let found = vec![
            "https://example.com/embed/1".to_string(),
            "https://cdn.example.com/master.m3u8".to_string(),
        ];
        assert_eq!(
            pick_direct(&found),
            Some("https://cdn.example.com/master.m3u8".to_string())
        );
    }

    #[test]
    fn a_resolved_page_reports_title_platform_and_hls() {
        let media = parse_page(PAGE, "https://donghuastream.org/xin-xia-episode-1/")
            .expect("the page resolves");
        assert_eq!(media.title, "Xin Xia Episode 1");
        assert_eq!(media.source, "donghuastream");
        assert_eq!(media.platform, "dailymotion");
        assert_eq!(media.url, "https://www.dailymotion.com/video/x8abc12");
        assert!(media.video_id.starts_with("dhs-"));
        assert!(!media.is_hls);

        let hls = parse_page(
            r#"<h1>Ep</h1><video src="https://cdn.example/master.m3u8"></video>"#,
            "https://donghuastream.org/ep/",
        )
        .expect("the HLS page resolves");
        assert!(hls.is_hls);
    }

    #[test]
    fn pages_without_a_player_are_reported_clearly() {
        let error = parse_page("<h1>Nothing</h1>", "https://donghuastream.org/x/")
            .expect_err("no player means an error");
        assert!(error.message().contains("playable video"));
    }

    #[test]
    fn entities_are_decoded_in_titles() {
        assert_eq!(page_title("<title>A &amp; B &#8211; Site</title>"), "A & B");
        assert_eq!(text_of("<h1>Hello&nbsp;<b>World</b></h1>"), "Hello World");
    }

    /// Fixture trimmed from a live episode page (2026-11-23). The real site
    /// serves the player through `geo.dailymotion.com/player/…?video=…` inside
    /// `data-litespeed-src`, with `src="about:blank"` placeholder first.
    #[test]
    fn a_live_episode_fixture_resolves_to_dailymotion() {
        let html = include_str!("../../../tests/fixtures/donghuastream_episode.html");
        let media = parse_page(
            html,
            "https://donghuastream.org/against-the-sky-supreme-episode-539to540-subtitles/",
        )
        .expect("the live page resolves");

        assert_eq!(
            media.url,
            "https://www.dailymotion.com/video/k6aiI8s8MuqFykIKj66"
        );
        assert_eq!(media.platform, "dailymotion");
        assert_eq!(
            media.title,
            "Against the Sky Supreme Episode 539to540 Subtitles"
        );
        assert!(media.video_id.starts_with("dhs-"));
        assert!(!media.is_hls);
    }
}
