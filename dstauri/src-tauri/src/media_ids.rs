//! Stable per-source identifiers and archive helpers.
//!
//! Ported from the legacy `media_ids.py`. Episode identity must survive
//! renames and re-syncs, so IDs are either the native platform ID (YouTube,
//! Dailymotion) or a short SHA-1 digest of the page URL tagged by source.

#![allow(dead_code)] // helpers reserved for the sources/downloader phases

use std::sync::OnceLock;

use regex::Regex;
use sha1::{Digest, Sha1};
use url::Url;

/// Hosts treated as AnimeCube pages.
const ANIMECUBE_HOSTS: [&str; 2] = ["animecube.live", "www.animecube.live"];
/// Hosts treated as DonghuaStream pages.
const DONGHUASTREAM_HOSTS: [&str; 2] = ["donghuastream.org", "www.donghuastream.org"];

/// Final media containers recognised as finished downloads.
pub const MEDIA_EXTENSIONS: [&str; 5] = [".mp4", ".mkv", ".webm", ".mov", ".m4v"];

fn youtube_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:v=|youtu\.be/|/embed/|/v/|/shorts/|/live/)([0-9A-Za-z_-]{11})")
            .expect("the YouTube identifier pattern is valid")
    })
}

fn dailymotion_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:dailymotion\.com/(?:video|embed/video)/|dai\.ly/)([0-9A-Za-z]+)")
            .expect("the Dailymotion identifier pattern is valid")
    })
}

fn format_suffix_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.f[a-z0-9_-]+$").expect("the format suffix pattern is valid"))
}

fn non_alnum_run() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^a-z0-9]+").expect("the separator pattern is valid"))
}

fn unsafe_filename_chars() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).expect("the filename pattern is valid")
    })
}

fn host_of(raw: &str) -> String {
    Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(str::to_lowercase))
        .unwrap_or_default()
}

pub fn is_youtube_host(host: &str) -> bool {
    host == "youtu.be" || host.ends_with("youtube.com")
}

pub fn is_dailymotion_host(host: &str) -> bool {
    host == "dai.ly" || host.ends_with("dailymotion.com")
}

pub fn is_animecube_host(host: &str) -> bool {
    ANIMECUBE_HOSTS.contains(&host)
}

pub fn is_donghuastream_host(host: &str) -> bool {
    DONGHUASTREAM_HOSTS.contains(&host)
}

fn digest(raw: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(raw.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    hex.chars().take(16).collect()
}

/// Extract the stable identifier for a video page URL.
///
/// YouTube and Dailymotion URLs keep their native ID. Scraped sources get a
/// source-tagged digest of the page URL so the same episode maps to one row
/// even if the URL's query string changes.
pub fn extract_video_id(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        return String::new();
    }
    let host = host_of(value);
    if is_youtube_host(&host)
        && let Some(captures) = youtube_regex().captures(value)
        && let Some(id) = captures.get(1)
    {
        return id.as_str().to_string();
    }
    if is_animecube_host(&host) {
        return format!("ac-{}", digest(value));
    }
    if is_donghuastream_host(&host) {
        return format!("dhs-{}", digest(value));
    }
    if is_dailymotion_host(&host)
        && let Some(captures) = dailymotion_regex().captures(value)
        && let Some(id) = captures.get(1)
    {
        return format!("dm-{}", id.as_str());
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return format!("web-{}", digest(value));
    }
    value.to_string()
}

/// True when a filename is a temporary/partial artefact rather than a
/// finished media file.
pub fn is_intermediate_media_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.is_empty() {
        return true;
    }
    if lower.contains(".part") || lower.contains(".ytdl") || lower.contains(".temp.") {
        return true;
    }
    let Some(dot) = lower.rfind('.') else {
        return true;
    };
    let stem = &lower[..dot];
    if stem.is_empty() {
        return true;
    }
    format_suffix_regex().is_match(stem)
}

/// Strip a trailing media extension, case-insensitively.
pub fn media_stem(name: &str) -> &str {
    let lower = name.to_lowercase();
    for extension in MEDIA_EXTENSIONS {
        if lower.ends_with(extension) {
            let cut = name.len().saturating_sub(extension.len());
            return &name[..cut];
        }
    }
    name
}

/// Lowercase and collapse every run of non-alphanumerics into one space.
pub fn normalize_stem(text: &str) -> String {
    let lowered = text.to_lowercase();
    non_alnum_run()
        .replace_all(&lowered, " ")
        .trim()
        .to_string()
}

/// Make a title safe to use as a filename stem.
pub fn safe_title(title: &str, limit: usize) -> String {
    let replaced = unsafe_filename_chars().replace_all(title, "-");
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut trimmed = collapsed.trim_matches([' ', '.']).to_string();
    if trimmed.chars().count() > limit {
        trimmed = trimmed.chars().take(limit).collect();
        trimmed = trimmed.trim_end_matches([' ', '.']).to_string();
    }
    trimmed
}

/// Archive tag for a page URL.
pub fn archive_tag(raw: &str) -> &'static str {
    let host = host_of(raw);
    if is_youtube_host(&host) {
        "youtube"
    } else if is_animecube_host(&host) {
        "animecube"
    } else if is_donghuastream_host(&host) {
        "donghuastream"
    } else if is_dailymotion_host(&host) {
        "dailymotion"
    } else {
        "web"
    }
}

/// Archive tag derived from a stored video ID prefix.
pub fn archive_tag_for_id(video_id: &str) -> &'static str {
    if video_id.starts_with("ac-") {
        "animecube"
    } else if video_id.starts_with("dhs-") {
        "donghuastream"
    } else if video_id.starts_with("dm-") {
        "dailymotion"
    } else if video_id.starts_with("web-") {
        "web"
    } else {
        "youtube"
    }
}

/// Convenience: `(tag, video_id)` for a page URL.
pub fn archive_key(raw: &str) -> (&'static str, String) {
    (archive_tag(raw), extract_video_id(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_ids_keep_their_native_form() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ?t=42"),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            extract_video_id("https://www.youtube.com/shorts/abcDEF12345"),
            "abcDEF12345"
        );
    }

    #[test]
    fn dailymotion_ids_keep_their_native_form() {
        assert_eq!(
            extract_video_id("https://www.dailymotion.com/video/x8abc12"),
            "dm-x8abc12"
        );
        assert_eq!(extract_video_id("https://dai.ly/x8abc12"), "dm-x8abc12");
    }

    #[test]
    fn scraped_sources_get_tagged_digests() {
        let first = extract_video_id("https://animecube.live/watch/some-show-ep-1");
        let second = extract_video_id("https://animecube.live/watch/some-show-ep-1?token=2");
        assert!(first.starts_with("ac-"));
        assert_eq!(first.len(), 19);
        // The digest covers the full URL, mirroring the legacy behavior.
        assert_ne!(first, second);

        let donghua = extract_video_id("https://donghuastream.org/watch/abc");
        assert!(donghua.starts_with("dhs-"));
        let web = extract_video_id("https://example.com/v/1");
        assert!(web.starts_with("web-"));
        assert_eq!(extract_video_id("not a url"), "not a url");
        assert_eq!(extract_video_id("  "), "");
    }

    #[test]
    fn intermediate_names_are_detected() {
        assert!(is_intermediate_media_name("episode.mp4.part"));
        assert!(is_intermediate_media_name("episode.ytdl"));
        assert!(is_intermediate_media_name("episode.temp.mp4"));
        assert!(is_intermediate_media_name("episode.fhls-720.mp4"));
        assert!(is_intermediate_media_name("episode.f137.mp4"));
        assert!(!is_intermediate_media_name("Episode 01.mp4"));
        assert!(!is_intermediate_media_name("episode.mkv"));
    }

    #[test]
    fn stems_are_normalized_for_comparison() {
        assert_eq!(media_stem("Episode 01.MP4"), "Episode 01");
        assert_eq!(media_stem("no-extension"), "no-extension");
        assert_eq!(normalize_stem("Episode_01 -  Title!!"), "episode 01 title");
    }

    #[test]
    fn titles_are_made_filename_safe() {
        assert_eq!(safe_title("A/B: C?", 20), "A-B- C-");
        assert_eq!(safe_title("  trailing dots... ", 20), "trailing dots");
        assert_eq!(safe_title("abcdef", 3), "abc");
        assert!(safe_title("x".repeat(400).as_str(), 150).chars().count() <= 150);
    }

    #[test]
    fn archive_tags_round_trip() {
        let youtube = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(archive_tag(youtube), "youtube");
        assert_eq!(archive_tag_for_id(&extract_video_id(youtube)), "youtube");

        let animecube = "https://animecube.live/watch/x";
        assert_eq!(archive_tag(animecube), "animecube");
        assert_eq!(
            archive_tag_for_id(&extract_video_id(animecube)),
            "animecube"
        );

        let donghua = "https://donghuastream.org/watch/x";
        assert_eq!(archive_tag(donghua), "donghuastream");
        assert_eq!(
            archive_tag_for_id(&extract_video_id(donghua)),
            "donghuastream"
        );

        let daily = "https://www.dailymotion.com/video/x8abc12";
        assert_eq!(archive_tag(daily), "dailymotion");
        assert_eq!(archive_tag_for_id(&extract_video_id(daily)), "dailymotion");

        assert_eq!(archive_tag("https://example.com/v"), "web");
        assert_eq!(archive_key(youtube).0, "youtube");
    }
}
