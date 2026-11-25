//! Remote source clients.
//!
//! Ported from the legacy `sources.py`. Every source resolves a *stable page
//! URL* into a *transient direct media URL* at download/play time; resolved
//! URLs are never persisted because they expire.
//!
//! All network access lives here (and in the downloader) — the frontend never
//! talks to third-party sites.

pub mod browse;
pub mod cache;
pub mod donghuastream;

use std::sync::OnceLock;
use std::time::Duration;

use serde::Serialize;
use url::Url;

use crate::error::AppError;
use crate::media_ids;

pub const DONGHUASTREAM_BASE: &str = "https://donghuastream.org";

/// Desktop browser UA; both scraped sources reject the default reqwest agent.
pub const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A playable media target resolved from a source episode page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMedia {
    /// Direct media URL (Dailymotion page, MP4, or HLS manifest).
    pub url: String,
    pub title: String,
    /// Source key, e.g. `donghuastream`.
    pub source: String,
    /// Stable identifier of the *page* URL, used for the download archive.
    pub video_id: String,
    /// Hosting platform, e.g. `dailymotion`, when detected.
    pub platform: String,
    pub is_hls: bool,
}

/// Identify the source that owns a page URL.
pub fn source_for_url(raw: &str) -> Option<&'static str> {
    let host = Url::parse(raw.trim())
        .ok()
        .and_then(|url| url.host_str().map(str::to_lowercase))?;
    if media_ids::is_donghuastream_host(&host) {
        Some(donghuastream::SOURCE_KEY)
    } else {
        None
    }
}

#[allow(dead_code)] // used by the Explore/show-details phase
pub fn supports(raw: &str) -> bool {
    source_for_url(raw).is_some()
}

/// Resolve a supported episode page to a playable target.
///
/// Returns `Ok(None)` for URLs no source owns (plain YouTube links etc.), so
/// the downloader can hand them straight to yt-dlp.
pub async fn resolve_for_download(page_url: &str) -> Result<Option<ResolvedMedia>, AppError> {
    match source_for_url(page_url) {
        Some(donghuastream::SOURCE_KEY) => Ok(Some(donghuastream::resolve(page_url).await?)),
        _ => Ok(None),
    }
}

/// Shared HTTP client (connection pooling, rustls, compressed responses).
fn client() -> Result<&'static reqwest::Client, AppError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let built = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|err| err.to_string())
    });
    built
        .as_ref()
        .map_err(|err| AppError::source_with("Could not prepare the network client.", err.clone()))
}

/// Fetch a page as text with the shared client.
pub async fn fetch_text(url: &str) -> Result<String, AppError> {
    let response = client()?
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .send()
        .await
        .map_err(|err| {
            AppError::source_with(
                "Could not reach the source site. Check your connection and try again.",
                format!("{url}: {err}"),
            )
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::source_with(
            format!("The source site answered with HTTP {status}."),
            url.to_string(),
        ));
    }

    response.text().await.map_err(|err| {
        AppError::source_with(
            "The source site sent a response that could not be read.",
            format!("{url}: {err}"),
        )
    })
}

/// True when a direct URL looks like an HLS manifest.
pub fn is_hls_url(raw: &str) -> bool {
    let path = Url::parse(raw)
        .map(|url| url.path().to_lowercase())
        .unwrap_or_else(|_| raw.to_lowercase());
    path.ends_with(".m3u8") || path.contains(".m3u8?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_donghuastream_pages_have_a_source() {
        assert_eq!(
            source_for_url("https://donghuastream.org/xin-xia-episode-1/"),
            Some("donghuastream")
        );
        assert_eq!(
            source_for_url("https://www.donghuastream.org/anime/x/"),
            Some("donghuastream")
        );
        assert_eq!(
            source_for_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            None
        );
        assert_eq!(source_for_url("not a url"), None);
    }

    #[test]
    fn hls_urls_are_detected() {
        assert!(is_hls_url("https://cdn.example/master.m3u8"));
        assert!(is_hls_url("https://cdn.example/master.m3u8?token=1"));
        assert!(!is_hls_url("https://cdn.example/video.mp4"));
    }
}
