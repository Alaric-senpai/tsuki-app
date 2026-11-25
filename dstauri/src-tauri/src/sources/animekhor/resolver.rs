//! Resolve an animekhor.org episode page to a direct playable media URL.

use crate::error::AppError;
use crate::sources::ResolvedMedia;

use super::SOURCE_KEY;

/// Pure part of the resolver: convert a fetched episode page into playable
/// media. Kept separate from the network call so real fixtures can be tested.
pub fn resolved_from_html(html: &str, page_url: &str) -> Result<ResolvedMedia, AppError> {
    let mut media = crate::sources::donghuastream::parse_page(html, page_url).map_err(|_| {
        AppError::source_with(
            "animekhor.org did not expose a playable video on this episode page.",
            page_url.to_string(),
        )
    })?;
    media.source = SOURCE_KEY.to_string();
    media.video_id = crate::media_ids::extract_video_id(page_url);
    Ok(media)
}

/// Fetch the episode page at play/download time, then resolve it.
///
/// Resolved URLs expire, so they are never persisted.
pub async fn resolve(page_url: &str) -> Result<ResolvedMedia, AppError> {
    let html = crate::sources::fetch_text(page_url).await?;
    resolved_from_html(&html, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::animekhor::BASE;

    #[test]
    fn a_dailymotion_embed_resolves_to_a_direct_url() {
        let page = format!("{BASE}/renegade-immortal-episode-168-subtitles-english-indonesian/");
        let media = resolved_from_html(include_str!("fixtures/episode.html"), &page)
            .expect("resolved media");
        assert_eq!(
            media.url,
            "https://www.dailymotion.com/video/k4Flk6iRcTHxrdMYTRY"
        );
        assert_eq!(media.source, SOURCE_KEY);
        assert_eq!(media.platform, "dailymotion");
    }
}
