//! Detection and pull of a legacy `donghua-stream` install.
//!
//! The shipped app never links a PostgreSQL client. When a legacy install is
//! still on the machine we can drive its `export_legacy.py` helper (which owns
//! the psycopg2 dependency) and then import the staged JSON through the normal
//! migration path.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::legacy_config_path;
use crate::error::AppError;
use crate::migrate::{ImportSummary, default_export_path, import_file};
use crate::state::AppState;

/// Names the legacy CLI/GUI may be installed under.
const LEGACY_BINARIES: [&str; 3] = ["donghua-stream", "donghua-gui", "donghua"];

/// What the Settings "Legacy" section needs to decide whether to show itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyStatus {
    /// A legacy config file or executable was found on this machine.
    pub detected: bool,
    pub config_path: Option<String>,
    /// Where a staged export is read from / written to.
    pub export_path: String,
    /// A staged export file already exists at `export_path`.
    pub export_ready: bool,
    pub series_count: i64,
    pub episode_count: i64,
    /// Legacy executables found on `PATH`.
    pub binaries: Vec<String>,
    /// The `export_legacy.py` helper, if one was found.
    pub export_script: Option<String>,
    pub imported: bool,
    pub imported_at: Option<String>,
    /// Human-readable explanation of what can be done next.
    pub note: String,
}

/// Result of a user-triggered pull.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullSummary {
    /// False when no export script was available, so nothing was run.
    pub exported: bool,
    pub path: String,
    /// Captured stdout/stderr from the export script.
    pub log: String,
    pub summary: ImportSummary,
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path().app_data_dir().map_err(|err| {
        AppError::io_with(
            "Could not resolve the application data directory.",
            err.to_string(),
        )
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

/// Look for a legacy executable on `PATH`.
fn find_binaries() -> Vec<PathBuf> {
    let path_var = match std::env::var_os("PATH") {
        Some(value) => value,
        None => return Vec::new(),
    };

    let mut found = Vec::new();
    for dir in std::env::split_paths(&path_var) {
        for name in LEGACY_BINARIES {
            let direct = dir.join(name);
            if is_executable(&direct) {
                found.push(direct);
                continue;
            }
            if cfg!(windows) {
                for suffix in ["exe", "cmd", "bat"] {
                    let candidate = dir.join(format!("{name}.{suffix}"));
                    if candidate.is_file() {
                        found.push(candidate);
                        break;
                    }
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Locate a legacy `export_legacy.py` helper we are allowed to run.
fn find_export_script(binaries: &[PathBuf]) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("TSUKI_LEGACY_EXPORT")
        .or_else(|| std::env::var_os("DONGHUA_EXPORT_SCRIPT"))
        .filter(|value| !value.is_empty())
    {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Some(home) = home_dir() {
        for rel in [
            ".local/share/donghua-stream/tools/export_legacy.py",
            ".local/share/donghua-stream/export_legacy.py",
            "donghua-stream/tools/export_legacy.py",
            "donghua-stream/export_legacy.py",
            ".local/share/ytdl/tools/export_legacy.py",
            ".local/share/ytdl/export_legacy.py",
        ] {
            candidates.push(home.join(rel));
        }
    }
    // pipx installs keep the script beside the package in the same tree as the
    // launcher, which lives in `<prefix>/bin`.
    for binary in binaries {
        if let Some(prefix) = binary.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share/donghua-stream/tools/export_legacy.py"));
            candidates.push(prefix.join("share/donghua-stream/export_legacy.py"));
        }
    }

    candidates.into_iter().find(|path| path.is_file())
}

/// Count series and episodes in a staged export without depending on the
/// private migration structs.
fn count_entries(raw: &str) -> (i64, i64) {
    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => return (0, 0),
    };
    let series = match parsed.get("series").and_then(|value| value.as_array()) {
        Some(items) => items,
        None => return (0, 0),
    };
    let episodes: i64 = series
        .iter()
        .map(|item| {
            item.get("episodes")
                .and_then(|value| value.as_array())
                .map(|items| items.len() as i64)
                .unwrap_or(0)
        })
        .sum();
    (series.len() as i64, episodes)
}

/// Report whether a legacy install is present and what the app can do with it.
#[tauri::command]
pub fn legacy_status(app: AppHandle, state: State<'_, AppState>) -> Result<LegacyStatus, AppError> {
    let dir = data_dir(&app)?;
    let export_path = default_export_path(&dir);
    let config = legacy_config_path().filter(|path| path.is_file());
    let binaries = find_binaries();
    let export_script = find_export_script(&binaries);

    let (series_count, episode_count) = match std::fs::read_to_string(&export_path) {
        Ok(raw) => count_entries(&raw),
        Err(_) => (0, 0),
    };
    let export_ready = export_path.is_file();

    let app_config = state.config()?;
    let imported = app_config.meta.legacy_data_imported;
    let imported_at = app_config.meta.legacy_data_imported_at.clone();

    let detected = config.is_some() || !binaries.is_empty() || export_script.is_some();
    let note = if !detected {
        "No legacy donghua-stream install was found on this machine.".to_string()
    } else if export_script.is_none() {
        "The legacy install was found but its export helper is missing. Run tools/export_legacy.py \
         from the legacy checkout, then re-check."
            .to_string()
    } else if export_ready {
        format!("A staged export is ready: {series_count} series, {episode_count} episodes.")
    } else {
        "The legacy install can be exported and imported now.".to_string()
    };

    Ok(LegacyStatus {
        detected,
        config_path: config.map(|path| path.display().to_string()),
        export_path: export_path.display().to_string(),
        export_ready,
        series_count,
        episode_count,
        binaries: binaries
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        export_script: export_script.map(|path| path.display().to_string()),
        imported,
        imported_at,
        note,
    })
}

/// Run the legacy export helper (when available) and import the result.
///
/// Without an export helper this imports a staged file that the user created
/// manually, so the action is never a silent no-op.
#[tauri::command]
pub async fn pull_legacy_data(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PullSummary, AppError> {
    let dir = data_dir(&app)?;
    let export_path = default_export_path(&dir);
    let binaries = find_binaries();
    let script = find_export_script(&binaries);

    let mut exported = false;
    let mut log = String::new();

    if let Some(script) = script {
        let output = tokio::process::Command::new(python_binary())
            .arg(&script)
            .arg("--out")
            .arg(&export_path)
            .output()
            .await
            .map_err(|err| {
                AppError::io_with(
                    "Could not run the legacy export helper.",
                    format!("{}: {err}", script.display()),
                )
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        log = match (stdout.is_empty(), stderr.is_empty()) {
            (false, false) => format!("{stdout}\n{stderr}"),
            (false, true) => stdout.clone(),
            (true, false) => stderr.clone(),
            (true, true) => String::new(),
        };
        if !output.status.success() {
            return Err(AppError::io_with(
                "The legacy export helper failed.",
                if log.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    log.clone()
                },
            ));
        }
        exported = true;
    }

    if !export_path.is_file() {
        return Err(AppError::io_with(
            "No legacy export is available.",
            format!(
                "Expected a staged file at {}. Run tools/export_legacy.py from the legacy \
                 checkout first.",
                export_path.display()
            ),
        ));
    }

    let summary = import_file(state.db().pool(), Path::new(&export_path)).await?;
    state.complete_legacy_import()?;
    log::info!(
        "legacy pull: {} series ({} new), {} episodes ({} new, {} updated)",
        summary.series_seen,
        summary.series_created,
        summary.episodes_seen,
        summary.episodes_added,
        summary.episodes_updated
    );

    Ok(PullSummary {
        exported,
        path: export_path.display().to_string(),
        log,
        summary,
    })
}

/// Interpreter used for the export helper.
fn python_binary() -> String {
    if let Some(explicit) = std::env::var_os("TSUKI_PYTHON").filter(|value| !value.is_empty()) {
        return PathBuf::from(explicit).display().to_string();
    }
    if cfg!(windows) {
        "python".to_string()
    } else {
        "python3".to_string()
    }
}
