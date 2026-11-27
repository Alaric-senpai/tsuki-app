//! App updater IPC commands (distinct from the yt-dlp updater).

use tauri::{AppHandle, State};

use crate::app_updater::{self, AppRelease};
use tauri::Emitter;

use crate::config::Config;
use crate::error::AppError;
use crate::events;
use crate::state::AppState;

/// Persist the update-check preferences from the changelog dialog.
#[tauri::command]
pub fn set_app_update_settings(
    state: State<'_, AppState>,
    auto_check: bool,
    auto_download: bool,
) -> Result<Config, AppError> {
    state.update_setting("auto_check_app_updates", if auto_check { "1" } else { "0" })?;
    state.update_setting(
        "auto_download_app_updates",
        if auto_download { "1" } else { "0" },
    )
}

/// Start an update from the dialog. Reuses the pre-downloaded installer when
/// automatic downloads were enabled; otherwise downloads and opens it.
#[tauri::command]
pub async fn start_app_update(
    state: State<'_, AppState>,
    app: AppHandle,
    release: AppRelease,
) -> Result<String, AppError> {
    let data_dir = state.data_dir().to_path_buf();
    let path = match release.downloaded_path.as_deref() {
        Some(path) if std::path::Path::new(path).is_file() => path.to_string(),
        _ => app_updater::download(&data_dir, &release).await?,
    };
    app_updater::open(&data_dir, &app, &path)?;
    Ok(path)
}

/// Check GitHub releases for a newer version of the installed app.
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<AppRelease>, AppError> {
    let found = app_updater::check_for_update().await?;
    if let Some(release) = &found
        && let Err(error) = app.emit(events::UPDATE_AVAILABLE, release)
    {
        log::warn!("could not emit {}: {error}", events::UPDATE_AVAILABLE);
    }
    Ok(found)
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
