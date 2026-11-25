//! Production app updater.
//!
//! Checks the project's public GitHub releases for a newer tag, downloads the
//! matching installer into the app data folder and opens it with the OS's
//! default handler. No signing keys or private endpoints are needed; the user
//! stays in control of the actual installation prompt.

use serde::Serialize;
use ts_rs::TS;

use crate::error::AppError;

const API: &str = "https://api.github.com/repos/Alaric-senpai/tsuki-app/releases/latest";

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
pub struct AppRelease {
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub url: String,
    /// serde_json sends JS numbers; ts-rs would otherwise emit `bigint`.
    #[ts(as = "f64")]
    pub size: u64,
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
    }))
}

/// Download the installer into `<app_data>/updates` and hand it to the OS.
/// Returns the local path so the UI can show where it was saved.
pub async fn download_and_open(
    data_dir: &std::path::Path,
    app: &tauri::AppHandle,
    release: &AppRelease,
) -> Result<String, AppError> {
    use tauri_plugin_opener::OpenerExt;

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
    let display = path.to_string_lossy().to_string();
    app.opener()
        .open_path(display.clone(), None::<&str>)
        .map_err(|error| AppError::io_with("Could not open the installer.", error.to_string()))?;
    Ok(display)
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
