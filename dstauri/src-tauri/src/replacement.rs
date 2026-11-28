//! Background repair of library episode links.
//!
//! Some sources host an episode on several sites; the page we imported may
//! stop working while a replacement still plays. Once at startup (and on
//! demand) we walk episodes that are still pending or failed, re-read the
//! series page from its source, and point the row at a working page URL for
//! the same episode number. Downloaded episodes are never touched.
//!
//! YouTube is excluded: it has no per-series page to re-scan, and Streamplay
//! is excluded because it has no resolver yet.

use std::collections::HashMap;
use std::future::Future;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::db::queries;
use crate::error::AppError;
use crate::media_ids;
use crate::sources::browse::SourceSeries;
use crate::sources::{
    animecube_live, animekhor, animexin, donghuastream, dongsub, is_streamplay_url, lmanime,
    luciferdonghua, source_for_url, wowtpix,
};

/// Result of one background repair pass.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub series_checked: usize,
    pub episodes_checked: usize,
    pub replaced: usize,
    pub errors: Vec<String>,
}

/// Source keys eligible for background link repair.
pub fn allowed_source(source: &str) -> bool {
    let key = source.trim();
    [
        donghuastream::SOURCE_KEY,
        animekhor::SOURCE_KEY,
        animecube_live::SOURCE_KEY,
        dongsub::SOURCE_KEY,
        lmanime::SOURCE_KEY,
        wowtpix::SOURCE_KEY,
        luciferdonghua::SOURCE_KEY,
        animexin::SOURCE_KEY,
    ]
    .contains(&key)
}

/// True for page URLs we can re-resolve at download time. YouTube links and
/// Streamplay embeds are deliberately not candidates.
pub fn allowed_url(url: &str) -> bool {
    let Some(source) = source_for_url(url) else {
        return false;
    };
    allowed_source(source) && !is_streamplay_url(url)
}

/// Run one repair pass. The caller supplies the source-specific series fetch,
/// keeping this module independent of each scraper's concrete signature.
pub async fn run_repair<F, Fut>(
    pool: &SqlitePool,
    fetch_series: F,
) -> Result<RepairReport, AppError>
where
    F: Fn(&str, &str) -> Fut,
    Fut: Future<Output = Result<SourceSeries, AppError>>,
{
    let mut report = RepairReport::default();
    let series = queries::list_series(pool).await?;

    for entry in series {
        if !allowed_source(&entry.source_key) || entry.source_url.trim().is_empty() {
            continue;
        }
        report.series_checked += 1;

        let mut pending = Vec::new();
        pending.extend(queries::list_episodes(pool, entry.id, Some("not_started")).await?);
        pending.extend(queries::list_episodes(pool, entry.id, Some("failed")).await?);
        if pending.is_empty() {
            continue;
        }

        let remote = match fetch_series(&entry.source_key, &entry.source_url).await {
            Ok(remote) => remote,
            Err(error) => {
                report
                    .errors
                    .push(format!("{}: {}", entry.name, error.message()));
                continue;
            }
        };
        let by_number: HashMap<i64, &crate::sources::browse::SourceEpisode> = remote
            .episodes
            .iter()
            .filter_map(|episode| episode.episode_number.map(|number| (number, episode)))
            .collect();

        for episode in pending {
            report.episodes_checked += 1;
            let Some(number) = episode.episode_number else {
                continue;
            };
            let Some(candidate) = by_number.get(&number) else {
                continue;
            };
            if candidate.url == episode.url || !allowed_url(&candidate.url) {
                continue;
            }
            // Only swap in a page our resolvers can actually turn into media.
            if crate::sources::resolve_for_download(&candidate.url)
                .await
                .is_err()
            {
                continue;
            }
            let video_id = media_ids::extract_video_id(&candidate.url);
            if let Err(error) =
                queries::update_episode_source_url(pool, episode.id, &candidate.url, &video_id)
                    .await
            {
                report
                    .errors
                    .push(format!("{} #{}: {}", entry.name, number, error.message()));
                continue;
            }
            report.replaced += 1;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_sources_are_explicit_and_exclude_youtube() {
        assert!(allowed_source(donghuastream::SOURCE_KEY));
        assert!(allowed_source(animecube_live::SOURCE_KEY));
        assert!(allowed_source(animexin::SOURCE_KEY));
        assert!(!allowed_source("youtube"));
        assert!(!allowed_source("streamplay"));
        assert!(!allowed_source(""));
    }

    #[test]
    fn excluded_urls_are_not_replacement_candidates() {
        assert!(!allowed_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(!allowed_url(
            "https://play.streamplay.co.in/embed/4809ec9e-9c45-435f-8b29-317a61cffd22"
        ));
        assert!(!allowed_url("not a url"));
    }
}
