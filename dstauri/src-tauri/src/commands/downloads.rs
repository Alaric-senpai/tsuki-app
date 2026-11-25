//! Download IPC commands.
//!
//! The frontend enqueues work here and listens to `download://state` /
//! `download://console` events. No process or network access crosses the IPC
//! boundary.

use tauri::State;

use crate::db::queries;
use crate::downloader::{self, DownloadSnapshot, Job};
use crate::error::AppError;
use crate::state::AppState;

/// Queue one episode download.
#[tauri::command]
pub async fn enqueue_episode_download(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<DownloadSnapshot, AppError> {
    enqueue_episode(&state, episode_id).await
}

/// Shared enqueue path: the Explore screen queues freshly added episodes
/// through the exact same settings and status transition as the library.
pub async fn enqueue_episode(
    state: &AppState,
    episode_id: i64,
) -> Result<DownloadSnapshot, AppError> {
    let pool = state.db().pool();
    let Some((episode, series)) = queries::episode_job_context(pool, episode_id).await? else {
        return Err(AppError::db("That episode no longer exists."));
    };

    if episode.status == "downloaded" {
        return Err(AppError::download(
            "That episode is already marked as downloaded. Sync the archive to re-check the disk.",
        ));
    }
    if episode.url.trim().is_empty() {
        return Err(AppError::download(
            "That episode has no page URL to download from.",
        ));
    }

    let config = state.config()?;
    let settings = &config.settings;
    let quality = settings
        .get("default_quality")
        .map(String::as_str)
        .unwrap_or("720p");
    let container = settings
        .get("default_container")
        .map(String::as_str)
        .unwrap_or("mp4");
    let sub_langs = settings
        .get("sub_langs")
        .map(String::as_str)
        .unwrap_or("en");
    let max_retries = settings
        .get("max_retries")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(3)
        .max(1);
    let rate_limit_mb = settings
        .get("rate_limit_mb")
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(0.0);
    let browser =
        downloader::normalize_browser(settings.get("browser_cookies").map(String::as_str));

    let save_dir = if series.save_dir.trim().is_empty() {
        let root = settings
            .get("global_save_dir")
            .map(String::as_str)
            .unwrap_or("~/Videos");
        queries::expand_dir(root).join(&series.name)
    } else {
        queries::expand_dir(&series.save_dir)
    };

    let job = Job {
        episode_id: episode.id,
        series_id: series.id,
        video_id: episode.video_id.clone(),
        page_url: episode.url.clone(),
        title: if episode.title.trim().is_empty() {
            format!("{} episode", series.name)
        } else {
            episode.title.clone()
        },
        save_dir,
        quality: quality.to_string(),
        container: container.to_string(),
        browser,
        // The legacy engine always asks for subtitles when the setting exists;
        // there is no separate toggle yet.
        write_subs: true,
        sub_langs: sub_langs.to_string(),
        rate_limit_mb,
        max_retries,
    };

    // Mark the row before the queue starts so the UI reflects intent
    // immediately, even while another episode is still downloading.
    queries::set_episode_status_by_id(pool, episode_id, "queued").await?;
    state.downloads().enqueue(job)
}

/// Requeue every episode still marked `queued` in the catalog.
///
/// The download queue lives in memory, so a restart would otherwise strand
/// rows the user asked for. Called once from the setup hook; returns how many
/// episodes were put back in line.
pub async fn resume_queued(state: &AppState) -> Result<usize, AppError> {
    let pool = state.db().pool();
    let ids = queries::queued_episode_ids(pool).await?;
    let mut resumed = 0usize;
    for id in ids {
        match enqueue_episode(state, id).await {
            Ok(_) => resumed += 1,
            Err(error) => log::warn!("could not resume episode {}: {}", id, error.message()),
        }
    }
    if resumed > 0 {
        log::info!("resumed {resumed} queued download(s) after startup");
    }
    Ok(resumed)
}

/// Requeue any episodes still marked `queued` in the catalog (startup safety
/// net, also exposed so the UI can retrigger it).
#[tauri::command]
pub async fn resume_downloads(state: State<'_, AppState>) -> Result<usize, AppError> {
    resume_queued(&state).await
}

/// Change the number of simultaneous downloads and persist the preference.
#[tauri::command]
pub fn set_download_concurrency(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<usize, AppError> {
    let limit = limit.clamp(1, 16);
    state.update_setting("concurrent_downloads", &limit.to_string())?;
    state.downloads().set_concurrency(limit);
    Ok(limit)
}

/// Ask the running download for one episode to stop.
#[tauri::command]
pub fn cancel_episode_download(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<(), AppError> {
    state.downloads().cancel(episode_id);
    state.notify_library_changed();
    Ok(())
}

/// Every job the queue knows about in this session.
#[tauri::command]
pub fn download_queue(state: State<'_, AppState>) -> Result<Vec<DownloadSnapshot>, AppError> {
    Ok(state.downloads().snapshots())
}

/// Remove one job from the queue: stops it if it is still running and drops
/// its row. Returns whether the queue knew about it.
#[tauri::command]
pub fn remove_from_queue(state: State<'_, AppState>, episode_id: i64) -> Result<bool, AppError> {
    let known = state.downloads().remove(episode_id);
    state.notify_library_changed();
    Ok(known)
}

/// Drop finished jobs (`done`/`failed`/`cancelled`) from the queue. Running and
/// queued jobs are untouched. Returns the episode ids that were cleared.
#[tauri::command]
pub fn clear_download_queue(state: State<'_, AppState>) -> Result<Vec<i64>, AppError> {
    let cleared = state.downloads().clear_finished();
    if !cleared.is_empty() {
        state.notify_library_changed();
    }
    Ok(cleared)
}

/// Resolve a downloaded episode's file path from the catalog, so /watch keeps
/// working after a restart or after the live queue row is cleared.
#[tauri::command]
pub async fn downloaded_episode_path(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<Option<String>, AppError> {
    queries::downloaded_episode_path(state.db().pool(), episode_id).await
}

/// One job's latest state, if the queue knows it.
#[tauri::command]
pub fn download_state(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<Option<DownloadSnapshot>, AppError> {
    Ok(state.downloads().snapshot(episode_id))
}
