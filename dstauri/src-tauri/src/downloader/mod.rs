//! Download engine: yt-dlp process driver, sequential queue, archive.
//!
//! Ported from the legacy `downloader.py`. The frontend never spawns
//! processes; it enqueues jobs and listens to `download://state` and
//! `download://console` events.

pub mod archive;
pub mod disk;
pub mod quality;
pub mod queue;
pub mod updater;
pub mod ytdlp;

pub use queue::{DownloadManager, DownloadSnapshot, Job, TRAY_ID};

/// Values the UI uses to mean "do not import browser cookies".
const DISABLED_BROWSERS: [&str; 6] = ["", "none", "no", "off", "disabled", "false"];

/// Turn the UI's "No cookies" sentinel into `None` for yt-dlp.
///
/// Passing the literal string `none` makes yt-dlp raise
/// `unsupported browser: "none"`, so every entry point funnels browser values
/// through this helper.
pub fn normalize_browser(raw: Option<&str>) -> Option<String> {
    let value = raw.unwrap_or("").trim().to_lowercase();
    if DISABLED_BROWSERS.contains(&value.as_str()) {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_browser_sentinels_become_none() {
        for value in [None, Some(""), Some("none"), Some(" OFF "), Some("false")] {
            assert_eq!(normalize_browser(value), None, "{value:?}");
        }
        assert_eq!(
            normalize_browser(Some("Firefox")),
            Some("firefox".to_string())
        );
        assert_eq!(
            normalize_browser(Some("chrome")),
            Some("chrome".to_string())
        );
    }
}
