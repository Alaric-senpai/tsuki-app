//! Production app updater.
//!
//! Checks the project's public GitHub releases for a newer tag, downloads the
//! matching installer into the app data folder and opens it with the OS's
//! default handler. No signing keys or private endpoints are needed; the user
//! stays in control of the actual installation prompt.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::config::Config;
use crate::error::AppError;
use crate::events;

const API: &str = "https://api.github.com/repos/Alaric-senpai/tsuki-app/releases/latest";

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
pub struct AppRelease {
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub url: String,
    /// serde_json sends JS numbers; ts-rs would otherwise emit `bigint`.
    #[ts(as = "f64")]
    pub size: u64,
    /// Set when the startup check already downloaded the installer in the
    /// background; the dialog then only has to open it.
    #[serde(default)]
    pub downloaded_path: Option<String>,
}

fn normalize_version(raw: &str) -> String {
    raw.trim().trim_start_matches('v').to_string()
}

/// Filename fragments that identify the installer for this platform. Ordered
/// from most specific to least; the first matching asset wins.
fn target_fragments() -> &'static [&'static str] {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        // Fedora/RHEL users get the RPM; Debian users the .deb; everyone else
        // the AppImage. The first matching published asset wins.
        ("linux", "x86_64") => &[
            "linux-x86_64",
            "x86_64.rpm",
            "amd64.deb",
            "x86_64.AppImage",
            "linux",
        ],
        ("linux", "aarch64") => &[
            "linux-aarch64",
            "aarch64.rpm",
            "arm64.deb",
            "aarch64.AppImage",
            "linux",
        ],
        ("windows", "x86_64") => &["x86_64.msi", "x86_64-setup.exe", "windows"],
        ("macos", "aarch64") => &["aarch64.dmg", "macos"],
        ("macos", "x86_64") => &["x86_64.dmg", "macos"],
        _ => &[],
    }
}

fn pick_asset(assets: &[serde_json::Value]) -> Option<(String, String, u64)> {
    let fragments = target_fragments();
    for fragment in fragments {
        if let Some(asset) = assets.iter().find(|item| {
            item.get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.to_ascii_lowercase().contains(fragment))
        }) {
            let name = asset
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("installer")
                .to_string();
            let url = asset
                .get("browser_download_url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            let size = asset
                .get("size")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if !url.is_empty() {
                return Some((name, url, size));
            }
        }
    }
    None
}

/// Read the latest public release. `Ok(None)` means the installed version is
/// already current (or this platform has no published installer).
pub async fn check_for_update() -> Result<Option<AppRelease>, AppError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("Tsuki/{}/app-updater", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| {
            AppError::internal_with("Could not initialise the updater.", error.to_string())
        })?;
    let response = client
        .get(API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| {
            AppError::source_with(
                "Could not reach GitHub for an update check.",
                error.to_string(),
            )
        })?;
    if !response.status().is_success() {
        return Err(AppError::source_with(
            "GitHub could not list the latest release.",
            format!("HTTP {}", response.status()),
        ));
    }
    let body = response.text().await.map_err(|error| {
        AppError::source_with("Could not read the release information.", error.to_string())
    })?;
    let release: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
        AppError::source_with(
            "GitHub returned unreadable release data.",
            error.to_string(),
        )
    })?;
    let version = normalize_version(
        release
            .get("tag_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
    );
    if version.is_empty() || version == normalize_version(env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let assets = release
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some((asset_name, url, size)) = pick_asset(&assets) else {
        return Ok(None);
    };
    Ok(Some(AppRelease {
        version,
        notes: release
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .chars()
            .take(2000)
            .collect(),
        asset_name,
        url,
        size,
        downloaded_path: None,
    }))
}

/// Download the installer into `<app_data>/updates`. Returns the local path so
/// the UI can show where it was saved and open it later.
pub async fn download(data_dir: &Path, release: &AppRelease) -> Result<String, AppError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("Tsuki/{}/app-updater", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| {
            AppError::internal_with("Could not initialise the updater.", error.to_string())
        })?;
    let response = client.get(&release.url).send().await.map_err(|error| {
        AppError::source_with("Could not download the installer.", error.to_string())
    })?;
    if !response.status().is_success() {
        return Err(AppError::source_with(
            "The installer download failed.",
            format!("HTTP {}", response.status()),
        ));
    }
    let bytes = response.bytes().await.map_err(|error| {
        AppError::source_with("The installer download was interrupted.", error.to_string())
    })?;
    let folder = data_dir.join("updates");
    std::fs::create_dir_all(&folder).map_err(|error| {
        AppError::io_with("Could not create the updates folder.", error.to_string())
    })?;
    let path = folder.join(&release.asset_name);
    std::fs::write(&path, &bytes)
        .map_err(|error| AppError::io_with("Could not save the installer.", error.to_string()))?;
    Ok(path.to_string_lossy().to_string())
}

/// Hand a previously downloaded installer to the OS. Only paths inside the
/// app's own `updates` folder are accepted, so a webview-supplied path can
/// never open an unrelated file.
pub fn open(data_dir: &Path, app: &AppHandle, path: &str) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;

    let candidate = Path::new(path);
    if !candidate.is_file() || !candidate.starts_with(data_dir.join("updates")) {
        return Err(AppError::config(
            "That installer is not in Tsuki's updates folder.",
        ));
    }
    app.opener()
        .open_path(path.to_string(), None::<&str>)
        .map_err(|error| AppError::io_with("Could not open the installer.", error.to_string()))
}

/// Download the installer and hand it to the OS in one step (Settings card).
pub async fn download_and_open(
    data_dir: &Path,
    app: &AppHandle,
    release: &AppRelease,
) -> Result<String, AppError> {
    let path = download(data_dir, release).await?;
    open(data_dir, app, &path)?;
    Ok(path)
}

/// True when the named on/off setting is enabled (defaults to `fallback`).
pub fn setting_enabled(config: &Config, key: &str, fallback: bool) -> bool {
    match config.settings.get(key).map(String::as_str) {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => fallback,
    }
}

/// Startup check. Emits `update://available` so the UI can show the changelog
/// dialog; when automatic downloads are enabled the installer is fetched in
/// the background first and the dialog only has to open it.
pub async fn startup_check(
    app: AppHandle,
    data_dir: std::path::PathBuf,
    auto_download: bool,
) -> Result<(), AppError> {
    let Some(mut release) = check_for_update().await? else {
        return Ok(());
    };
    if auto_download && let Ok(path) = download(&data_dir, &release).await {
        release.downloaded_path = Some(path);
    }
    if let Err(error) = app.emit(events::UPDATE_AVAILABLE, &release) {
        log::warn!("could not emit {}: {error}", events::UPDATE_AVAILABLE);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_tags_are_normalised() {
        assert_eq!(normalize_version(" v1.2.3 "), "1.2.3");
        assert_eq!(normalize_version("0.4.0"), "0.4.0");
    }

    #[test]
    fn installer_assets_are_selected_by_platform() {
        let assets = serde_json::json!([
            {"name": "source.tar.gz", "browser_download_url": "https://example.test/src", "size": 1},
            {"name": "tsuki_0.2.0_amd64.deb", "browser_download_url": "https://example.test/deb", "size": 42}
        ]);
        let array = assets.as_array().expect("array");
        if !target_fragments().is_empty() {
            let picked = pick_asset(array).expect("a matching asset");
            assert_eq!(picked.2, 42);
        }
    }
}
