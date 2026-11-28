//! Remote source clients.
//!
//! Ported from the legacy `sources.py`. Every source resolves a *stable page
//! URL* into a *transient direct media URL* at download/play time; resolved
//! URLs are never persisted because they expire.
//!
//! All network access lives here (and in the downloader) — the frontend never
//! talks to third-party sites.

pub mod animecube;
pub mod animecube_live;
pub mod animekhor;
pub mod animexin;
pub mod browse;
pub mod cache;
pub mod donghuastream;
pub mod dongsub;
pub mod lmanime;
pub mod luciferdonghua;
pub mod wowtpix;

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
    } else if animekhor::is_animekhor_host(&host) {
        Some(animekhor::SOURCE_KEY)
    } else if animecube_live::is_animecube_live_host(&host) {
        Some(animecube_live::SOURCE_KEY)
    } else if dongsub::is_dongsub_host(&host) {
        Some(dongsub::SOURCE_KEY)
    } else if lmanime::is_lmanime_host(&host) {
        Some(lmanime::SOURCE_KEY)
    } else if wowtpix::is_wowtpix_host(&host) {
        Some(wowtpix::SOURCE_KEY)
    } else if luciferdonghua::is_luciferdonghua_host(&host) {
        Some(luciferdonghua::SOURCE_KEY)
    } else if animexin::is_animexin_host(&host) {
        Some(animexin::SOURCE_KEY)
    } else if animecube::is_animecube_host(&host) {
        Some(animecube::SOURCE_KEY)
    } else {
        None
    }
}

#[allow(dead_code)] // used by the Explore/show-details phase
pub fn supports(raw: &str) -> bool {
    source_for_url(raw).is_some()
}

/// Streamplay embeds have no working resolver yet (see `docs/PROGRESS.md`).
/// Returning `Ok(None)` would hand the raw page to yt-dlp, which reports a
/// generic "Unsupported URL" after all retries; fail before any network call.
pub fn is_streamplay_url(raw: &str) -> bool {
    Url::parse(raw.trim())
        .ok()
        .and_then(|url| url.host_str().map(str::to_lowercase))
        .is_some_and(|host| {
            host == "play.streamplay.co.in" || host.ends_with(".play.streamplay.co.in")
        })
}

/// Resolve a supported episode page to a playable target.
///
/// Returns `Ok(None)` for URLs no source owns (plain YouTube links etc.), so
/// the downloader can hand them straight to yt-dlp.
pub async fn resolve_for_download(page_url: &str) -> Result<Option<ResolvedMedia>, AppError> {
    if is_streamplay_url(page_url) {
        return Err(AppError::source_with(
            "Streamplay.co.in downloads are pending development. Choose another source or try again in a later release.",
            format!("requested page: {page_url}"),
        ));
    }
    match source_for_url(page_url) {
        Some(donghuastream::SOURCE_KEY) => Ok(Some(donghuastream::resolve(page_url).await?)),
        Some(animekhor::SOURCE_KEY) => Ok(Some(animekhor::resolve(page_url).await?)),
        Some(animecube_live::SOURCE_KEY) => Ok(Some(animecube_live::resolve(page_url).await?)),
        Some(dongsub::SOURCE_KEY) => Ok(Some(dongsub::resolve(page_url).await?)),
        Some(lmanime::SOURCE_KEY) => Ok(Some(lmanime::resolve(page_url).await?)),
        Some(wowtpix::SOURCE_KEY) => Ok(Some(wowtpix::resolve(page_url).await?)),
        Some(luciferdonghua::SOURCE_KEY) => Ok(Some(luciferdonghua::resolve(page_url).await?)),
        Some(animexin::SOURCE_KEY) => Ok(Some(animexin::resolve(page_url).await?)),
        // The retired AnimeCube scraper still owns the host so a raw page is
        // never handed to yt-dlp; the live replacement is `animecube_live`.
        Some(animecube::SOURCE_KEY) => Err(AppError::source_with(
            "AnimeCube downloads are temporarily unavailable.",
            format!("requested page: {page_url}"),
        )),
        _ => Ok(None),
    }
}

/// Shared HTTP client (connection pooling, rustls, compressed responses).
pub(crate) fn client() -> Result<&'static reqwest::Client, AppError> {
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
    // A cached connectivity check keeps offline Explore searches from failing
    // with a confusing source-parser error. The probe is short and shared.
    crate::network::ensure_online().await?;
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
/// Turn a protocol-relative URL (`//host/path`) into an absolute https URL.
///
/// DonghuaStream embeds several mirrors with `src="//host/..."`; yt-dlp's
/// generic extractor cannot resolve those and reports `Unsupported URL`. All
/// resolvers funnel their final media URL through this helper.
pub fn absolute_media_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        trimmed.to_string()
    }
}

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
    fn protocol_relative_media_urls_become_absolute() {
        assert_eq!(
            absolute_media_url("//play.streamplay.co.in/embed/uuid"),
            "https://play.streamplay.co.in/embed/uuid"
        );
        assert_eq!(
            absolute_media_url("  https://example.com/video.mp4  "),
            "https://example.com/video.mp4"
        );
    }

    #[test]
    fn animecube_live_is_now_an_owned_source() {
        assert_eq!(
            source_for_url("https://animecube.live/title/s45n2a5c7x"),
            Some(animecube_live::SOURCE_KEY)
        );
    }

    #[test]
    fn animekhor_pages_are_recognised() {
        assert_eq!(
            source_for_url("https://animekhor.org/renegade-immortal-episode-1/"),
            Some("animekhor")
        );
        assert_eq!(
            source_for_url("https://www.animekhor.org/anime/renegade-immortal/"),
            Some("animekhor")
        );
    }

    #[tokio::test]
    async fn streamplay_pages_fail_with_an_actionable_error() {
        let result = resolve_for_download(
            "https://play.streamplay.co.in/embed/4809ec9e-9c45-435f-8b29-317a61cffd22",
        )
        .await;
        let error = match result {
            Ok(Some(_)) => panic!("streamplay must not be handed to yt-dlp"),
            Ok(None) => panic!("streamplay must be recognised, not treated as a generic URL"),
            Err(error) => error,
        };
        assert!(error.message().contains("pending development"));
    }

    #[test]
    fn hls_urls_are_detected() {
        assert!(is_hls_url("https://cdn.example/master.m3u8"));
        assert!(is_hls_url("https://cdn.example/master.m3u8?token=1"));
        assert!(!is_hls_url("https://cdn.example/video.mp4"));
    }
}
