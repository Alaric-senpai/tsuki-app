//! Player IPC commands.
//!
//! Episode page URLs are transient: the direct media target is resolved at
//! play time, never stored. This command exposes that resolution to the
//! frontend so the webview player can play remote sources.

use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::sources;
use crate::state::AppState;

/// A playable target handed to the frontend player.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayableSource {
    /// Direct URL (Dailymotion page, MP4, HLS manifest, or the original URL).
    pub url: String,
    pub title: String,
    pub source: String,
    /// Hosting platform, e.g. `donghuastream`, when detected.
    pub platform: String,
    pub is_hls: bool,
    /// True when the URL was a direct link that needed no source resolution.
    pub direct: bool,
}

/// Resolve an episode page URL to a playable media target.
///
/// Supported source pages are scraped at call time. Anything else is passed
/// through untouched (e.g. a direct MP4 or a YouTube watch URL) so the webview
/// player can still try it.
#[tauri::command]
pub async fn resolve_playable(
    _state: State<'_, AppState>,
    page_url: String,
) -> Result<PlayableSource, AppError> {
    let trimmed = page_url.trim();
    if trimmed.is_empty() {
        return Err(AppError::source_with(
            "There is no episode URL to play.",
            "empty page URL",
        ));
    }

    match sources::resolve_for_download(trimmed).await? {
        Some(media) => Ok(PlayableSource {
            url: media.url,
            title: media.title,
            source: media.source,
            platform: media.platform,
            is_hls: media.is_hls,
            direct: false,
        }),
        None => Ok(PlayableSource {
            url: trimmed.to_string(),
            title: String::new(),
            source: String::new(),
            platform: String::new(),
            is_hls: false,
            direct: true,
        }),
    }
}
