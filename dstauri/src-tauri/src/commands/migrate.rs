//! Legacy catalog import commands.

use std::path::Path;

use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::migrate::{ImportSummary, default_export_path, import_file};
use crate::state::AppState;

/// Default location of the staged legacy catalog export.
#[tauri::command]
pub fn legacy_export_path(app: AppHandle) -> Result<String, AppError> {
    let data_dir = app.path().app_data_dir().map_err(|err| {
        AppError::io_with(
            "Could not resolve the application data directory.",
            err.to_string(),
        )
    })?;
    Ok(default_export_path(&data_dir).display().to_string())
}

/// Import a legacy `donghua-stream` export JSON file on demand.
#[tauri::command]
pub async fn import_legacy_json(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportSummary, AppError> {
    let summary = import_file(state.db().pool(), Path::new(&path)).await?;
    state.complete_legacy_import()?;
    Ok(summary)
}
