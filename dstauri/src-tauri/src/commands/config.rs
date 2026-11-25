//! Configuration IPC commands.

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;

/// Split a user-configured player command into arguments. Supports double
/// quotes and backslash escapes so paths with spaces still work.
fn split_command(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if ch.is_whitespace() && !quoted {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// Hand a downloaded file to the user's own player (or the OS default when no
/// custom command is configured). The file path is always passed as one
/// argument; only the user's setting is split into program + arguments.
#[tauri::command]
pub fn open_in_external_player(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), AppError> {
    let configured = state.config()?.player.external_player.trim().to_string();
    if configured.is_empty() {
        return app.opener().open_path(path, None::<&str>).map_err(|err| {
            AppError::io_with(
                "Could not open the file with the default player.",
                err.to_string(),
            )
        });
    }

    let mut parts = split_command(&configured).into_iter();
    let program = parts
        .next()
        .ok_or_else(|| AppError::config("The external player command is empty."))?;
    let mut command = std::process::Command::new(program);
    command.args(parts);
    command.arg(&path);
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| AppError::io_with("Could not start the external player.", err.to_string()))
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, AppError> {
    state.config()
}

#[tauri::command]
pub fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<Config, AppError> {
    state.update_setting(&key, &value)
}

#[tauri::command]
pub fn get_config_path(state: State<'_, AppState>) -> String {
    state.config_store().path().display().to_string()
}

#[tauri::command]
pub fn set_player_config(
    state: State<'_, AppState>,
    engine: Option<String>,
    volume: Option<f64>,
    external_player: Option<String>,
    remember_position: Option<bool>,
    auto_play_next: Option<bool>,
) -> Result<Config, AppError> {
    state.update_player_config(
        engine,
        volume,
        external_player,
        remember_position,
        auto_play_next,
    )
}

#[tauri::command]
pub fn set_explore_config(
    state: State<'_, AppState>,
    result_limit: Option<u32>,
    cache_minutes: Option<u32>,
    update_source: Option<String>,
) -> Result<Config, AppError> {
    state.update_explore_config(result_limit, cache_minutes, update_source)
}

#[tauri::command]
pub fn set_youtube_config(
    state: State<'_, AppState>,
    api_key: Option<String>,
    region_code: Option<String>,
    language: Option<String>,
    safe_search: Option<String>,
) -> Result<Config, AppError> {
    state.update_youtube_config(api_key, region_code, language, safe_search)
}
