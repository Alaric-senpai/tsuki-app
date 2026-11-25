//! YouTube IPC commands. Nothing here is invoked on Explore mount — the
//! frontend calls `youtube_search` only after the user submits a search.

use tauri::State;

use crate::config::now_iso8601;
use crate::error::AppError;
use crate::state::AppState;
use crate::youtube::{YouTubeClient, YouTubeDiscovery, YouTubeRail, YouTubeVideo};

fn client(state: &AppState) -> Result<YouTubeClient, AppError> {
    let config = state.config()?;
    if !config.youtube.enabled {
        return Err(AppError::config_with(
            "YouTube is turned off.",
            "Enable it in Settings → Explore sources.",
        ));
    }
    if config.youtube.api_key.trim().is_empty() {
        return Err(AppError::config_with(
            "No YouTube API key is configured.",
            "Add a key in Settings → YouTube.",
        ));
    }
    Ok(YouTubeClient::new(
        config.youtube.api_key.clone(),
        config.youtube.region_code.clone(),
        config.youtube.language.clone(),
        config.youtube.safe_search.clone(),
        state.data_dir().join("cache").join("youtube"),
        u64::from(config.explore.cache_minutes),
    ))
}

#[tauri::command]
pub async fn youtube_search(
    state: State<'_, AppState>,
    query: String,
    max_results: Option<usize>,
) -> Result<Vec<YouTubeVideo>, AppError> {
    let limit = max_results.unwrap_or(state.config()?.explore.result_limit as usize);
    client(&state)?.search(&query, limit).await
}

#[tauri::command]
pub async fn youtube_trending(
    state: State<'_, AppState>,
    max_results: Option<usize>,
) -> Result<Vec<YouTubeVideo>, AppError> {
    let limit = max_results.unwrap_or(state.config()?.explore.result_limit as usize);
    client(&state)?.trending(limit).await
}

#[tauri::command]
pub async fn youtube_video(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<YouTubeVideo>, AppError> {
    client(&state)?.video(&id).await
}

#[tauri::command]
pub fn youtube_clear_cache(state: State<'_, AppState>) -> Result<bool, AppError> {
    let dir = state.data_dir().join("cache").join("youtube");
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir)
        .map_err(|err| AppError::io_with("Could not clear the YouTube cache.", err.to_string()))?;
    Ok(true)
}

/// Two rails (trending + curated search). Keeps partial results when one rail
/// fails, matching the legacy `discover()` behaviour.
#[tauri::command]
pub async fn youtube_discover(state: State<'_, AppState>) -> Result<YouTubeDiscovery, AppError> {
    let client = client(&state)?;
    let limit = state.config()?.explore.result_limit as usize;
    let mut rails = Vec::new();
    let mut errors = Vec::new();

    match client.trending(limit).await {
        Ok(videos) => rails.push(YouTubeRail {
            title: "Trending now".to_string(),
            query: "trending".to_string(),
            videos,
            error: None,
        }),
        Err(err) => {
            errors.push(err.message().to_string());
            rails.push(YouTubeRail {
                title: "Trending now".to_string(),
                query: "trending".to_string(),
                videos: Vec::new(),
                error: Some(err.message().to_string()),
            });
        }
    }

    match client.search("donghua", limit).await {
        Ok(videos) => rails.push(YouTubeRail {
            title: "Popular donghua".to_string(),
            query: "donghua".to_string(),
            videos,
            error: None,
        }),
        Err(err) => {
            errors.push(err.message().to_string());
            rails.push(YouTubeRail {
                title: "Popular donghua".to_string(),
                query: "donghua".to_string(),
                videos: Vec::new(),
                error: Some(err.message().to_string()),
            });
        }
    }

    Ok(YouTubeDiscovery {
        rails,
        errors,
        fetched_at: now_iso8601(),
    })
}
