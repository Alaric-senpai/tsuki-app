//! Quality presets → yt-dlp format selectors.
//!
//! Ported from `resolve_format_string()` / `_format_for_height()` in the
//! legacy `downloader.py`. The selector deliberately caps the height
//! (`height<=N`) so a source without an exact rendition still downloads at the
//! best available stream below the cap instead of failing.

/// Map a user quality value to a yt-dlp format string and a display label.
pub fn resolve_format(quality: &str) -> (String, String) {
    let value = quality.trim().to_lowercase();
    if matches!(value.as_str(), "best" | "max" | "highest" | "auto" | "none") {
        return (
            "bestvideo+bestaudio/best".to_string(),
            "Best Available".to_string(),
        );
    }
    let cleaned = value.strip_suffix('p').unwrap_or(&value);
    match cleaned.parse::<u32>() {
        Ok(height) if height > 0 => (format_for_height(height), format!("{height}p")),
        _ => (format_for_height(720), "720p (fallback)".to_string()),
    }
}

/// Prefer the exact height, then the best stream at or below it.
pub fn format_for_height(height: u32) -> String {
    let capped = height.max(1);
    format!(
        "bestvideo[height={capped}]+bestaudio/\
         bestvideo[height<={capped}]+bestaudio/\
         best[height={capped}]/\
         best[height<={capped}]/\
         best"
    )
}

/// Container yt-dlp may merge into. Unknown values fall back to `mp4`.
pub fn merge_container(container: &str) -> &'static str {
    match container.trim().to_lowercase().as_str() {
        "mkv" => "mkv",
        "webm" => "webm",
        "mov" => "mov",
        _ => "mp4",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cap_at_the_requested_height() {
        assert_eq!(
            resolve_format("720p"),
            (format_for_height(720), "720p".to_string())
        );
        assert!(format_for_height(720).contains("bestvideo[height<=720]+bestaudio"));
        assert!(format_for_height(720).starts_with("bestvideo[height=720]+bestaudio/"));
    }

    #[test]
    fn best_available_has_no_cap() {
        assert_eq!(
            resolve_format("best"),
            (
                "bestvideo+bestaudio/best".to_string(),
                "Best Available".to_string()
            )
        );
    }

    #[test]
    fn integers_and_unknown_values_are_handled() {
        assert_eq!(resolve_format("1080").0, format_for_height(1080));
        assert_eq!(resolve_format("1080P").1, "1080p");
        assert_eq!(
            resolve_format("ultra"),
            (format_for_height(720), "720p (fallback)".to_string())
        );
    }

    #[test]
    fn containers_are_restricted_to_supported_values() {
        assert_eq!(merge_container("MKV"), "mkv");
        assert_eq!(merge_container("avi"), "mp4");
        assert_eq!(merge_container(""), "mp4");
    }
}
