//! Playback position IPC commands.
//!
//! Rust owns the SQLite `watch_progress` table; the webview only sends
//! position updates and reads them back to resume playback.

use tauri::State;

use crate::db::models::WatchProgress;
use crate::db::queries;
use crate::error::AppError;
use crate::state::AppState;

/// Save (or update) the playback position of an episode.
#[tauri::command]
pub async fn save_watch_progress(
    state: State<'_, AppState>,
    episode_id: i64,
    position_sec: f64,
    duration_sec: f64,
    completed: bool,
) -> Result<bool, AppError> {
    queries::save_watch_progress(
        state.db().pool(),
        episode_id,
        position_sec,
        duration_sec,
        completed,
    )
    .await
}

/// Read the saved position of an episode, if any.
#[tauri::command]
pub async fn get_watch_progress(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<Option<WatchProgress>, AppError> {
    queries::get_watch_progress(state.db().pool(), episode_id).await
}

/// Forget the saved position of an episode.
#[tauri::command]
pub async fn clear_watch_progress(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<bool, AppError> {
    queries::clear_watch_progress(state.db().pool(), episode_id).await
}

/// Every saved position, newest first.
#[tauri::command]
pub async fn all_watch_progress(
    state: State<'_, AppState>,
) -> Result<Vec<WatchProgress>, AppError> {
    queries::all_watch_progress(state.db().pool()).await
}
