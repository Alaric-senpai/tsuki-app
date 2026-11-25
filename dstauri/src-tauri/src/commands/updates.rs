//! Background "check for updates" for library entries.
//!
//! Only entries the user opted into (`check_updates = 1`) and that carry a
//! source page URL are re-checked. The Home screen renders the results; nothing
//! here downloads or adds episodes — the user decides what to pull in.

use std::collections::HashSet;

use serde::Serialize;
use tauri::State;

use crate::db::queries;
use crate::error::AppError;
use crate::media_ids::extract_video_id;
use crate::sources::donghuastream::browse;
use crate::state::AppState;

/// Normalize a page URL for comparison: whitespace, trailing slash and case
/// on the host do not make an episode new.
fn canonical(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_lowercase()
}

/// One entry's update check outcome.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesUpdate {
    pub series_id: i64,
    pub series_name: String,
    /// Episodes on the source page that the library does not know about yet.
    pub new_episodes: i64,
    /// Set when the source page could not be read; the row is still returned so
    /// the UI can explain why nothing was checked.
    pub error: Option<String>,
}

/// Re-check every library entry that is marked for update checks.
#[tauri::command]
pub async fn check_series_updates(
    state: State<'_, AppState>,
) -> Result<Vec<SeriesUpdate>, AppError> {
    let pool = state.db().pool();
    let minutes = state.config()?.explore.cache_minutes;
    let targets = queries::series_update_targets(pool).await?;

    let mut results = Vec::with_capacity(targets.len());
    for target in targets {
        let mut outcome = SeriesUpdate {
            series_id: target.id,
            series_name: target.name.clone(),
            new_episodes: 0,
            error: None,
        };

        // Only DonghuaStream exposes a per-series episode list today, so a
        // stored URL from any other source is skipped rather than guessed at.
        if !target.source_key.is_empty() && !target.source_key.eq_ignore_ascii_case("donghuastream")
        {
            outcome.error = Some(format!(
                "{} is not supported for update checks yet.",
                target.source_key
            ));
            results.push(outcome);
            continue;
        }

        match browse::series(&target.source_url, minutes, true).await {
            Ok(remote) => {
                // Compare full rows, not just hashed ids: the DonghuaStream
                // media id hashes the whole URL, so a trailing slash or query
                // change made an already-known episode look brand new.
                let known = queries::list_episodes(pool, target.id, None).await?;
                let known_urls: HashSet<String> =
                    known.iter().map(|row| canonical(&row.url)).collect();
                let known_ids: HashSet<String> =
                    known.iter().map(|row| extract_video_id(&row.url)).collect();
                let last_known_number = known
                    .iter()
                    .filter_map(|row| row.episode_number)
                    .max()
                    .unwrap_or(0);
                let new_episodes = remote
                    .episodes
                    .iter()
                    .filter(|episode| {
                        if known_urls.contains(&canonical(&episode.url)) {
                            return false;
                        }
                        if known_ids.contains(&extract_video_id(&episode.url)) {
                            return false;
                        }
                        // A missing id can still be an older episode the site
                        // re-listed. Anything at or below the last known
                        // number is not an update.
                        !episode
                            .episode_number
                            .is_some_and(|number| number <= last_known_number)
                    })
                    .count() as i64;
                queries::record_update_check(pool, target.id, new_episodes).await?;
                outcome.new_episodes = new_episodes;
            }
            Err(error) => {
                outcome.error = Some(error.message().to_string());
            }
        }

        results.push(outcome);
    }

    Ok(results)
}

/// Opt a library entry in or out of update checks.
#[tauri::command]
pub async fn set_series_check_updates(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<bool, AppError> {
    queries::set_series_check_updates(state.db().pool(), id, enabled).await
}
