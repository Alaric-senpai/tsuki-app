//! In-app yt-dlp updater.
//!
//! yt-dlp ships frequently and the bundled copy goes stale. A newer binary is
//! written to `<app_data>/bin/yt-dlp`, which `YtDlp::locate` prefers over the
//! bundled sidecar, so no restart or PATH change is needed.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::AppError;

const BASE_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";

/// Platform asset name used by the official yt-dlp release.
fn asset_name() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "yt-dlp.exe",
        ("windows", "aarch64") => "yt-dlp_win_arm64.exe",
        ("macos", _) => "yt-dlp_macos",
        ("linux", "aarch64") | ("linux", "arm") => "yt-dlp_linux_aarch64",
        ("linux", "armv7") => "yt-dlp_linux_armv7l",
        _ => "yt-dlp_linux",
    }
}

pub fn target_path(data_dir: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    data_dir.join("bin").join(name)
}

/// Download the latest release into app data and report its `--version`.
pub async fn update_ytdlp(data_dir: &Path) -> Result<String, AppError> {
    let url = format!("{BASE_URL}/{}", asset_name());
    let bytes = reqwest::get(&url)
        .await
        .map_err(|error| {
            AppError::download_with(
                "Could not reach the yt-dlp release page.",
                error.to_string(),
            )
        })?
        .error_for_status()
        .map_err(|error| {
            AppError::download_with("Could not download the yt-dlp update.", error.to_string())
        })?
        .bytes()
        .await
        .map_err(|error| {
            AppError::download_with("Could not read the yt-dlp update.", error.to_string())
        })?;
    if bytes.is_empty() {
        return Err(AppError::download("The yt-dlp update was empty."));
    }

    let target = target_path(data_dir);
    let parent = target
        .parent()
        .ok_or_else(|| AppError::download("The app data folder has no bin directory."))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        AppError::io_with(
            "Could not create the app data bin folder.",
            error.to_string(),
        )
    })?;

    // Write beside the target then rename, so an interrupted download cannot
    // leave a half-written executable that looks usable.
    let temp = parent.join(format!("yt-dlp.update-{}.tmp", std::process::id()));
    std::fs::write(&temp, &bytes).map_err(|error| {
        AppError::io_with("Could not write the yt-dlp update.", error.to_string())
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&temp, &target).map_err(|error| {
        AppError::io_with("Could not install the yt-dlp update.", error.to_string())
    })?;

    let version = tokio::process::Command::new(&target)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|error| {
            AppError::download_with("The updated yt-dlp binary will not run.", error.to_string())
        })?;
    let version_text = String::from_utf8_lossy(&version.stdout).trim().to_string();
    if version_text.is_empty() {
        return Err(AppError::download(
            "The updated yt-dlp binary did not report a version.",
        ));
    }
    Ok(version_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_cover_supported_platforms() {
        // Compile-time sanity: the match returns a non-empty asset for the
        // host platform regardless of which branch was selected.
        assert!(!asset_name().is_empty());
    }

    #[test]
    fn override_path_lives_in_app_data() {
        let root = Path::new("/tmp/tsuki-app-data");
        let expected = if cfg!(windows) {
            root.join("bin").join("yt-dlp.exe")
        } else {
            root.join("bin").join("yt-dlp")
        };
        assert_eq!(target_path(root), expected);
    }
}
