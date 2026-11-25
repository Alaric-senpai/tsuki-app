//! Library IPC commands: catalog access for the Library and Home pages.
//!
//! Banner images never cross the filesystem boundary: the command reads the
//! cached file and inlines it as a data URL in the response view.

use std::fs;
use std::path::{Component, Path};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::db::models::{
    Episode, EpisodeWithSeries, LibraryStats, SeriesPoster, SeriesSummary, SeriesSummaryView,
};
use crate::db::queries;
use crate::error::AppError;
use crate::migrate::backfill_legacy_banners;
use crate::state::AppState;

const MAX_BANNER_BYTES: u64 = 12 * 1024 * 1024;
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[tauri::command]
pub async fn list_series(state: State<'_, AppState>) -> Result<Vec<SeriesSummaryView>, AppError> {
    let rows = queries::list_series(state.db().pool()).await?;
    Ok(rows
        .into_iter()
        .map(|row| series_view(state.data_dir(), row))
        .collect())
}

#[tauri::command]
pub async fn get_series(
    state: State<'_, AppState>,
    query: String,
) -> Result<Option<SeriesSummaryView>, AppError> {
    let row = queries::get_series(state.db().pool(), &query).await?;
    Ok(row.map(|row| series_view(state.data_dir(), row)))
}

/// Edit a series' name, save folder and description without touching its banner.
#[tauri::command]
pub async fn update_series_details(
    state: State<'_, AppState>,
    id: i64,
    name: Option<String>,
    save_dir: Option<String>,
    description: Option<String>,
) -> Result<SeriesSummaryView, AppError> {
    let pool = state.db().pool();
    let changed = queries::update_series(
        pool,
        id,
        name.as_deref(),
        save_dir.as_deref(),
        description.as_deref(),
    )
    .await?;
    if !changed {
        return Err(AppError::db("That series no longer exists."));
    }
    series_view_by_id(pool, state.data_dir(), id).await
}

/// Copy a user-chosen image into the app cache and make it the series banner.
#[tauri::command]
pub async fn set_series_banner(
    state: State<'_, AppState>,
    id: i64,
    source_path: String,
) -> Result<SeriesSummaryView, AppError> {
    let pool = state.db().pool();
    let source = queries::expand_dir(&source_path);
    let relative = copy_banner(&source, state.data_dir(), id)?;
    let changed = queries::set_series_banner(pool, id, &relative, "custom").await?;
    if !changed {
        return Err(AppError::db("That series no longer exists."));
    }
    series_view_by_id(pool, state.data_dir(), id).await
}

/// Download a remote poster into the banner cache and attach it to a series.
///
/// Used when a series is created from an online source so the library shows
/// real artwork offline instead of the placeholder. Failures are returned so
/// the caller can log them — artwork is never worth blocking an import.
pub async fn fetch_remote_banner(
    pool: &SqlitePool,
    data_dir: &Path,
    id: i64,
    url: &str,
    source: &str,
) -> Result<bool, AppError> {
    let url = url.trim();
    if url.is_empty() || url.starts_with("data:") {
        return Ok(false);
    }

    let client = reqwest::Client::builder()
        .user_agent("Tsuki/0.1 (Donghua Stream)")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| {
            AppError::source_with("Could not build the image downloader.", err.to_string())
        })?;
    let response = client.get(url).send().await.map_err(|err| {
        AppError::source_with("Could not download the series banner.", err.to_string())
    })?;
    if !response.status().is_success() {
        return Err(AppError::source_with(
            "Could not download the series banner.",
            format!("HTTP {}", response.status()),
        ));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    let extension = if content_type.contains("png") {
        "png".to_string()
    } else if content_type.contains("webp") {
        "webp".to_string()
    } else if content_type.contains("jpeg") || content_type.contains("jpg") {
        "jpg".to_string()
    } else {
        let path = url.split(['?', '#']).next().unwrap_or(url);
        let guessed = Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_lowercase();
        match guessed.as_str() {
            "jpg" | "jpeg" => "jpg".to_string(),
            "png" | "webp" => guessed,
            _ => {
                return Err(AppError::source_with(
                    "Unsupported banner image format.",
                    if content_type.is_empty() {
                        url.to_string()
                    } else {
                        content_type
                    },
                ));
            }
        }
    };

    let bytes = response.bytes().await.map_err(|err| {
        AppError::source_with("Could not read the series banner.", err.to_string())
    })?;
    if bytes.len() as u64 > MAX_BANNER_BYTES {
        return Err(AppError::io("The banner image is larger than 12 MB."));
    }

    let cache_dir = data_dir.join("cache").join("banners");
    fs::create_dir_all(&cache_dir).map_err(|err| {
        AppError::io_with(
            "Could not create the banner cache directory.",
            err.to_string(),
        )
    })?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let temp = cache_dir.join(format!(".{id}-{stamp}.{extension}"));
    fs::write(&temp, &bytes)
        .map_err(|err| AppError::io_with("Could not save the banner image.", err.to_string()))?;
    let relative = copy_banner(&temp, data_dir, id);
    let _ = fs::remove_file(&temp);
    let relative = relative?;
    queries::set_series_banner(pool, id, &relative, source).await
}

/// Fetch and cache a remote poster for an existing library entry.
#[tauri::command]
pub async fn set_series_banner_from_url(
    state: State<'_, AppState>,
    id: i64,
    url: String,
    source: String,
) -> Result<crate::db::models::SeriesSummaryView, AppError> {
    let pool = state.db().pool();
    fetch_remote_banner(pool, state.data_dir(), id, &url, &source).await?;
    series_view_by_id(pool, state.data_dir(), id).await
}

/// Fetch and cache posters for library entries that have a remote page but
/// no banner yet. Legacy/local entries are left alone.
#[tauri::command]
pub async fn resync_remote_banners(state: State<'_, AppState>) -> Result<i64, AppError> {
    let pool = state.db().pool();
    let targets = queries::series_without_banners(pool).await?;
    let mut fixed = 0i64;
    for (id, source_key, source_url) in targets {
        // Only DonghuaStream exposes a parseable poster in a known shape.
        if !source_key.eq_ignore_ascii_case("donghuastream") {
            continue;
        }
        let minutes = state.config()?.explore.cache_minutes;
        match crate::sources::donghuastream::browse::series(&source_url, minutes, true).await {
            Ok(remote) => {
                match fetch_remote_banner(
                    pool,
                    state.data_dir(),
                    id,
                    &remote.thumbnail,
                    &source_key,
                )
                .await
                {
                    Ok(true) => fixed += 1,
                    Ok(false) => log::warn!("series {id} still has no banner after resync"),
                    Err(error) => log::warn!(
                        "could not cache banner for series {id}: {}",
                        error.message()
                    ),
                }
            }
            Err(error) => log::warn!(
                "could not resolve series {id} for banner resync: {}",
                error.message()
            ),
        }
    }
    if fixed > 0 {
        log::info!("cached {fixed} remote banner(s)");
    }
    Ok(fixed)
}

/// Open a native file picker and return the chosen image path, if any.
/// Returns `None` when the user cancels the dialog.
#[tauri::command]
pub async fn pick_banner_file(app: AppHandle) -> Result<Option<String>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    app.dialog().file().pick_file(move |file| {
        let result = file
            .and_then(|file| file.into_path().ok())
            .map(|path| path.to_string_lossy().to_string());
        let _ = tx.send(result);
    });
    rx.await
        .map_err(|err| AppError::io_with("The file picker was interrupted.", err.to_string()))
}

/// Re-run the legacy banner backfill for series that still have no banner.
#[tauri::command]
pub async fn resync_series_banners(state: State<'_, AppState>) -> Result<i64, AppError> {
    backfill_legacy_banners(state.db().pool(), state.data_dir()).await
}

#[tauri::command]
pub async fn list_episodes(
    state: State<'_, AppState>,
    series_id: i64,
    status: Option<String>,
) -> Result<Vec<Episode>, AppError> {
    queries::list_episodes(state.db().pool(), series_id, status.as_deref()).await
}

/// Create a new episode for a series.
#[tauri::command]
pub async fn create_episode(
    state: State<'_, AppState>,
    series_id: i64,
    video_id: String,
    url: String,
    episode_number: Option<i64>,
    title: String,
    status: String,
) -> Result<Episode, AppError> {
    queries::create_episode(
        state.db().pool(),
        series_id,
        &video_id,
        &url,
        episode_number,
        &title,
        &status,
    )
    .await
}

/// Update an episode's editable fields. Only the provided fields are changed.
#[tauri::command]
pub async fn update_episode(
    state: State<'_, AppState>,
    id: i64,
    title: Option<String>,
    status: Option<String>,
    video_id: Option<String>,
    url: Option<String>,
    episode_number: Option<i64>,
) -> Result<Episode, AppError> {
    queries::update_episode(
        state.db().pool(),
        id,
        title.as_deref(),
        status.as_deref(),
        video_id.as_deref(),
        url.as_deref(),
        episode_number,
    )
    .await
}

/// Delete an episode and return the deleted row.
#[tauri::command]
pub async fn delete_episode(state: State<'_, AppState>, id: i64) -> Result<Episode, AppError> {
    queries::delete_episode(state.db().pool(), id).await
}

/// Episodes from across the library, used by Home and Downloads surfaces.
#[tauri::command]
pub async fn recent_episodes(
    state: State<'_, AppState>,
    status: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EpisodeWithSeries>, AppError> {
    queries::episodes_with_series(state.db().pool(), status.as_deref(), limit).await
}

#[tauri::command]
pub async fn series_posters(state: State<'_, AppState>) -> Result<Vec<SeriesPoster>, AppError> {
    queries::series_poster_ids(state.db().pool()).await
}

#[tauri::command]
pub async fn library_stats(state: State<'_, AppState>) -> Result<LibraryStats, AppError> {
    queries::library_stats(state.db().pool()).await
}

/// Re-scan every series folder and reconcile the `downloaded.txt` archives.
/// Returns the number of episode rows whose status changed.
#[tauri::command]
pub async fn sync_archives(state: State<'_, AppState>) -> Result<i64, AppError> {
    let pool = state.db().pool();
    let repair = queries::repair_archives(pool).await?;
    log::info!(
        "archive repair: {} folder(s) scanned, {} archive(s) created",
        repair.folders_scanned,
        repair.archives_created
    );
    queries::sync_all_archives(pool).await
}

async fn series_view_by_id(
    pool: &SqlitePool,
    data_dir: &Path,
    id: i64,
) -> Result<SeriesSummaryView, AppError> {
    let row = queries::get_series_summary_by_id(pool, id).await?;
    row.map(|row| series_view(data_dir, row))
        .ok_or_else(|| AppError::db("That series no longer exists."))
}

fn series_view(data_dir: &Path, row: SeriesSummary) -> SeriesSummaryView {
    let banner_url = row
        .banner_path
        .as_deref()
        .and_then(|relative| banner_data_url(data_dir, relative));
    SeriesSummaryView {
        id: row.id,
        key: row.key,
        name: row.name,
        save_dir: row.save_dir,
        description: row.description,
        banner_url,
        banner_source: row.banner_source,
        created_at: row.created_at,
        updated_at: row.updated_at,
        total_episodes: row.total_episodes,
        downloaded_episodes: row.downloaded_episodes,
        pending_episodes: row.pending_episodes,
        failed_episodes: row.failed_episodes,
        queued_episodes: row.queued_episodes,
        source_key: row.source_key,
        source_url: row.source_url,
        check_updates: row.check_updates != 0,
        update_count: row.update_count,
        last_checked_at: row.last_checked_at,
    }
}

/// Validate, copy and name a banner under `<data_dir>/cache/banners`.
/// Returns the path stored in the database (relative to the data dir).
fn copy_banner(source: &Path, data_dir: &Path, id: i64) -> Result<String, AppError> {
    let metadata = fs::metadata(source)
        .map_err(|err| AppError::io_with("Could not read the banner image.", err.to_string()))?;
    if !metadata.is_file() {
        return Err(AppError::io("The banner must be an image file."));
    }
    if metadata.len() > MAX_BANNER_BYTES {
        return Err(AppError::io("The banner image is larger than 12 MB."));
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if banner_mime_from_extension(&extension).is_none() {
        return Err(AppError::io(
            "Unsupported image format. Use a JPG, PNG or WEBP file.",
        ));
    }

    let cache_dir = data_dir.join("cache").join("banners");
    fs::create_dir_all(&cache_dir).map_err(|err| {
        AppError::io_with(
            "Could not create the banner cache directory.",
            err.to_string(),
        )
    })?;
    let target = cache_dir.join(format!("{id}.{extension}"));
    fs::copy(source, &target)
        .map_err(|err| AppError::io_with("Could not save the banner image.", err.to_string()))?;
    Ok(format!("cache/banners/{id}.{extension}"))
}

fn banner_data_url(data_dir: &Path, relative: &str) -> Option<String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)?;
    let mime = banner_mime_from_extension(&extension)?;
    let bytes = fs::read(data_dir.join(path)).ok()?;
    Some(format!("data:{mime};base64,{}", base64_encode(&bytes)))
}

fn banner_mime_from_extension(extension: &str) -> Option<&'static str> {
    match extension.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Minimal RFC 4648 base64 encoder (no new dependency for one small helper).
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let packed = (a << 16) | (b << 8) | c;
        out.push(BASE64_ALPHABET[((packed >> 18) & 63) as usize] as char);
        out.push(BASE64_ALPHABET[((packed >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[((packed >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(packed & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn base64_handles_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn banner_urls_are_limited_to_the_banner_cache() {
        let dir = PathBuf::from("/tmp/tsuki-data");
        assert!(banner_data_url(&dir, "cache/banners/1.jpg").is_none());
        assert!(banner_data_url(&dir, "../../secret.png").is_none());
        assert!(banner_data_url(&dir, "/etc/passwd").is_none());
        assert!(banner_data_url(&dir, "cache/banners/1.txt").is_none());
        assert_eq!(banner_mime_from_extension("JPEG"), Some("image/jpeg"));
    }
}
