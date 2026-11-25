//! One-shot migration from the legacy `donghua-stream` catalog.
//!
//! The legacy app keeps its catalog in PostgreSQL. Tsuki ships with SQLite and
//! must not link a Postgres client, so `tools/export_legacy.py` dumps the old
//! catalog to JSON and this module imports that file. The import runs at
//! startup until `meta.legacy_data_imported` is recorded in the config.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::config::{Config, ConfigStore, now_iso8601};
use crate::db::queries;
use crate::error::AppError;

/// File name looked for inside the application data directory.
pub const LEGACY_EXPORT_FILE: &str = "legacy-export.json";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub series_seen: i64,
    pub series_created: i64,
    pub episodes_seen: i64,
    pub episodes_added: i64,
    pub episodes_updated: i64,
}

impl ImportSummary {
    fn record_series(&mut self, created: bool) {
        self.series_seen += 1;
        if created {
            self.series_created += 1;
        }
    }

    fn record_episode(&mut self, inserted: bool) {
        self.episodes_seen += 1;
        if inserted {
            self.episodes_added += 1;
        } else {
            self.episodes_updated += 1;
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyExport {
    #[serde(default)]
    series: Vec<LegacySeries>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySeries {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    save_dir: String,
    #[serde(default)]
    description: String,
    /// Explicit poster choice in newer exports; falls back to the first episode.
    #[serde(default)]
    poster_video_id: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    episodes: Vec<LegacyEpisode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyEpisode {
    #[serde(default)]
    video_id: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    episode_number: Option<i64>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    downloaded_at: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
}

/// Import a legacy export held in memory.
pub async fn import_str(pool: &SqlitePool, raw: &str) -> Result<ImportSummary, AppError> {
    let export: LegacyExport = serde_json::from_str(raw).map_err(|err| {
        AppError::config_with("The legacy export file is not valid JSON.", err.to_string())
    })?;

    let mut summary = ImportSummary::default();
    for series in &export.series {
        let name = series.name.trim();
        if name.is_empty() {
            continue;
        }
        let key = series
            .key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let existing = match key {
            Some(candidate) => queries::find_series_by_key(pool, candidate).await?,
            None => None,
        };
        let row = match existing {
            Some(row) => {
                summary.record_series(false);
                row
            }
            None => {
                let created = queries::create_series_with_key(
                    pool,
                    key.unwrap_or(""),
                    name,
                    &series.save_dir,
                    &series.description,
                )
                .await?;
                summary.record_series(true);
                created
            }
        };

        let fallback_created = series
            .created_at
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(now_iso8601);
        for episode in &series.episodes {
            let created_at = episode
                .created_at
                .as_deref()
                .filter(|value| !value.is_empty())
                .unwrap_or(&fallback_created);
            let inserted = queries::upsert_imported_episode(
                pool,
                row.id,
                &episode.video_id,
                &episode.url,
                episode.episode_number,
                &episode.title,
                &episode.status,
                episode.downloaded_at.as_deref(),
                created_at,
            )
            .await?;
            summary.record_episode(inserted);
        }
    }
    Ok(summary)
}

/// Import a legacy export from disk.
pub async fn import_file(pool: &SqlitePool, path: &Path) -> Result<ImportSummary, AppError> {
    let raw = std::fs::read_to_string(path).map_err(|err| {
        AppError::io_with("Could not read the legacy export file.", err.to_string())
    })?;
    import_str(pool, &raw).await
}

/// Default location of the legacy export inside the application data dir.
pub fn default_export_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LEGACY_EXPORT_FILE)
}

/// Import the pending legacy export once, then record the flag.
pub async fn run_pending_migration(
    pool: &SqlitePool,
    store: &ConfigStore,
    config: &mut Config,
    data_dir: &Path,
) -> Result<Option<ImportSummary>, AppError> {
    if config.meta.legacy_data_imported {
        return Ok(None);
    }
    let path = default_export_path(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let summary = import_file(pool, &path).await?;
    // Resolve legacy artwork once, during the initial import only.
    backfill_legacy_banners(pool, data_dir).await?;
    config.meta.legacy_data_imported = true;
    config.meta.legacy_data_imported_at = Some(now_iso8601());
    store.save(config)?;
    log::info!(
        "legacy catalog imported: {} series ({} new), {} episodes ({} new, {} updated)",
        summary.series_seen,
        summary.series_created,
        summary.episodes_seen,
        summary.episodes_added,
        summary.episodes_updated
    );
    Ok(Some(summary))
}

/// Copy cached legacy thumbnails into the app banner cache.
///
/// This runs once during the initial legacy import (see [`run_pending_migration`]),
/// not on every startup. Catalog rows can already exist while their banners are
/// still missing. A user-chosen (`custom`) banner is never replaced, and the
/// config flags are left untouched. Re-runs are idempotent: series that already
/// have a banner are skipped.
pub async fn backfill_legacy_banners(pool: &SqlitePool, data_dir: &Path) -> Result<i64, AppError> {
    let export_path = default_export_path(data_dir);
    let raw = match fs::read_to_string(&export_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => {
            log::warn!(
                "could not read {} for banner backfill: {err}",
                export_path.display()
            );
            return Ok(0);
        }
    };
    let export: LegacyExport = match serde_json::from_str(&raw) {
        Ok(export) => export,
        Err(err) => {
            log::warn!("could not parse the legacy export for banners: {err}");
            return Ok(0);
        }
    };

    let thumb_dir = legacy_thumb_dir();
    let cache_dir = data_dir.join("cache").join("banners");
    let mut copied = 0i64;
    for series in &export.series {
        let Some(key) = series
            .key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(video_id) = poster_video_id(series) else {
            continue;
        };
        let Some(row) = queries::find_series_by_key(pool, key).await? else {
            continue;
        };
        if row.banner_source == "custom" || row.banner_path.is_some() {
            continue;
        }

        let source = thumb_dir.join(format!("{video_id}.jpg"));
        let Ok(metadata) = fs::metadata(&source) else {
            continue;
        };
        // YouTube placeholders are ~1.3 KB; real thumbnails are ~29 KB.
        if !metadata.is_file() || metadata.len() <= 4096 {
            continue;
        }
        fs::create_dir_all(&cache_dir).map_err(|err| {
            AppError::io_with(
                "Could not create the banner cache directory.",
                err.to_string(),
            )
        })?;
        let target = cache_dir.join(format!("{}.jpg", row.id));
        fs::copy(&source, &target).map_err(|err| {
            AppError::io_with("Could not cache the legacy banner.", err.to_string())
        })?;
        let relative = format!("cache/banners/{}.jpg", row.id);
        if queries::backfill_series_banner(pool, row.id, &relative, "legacy").await? {
            copied += 1;
        }
    }
    log::info!("legacy banner backfill copied {copied} banner(s)");
    Ok(copied)
}

fn poster_video_id(series: &LegacySeries) -> Option<&str> {
    series
        .poster_video_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            series.episodes.iter().find_map(|episode| {
                let video_id = episode.video_id.trim();
                (!video_id.is_empty()).then_some(video_id)
            })
        })
        .filter(|value| is_safe_video_id(value))
}

/// Video IDs become file names, so reject anything but the YouTube alphabet.
fn is_safe_video_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn legacy_thumb_dir() -> PathBuf {
    let cache_root = std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".cache")
        });
    cache_root.join("donghua-stream").join("thumbs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_ids_are_safe_file_names() {
        assert!(is_safe_video_id("dQw4w9WgXcQ"));
        assert!(is_safe_video_id("abc-123_XYZ"));
        assert!(!is_safe_video_id(""));
        assert!(!is_safe_video_id("../etc/passwd"));
        assert!(!is_safe_video_id("id with space"));
    }

    #[test]
    fn explicit_poster_wins_over_first_episode() {
        let series = LegacySeries {
            key: None,
            name: String::new(),
            save_dir: String::new(),
            description: String::new(),
            poster_video_id: Some(" chosen-id ".to_string()),
            created_at: None,
            episodes: vec![LegacyEpisode {
                video_id: "first-id".to_string(),
                url: String::new(),
                episode_number: None,
                title: String::new(),
                status: String::new(),
                downloaded_at: None,
                created_at: None,
            }],
        };
        assert_eq!(poster_video_id(&series), Some("chosen-id"));
    }
}
