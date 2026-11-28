//! Explore IPC commands: remote catalogs only.
//!
//! The Explore screen never reads the local library. Everything here talks to
//! a remote source through `sources::*`, and the only writes are the explicit
//! "add to library" / "add and download" actions.

use serde::Serialize;
use tauri::State;

use crate::db::models::EpisodeEntry;
use crate::db::queries;
use crate::downloader::DownloadSnapshot;
use crate::error::AppError;
use crate::media_ids::extract_video_id;
use crate::sources::animecube;
use crate::sources::animecube_live;
use crate::sources::animekhor;
use crate::sources::animexin;
use crate::sources::browse::{CatalogFilters, CatalogPage, SourceSeries};
use crate::sources::donghuastream::browse::{self, CatalogQuery};
use crate::sources::dongsub;
use crate::sources::lmanime;
use crate::sources::luciferdonghua;
use crate::sources::wowtpix;
use crate::state::AppState;

use super::downloads::enqueue_episode;

/// One entry in the source dropdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub key: String,
    pub label: String,
    /// Unavailable sources stay visible but disabled, so the dropdown explains
    /// what is coming instead of silently hiding it.
    pub available: bool,
    pub note: String,
}

/// Result of adding a remote series (and optionally queueing its episodes).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceImportResult {
    pub series_id: i64,
    pub series_name: String,
    pub added: i64,
    pub updated: i64,
    pub total: i64,
    pub queued: Vec<DownloadSnapshot>,
    /// Per-episode enqueue problems; the import itself still succeeded.
    pub warnings: Vec<String>,
}

/// Sources offered by the Explore dropdown.
#[tauri::command]
pub fn explore_sources(state: tauri::State<'_, crate::state::AppState>) -> Vec<SourceInfo> {
    let config = state.config().ok();
    let youtube_enabled = config
        .as_ref()
        .is_some_and(|value| value.youtube.enabled && !value.youtube.api_key.trim().is_empty());
    source_infos(youtube_enabled)
}

/// Pure source list so the availability contract can be unit-tested.
fn source_infos(youtube_enabled: bool) -> Vec<SourceInfo> {
    vec![
        SourceInfo {
            key: "donghuastream".to_string(),
            label: "DonghuaStream".to_string(),
            available: true,
            note: "Search, genres and trending. No account needed.".to_string(),
        },
        SourceInfo {
            key: animecube_live::SOURCE_KEY.to_string(),
            label: "AnimeCube".to_string(),
            available: true,
            note: "Live catalog and downloads. No account needed.".to_string(),
        },
        SourceInfo {
            key: animekhor::SOURCE_KEY.to_string(),
            label: "AnimeKhor".to_string(),
            available: true,
            note: "No account needed.".to_string(),
        },
        SourceInfo {
            key: dongsub::SOURCE_KEY.to_string(),
            label: "DongSub".to_string(),
            available: true,
            note: "Catalog and downloads. No account needed.".to_string(),
        },
        SourceInfo {
            key: lmanime::SOURCE_KEY.to_string(),
            label: "LMAnime".to_string(),
            available: true,
            note: "Catalog and downloads. No account needed.".to_string(),
        },
        SourceInfo {
            key: wowtpix::SOURCE_KEY.to_string(),
            label: "WowtPix".to_string(),
            available: true,
            note: "Catalog and downloads. No account needed.".to_string(),
        },
        SourceInfo {
            key: luciferdonghua::SOURCE_KEY.to_string(),
            label: "Lucifer Donghua".to_string(),
            available: true,
            note: "Also covers Mister Donghua. No account needed.".to_string(),
        },
        SourceInfo {
            key: animexin::SOURCE_KEY.to_string(),
            label: "AnimeXin".to_string(),
            available: true,
            note: "Catalog and downloads. No account needed.".to_string(),
        },
        SourceInfo {
            key: "youtube".to_string(),
            label: "YouTube".to_string(),
            available: youtube_enabled,
            note: if youtube_enabled {
                "Opted in. Search runs only when you submit a query.".to_string()
            } else {
                "Optional source — enable YouTube and add an API key in Settings.".to_string()
            },
        },
    ]
}

fn ensure_source(source: &str) -> Result<&str, AppError> {
    let source = source.trim();
    const AVAILABLE: [&str; 8] = [
        donghuastream::SOURCE_KEY,
        animekhor::SOURCE_KEY,
        animecube_live::SOURCE_KEY,
        dongsub::SOURCE_KEY,
        lmanime::SOURCE_KEY,
        wowtpix::SOURCE_KEY,
        luciferdonghua::SOURCE_KEY,
        animexin::SOURCE_KEY,
    ];
    if AVAILABLE.iter().any(|key| source.eq_ignore_ascii_case(key)) {
        Ok(source)
    } else {
        Err(AppError::source_with(
            "That source is not available yet.",
            format!("requested source: {source}"),
        ))
    }
}

/// One match arm per source; every command that needs a remote series uses it.
pub(crate) async fn remote_series(
    source: &str,
    id: &str,
    minutes: u32,
    refresh: bool,
) -> Result<SourceSeries, AppError> {
    if source.eq_ignore_ascii_case(animekhor::SOURCE_KEY) {
        animekhor::series(id, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(animecube_live::SOURCE_KEY) {
        animecube_live::series(id, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(dongsub::SOURCE_KEY) {
        dongsub::series(id, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(lmanime::SOURCE_KEY) {
        lmanime::series(id, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(wowtpix::SOURCE_KEY) {
        wowtpix::series(id, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(luciferdonghua::SOURCE_KEY) {
        luciferdonghua::series(id, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(animexin::SOURCE_KEY) {
        animexin::series(id, minutes, refresh).await
    } else {
        browse::series(id, minutes, refresh).await
    }
}

async fn remote_catalog(
    source: &str,
    query: &CatalogQuery,
    minutes: u32,
    refresh: bool,
) -> Result<CatalogPage, AppError> {
    if source.eq_ignore_ascii_case(animekhor::SOURCE_KEY) {
        animekhor::catalog(query, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(animecube_live::SOURCE_KEY) {
        animecube_live::catalog(query, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(dongsub::SOURCE_KEY) {
        dongsub::catalog(query, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(lmanime::SOURCE_KEY) {
        lmanime::catalog(query, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(wowtpix::SOURCE_KEY) {
        wowtpix::catalog(query, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(luciferdonghua::SOURCE_KEY) {
        luciferdonghua::catalog(query, minutes, refresh).await
    } else if source.eq_ignore_ascii_case(animexin::SOURCE_KEY) {
        animexin::catalog(query, minutes, refresh).await
    } else {
        browse::catalog(query, minutes, refresh).await
    }
}

async fn remote_filters(
    source: &str,
    minutes: u32,
    refresh: bool,
) -> Result<CatalogFilters, AppError> {
    if source.eq_ignore_ascii_case(animekhor::SOURCE_KEY) {
        Err(AppError::source_with(
            "AnimeKhor filters are read from its catalog page.",
            source.to_string(),
        ))
    } else if source.eq_ignore_ascii_case(animecube_live::SOURCE_KEY) {
        animecube_live::filters(minutes, refresh).await
    } else if source.eq_ignore_ascii_case(dongsub::SOURCE_KEY) {
        dongsub::filters(minutes, refresh).await
    } else if source.eq_ignore_ascii_case(lmanime::SOURCE_KEY) {
        lmanime::filters(minutes, refresh).await
    } else if source.eq_ignore_ascii_case(wowtpix::SOURCE_KEY) {
        wowtpix::filters(minutes, refresh).await
    } else if source.eq_ignore_ascii_case(luciferdonghua::SOURCE_KEY) {
        luciferdonghua::filters(minutes, refresh).await
    } else if source.eq_ignore_ascii_case(animexin::SOURCE_KEY) {
        animexin::filters(minutes, refresh).await
    } else {
        browse::filters(minutes, refresh).await
    }
}

/// Genre chips and sort/status/type options for the active source.
#[tauri::command]
pub async fn explore_filters(
    state: State<'_, AppState>,
    source: String,
    refresh: Option<bool>,
) -> Result<CatalogFilters, AppError> {
    ensure_source(&source)?;
    let source = ensure_source(&source)?;
    let minutes = state.config()?.explore.cache_minutes;
    if source.eq_ignore_ascii_case(animekhor::SOURCE_KEY) {
        animekhor::filters(minutes, refresh.unwrap_or(false)).await
    } else if source.eq_ignore_ascii_case(animecube::SOURCE_KEY) {
        Ok(animecube::filters())
    } else {
        remote_filters(source, minutes, refresh.unwrap_or(false)).await
    }
}

/// Trending / genre / search results. `query.limit == 0` uses the configured
/// result limit.
#[tauri::command]
pub async fn explore_catalog(
    state: State<'_, AppState>,
    source: String,
    query: CatalogQuery,
    refresh: Option<bool>,
) -> Result<CatalogPage, AppError> {
    ensure_source(&source)?;
    let explore = state.config()?.explore.clone();
    let mut query = query;
    if query.limit == 0 {
        query.limit = explore.result_limit as usize;
    }
    let source = ensure_source(&source)?;
    remote_catalog(
        source,
        &query,
        explore.cache_minutes,
        refresh.unwrap_or(false),
    )
    .await
}

/// Full details for one remote series, including its episode list.
#[tauri::command]
pub async fn explore_series(
    state: State<'_, AppState>,
    source: String,
    id: String,
    refresh: Option<bool>,
) -> Result<SourceSeries, AppError> {
    ensure_source(&source)?;
    let minutes = state.config()?.explore.cache_minutes;
    let source = ensure_source(&source)?;
    remote_series(source, &id, minutes, refresh.unwrap_or(false)).await
}

/// Add a remote series to the library, optionally queueing the selected
/// episodes for download immediately.
///
/// An empty `episode_urls` list means "every episode".
///
/// `target_series_id` appends the selection to an existing library entry
/// instead of creating one. `series_name` names (and therefore names the
/// folder of) a new entry; empty falls back to the remote title.
#[tauri::command]
pub async fn import_source_series(
    state: State<'_, AppState>,
    source: String,
    id: String,
    episode_urls: Vec<String>,
    download: bool,
    target_series_id: Option<i64>,
    series_name: Option<String>,
) -> Result<SourceImportResult, AppError> {
    ensure_source(&source)?;
    let minutes = state.config()?.explore.cache_minutes;
    let source = ensure_source(&source)?;
    let remote = remote_series(source, &id, minutes, false).await?;

    let selected: Vec<&crate::sources::browse::SourceEpisode> = if episode_urls.is_empty() {
        remote.episodes.iter().collect()
    } else {
        let wanted: std::collections::HashSet<&str> =
            episode_urls.iter().map(String::as_str).collect();
        remote
            .episodes
            .iter()
            .filter(|episode| wanted.contains(episode.url.as_str()))
            .collect()
    };

    if selected.is_empty() {
        return Err(AppError::source_with(
            "No episodes were selected.",
            format!("{} offers {} episodes", remote.title, remote.episode_count),
        ));
    }

    let pool = state.db().pool();
    let mut banner_warning: Option<String> = None;
    let (series_id, series_name) = match target_series_id {
        Some(target) => {
            let existing = queries::get_series_summary_by_id(pool, target)
                .await?
                .ok_or_else(|| {
                    AppError::db_with(
                        "That library entry no longer exists.",
                        format!("series id {target}"),
                    )
                })?;
            (existing.id, existing.name)
        }
        None => {
            let requested = series_name.unwrap_or_default();
            let name = if requested.trim().is_empty() {
                remote.title.as_str()
            } else {
                requested.trim()
            };
            // `create_series` also creates (or reuses) the per-series folder.
            let created = queries::create_series(pool, name, None, &remote.description).await?;
            // Cache the source poster so a new entry has artwork offline.
            // Artwork is best-effort: a broken image must not fail the import.
            if let Err(error) = super::library::fetch_remote_banner(
                pool,
                state.data_dir(),
                created.id,
                &remote.thumbnail,
                &source,
            )
            .await
            {
                log::warn!(
                    "could not cache banner for series {}: {}",
                    created.id,
                    error.message()
                );
                banner_warning = Some(
                    "The poster could not be cached; the library will use a placeholder."
                        .to_string(),
                );
            }
            (created.id, created.name)
        }
    };
    // Remember where this entry came from so the Home "Updates" section can
    // re-check the page later.
    queries::set_series_source(pool, series_id, &source, &remote.url).await?;
    let entries: Vec<EpisodeEntry> = selected
        .iter()
        .map(|episode| EpisodeEntry {
            url: episode.url.clone(),
            title: episode.title.clone(),
            episode_number: episode.episode_number,
            status: None,
        })
        .collect();
    let counts = queries::add_episode_entries(pool, series_id, &entries).await?;

    let mut result = SourceImportResult {
        series_id,
        series_name,
        added: counts.added,
        updated: counts.updated,
        total: counts.total,
        queued: Vec::new(),
        warnings: banner_warning.into_iter().collect(),
    };
    state.notify_library_changed();

    if !download {
        return Ok(result);
    }

    // Match the freshly upserted rows back to the selected page URLs so the
    // queue gets real episode ids. Walk `selected` in the order the user
    // picked them — a HashSet/DB pass would reorder the queue arbitrarily.
    let rows = queries::list_episodes(pool, series_id, None).await?;
    let by_video_id: std::collections::HashMap<String, _> = rows
        .into_iter()
        .map(|row| (row.video_id.clone(), row))
        .collect();
    for episode in &selected {
        let video_id = extract_video_id(&episode.url);
        let Some(row) = by_video_id.get(&video_id) else {
            continue;
        };
        if row.status == "downloaded" {
            continue;
        }
        match enqueue_episode(&state, row.id).await {
            Ok(snapshot) => result.queued.push(snapshot),
            Err(error) => result.warnings.push(format!(
                "{}: {}",
                if episode.title.trim().is_empty() {
                    video_id
                } else {
                    episode.title.clone()
                },
                error.message()
            )),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_infos_offer_the_live_animecube_replacement() {
        let sources = source_infos(true);
        let cube = sources
            .iter()
            .find(|source| source.key == animecube_live::SOURCE_KEY)
            .expect("animecube entry");
        assert!(cube.available);
        assert!(!cube.note.contains("unavailable"));
    }

    #[test]
    fn source_infos_include_animekhor() {
        let sources = source_infos(true);
        let khor = sources
            .iter()
            .find(|source| source.key == animekhor::SOURCE_KEY)
            .expect("animekhor entry");
        assert!(khor.available);
        assert_eq!(khor.label, "AnimeKhor");
    }

    #[test]
    fn all_new_sources_are_allowed() {
        assert!(ensure_source("animekhor").is_ok());
        assert!(ensure_source("donghuastream").is_ok());
        assert!(ensure_source(animecube_live::SOURCE_KEY).is_ok());
        assert!(ensure_source(dongsub::SOURCE_KEY).is_ok());
        assert!(ensure_source(lmanime::SOURCE_KEY).is_ok());
        assert!(ensure_source(wowtpix::SOURCE_KEY).is_ok());
        assert!(ensure_source(luciferdonghua::SOURCE_KEY).is_ok());
        assert!(ensure_source(animexin::SOURCE_KEY).is_ok());
    }

    #[test]
    fn retired_and_unowned_sources_are_still_rejected() {
        assert!(ensure_source("animecube").is_err());
        assert!(ensure_source("youtube").is_err());
        assert!(ensure_source("streamplay").is_err());
    }
}
