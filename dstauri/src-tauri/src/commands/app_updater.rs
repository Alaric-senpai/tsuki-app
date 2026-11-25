//! App updater IPC commands (distinct from the yt-dlp updater).

use tauri::{AppHandle, State};

use crate::app_updater::{self, AppRelease};
use crate::error::AppError;
use crate::state::AppState;

/// Check GitHub releases for a newer version of the installed app.
#[tauri::command]
pub async fn check_app_update() -> Result<Option<AppRelease>, AppError> {
    app_updater::check_for_update().await
}

/// Download the matching installer and open it for the user to install.
#[tauri::command]
pub async fn install_app_update(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, AppError> {
    let Some(release) = app_updater::check_for_update().await? else {
        return Err(AppError::config("This app is already up to date."));
    };
    app_updater::download_and_open(state.data_dir(), &app, &release).await
}
