//! Library queries, ported from the legacy `db.py` helpers.
//!
//! Every function mirrors its legacy counterpart: same ordering, same
//! upsert/merge semantics, same archive + on-disk reconciliation rules.
//! Timestamps are ISO-8601 UTC strings from [`now_iso8601`].

#![allow(dead_code)] // write APIs are consumed by later phases

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::config::now_iso8601;
use crate::error::AppError;
use crate::media_ids::{
    MEDIA_EXTENSIONS, extract_video_id, is_intermediate_media_name, media_stem, normalize_stem,
    safe_title,
};

use super::models::{
    ArchiveRepairResult, BulkAddResult, EntriesAddResult, Episode, EpisodeEntry, EpisodeWithSeries,
    LibraryStats, Series, SeriesPoster, SeriesSummary,
};

/// Finished media smaller than this is treated as a leftover fragment.
const MIN_MEDIA_BYTES: u64 = 64 * 1024;

const SUMMARY_SELECT: &str = "SELECT s.id, s.key, s.name, s.save_dir, s.description, \
     s.banner_path, s.banner_source, s.created_at, s.updated_at, s.source_key, \
     s.source_url, s.check_updates, s.update_count, s.last_checked_at, \
     COUNT(e.id) AS total_episodes, \
     COALESCE(SUM(CASE WHEN e.status = 'downloaded' THEN 1 ELSE 0 END), 0) \
         AS downloaded_episodes, \
     COALESCE(SUM(CASE WHEN e.id IS NOT NULL AND e.status <> 'downloaded' THEN 1 ELSE 0 END), 0) \
         AS pending_episodes, \
     COALESCE(SUM(CASE WHEN e.status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_episodes, \
     COALESCE(SUM(CASE WHEN e.status = 'queued' THEN 1 ELSE 0 END), 0) AS queued_episodes \
     FROM series s LEFT JOIN episodes e ON e.series_id = s.id";

const EPISODE_COLUMNS: &str = "id, series_id, video_id, url, episode_number, title, status, \
     downloaded_at, created_at";

const CROSS_LIBRARY_EPISODE_COLUMNS: &str = "e.id, e.series_id, s.name AS series_name, e.video_id, e.url, e.episode_number, \
     e.title, e.status, e.downloaded_at, e.created_at";

fn db_error(err: sqlx::Error) -> AppError {
    AppError::db_with("The library database request failed.", err.to_string())
}

// ---- slugs and paths ---------------------------------------------------------

/// Lowercase, collapse runs of non-alphanumerics to `_`, trim underscores.
pub fn sanitize_slug(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    let mut slug = String::with_capacity(lowered.len());
    let mut pending_separator = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_separator && !slug.is_empty() {
                slug.push('_');
            }
            pending_separator = false;
            slug.push(ch);
        } else {
            pending_separator = true;
        }
    }
    if slug.is_empty() {
        "series".to_string()
    } else {
        slug
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Expand a leading `~` / `~/` without pulling in a crate for it.
pub fn expand_dir(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        return home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("~\\") {
        return home_dir().join(rest);
    }
    PathBuf::from(trimmed)
}

fn default_save_dir(name: &str) -> PathBuf {
    let cleaned: String = name
        .chars()
        .filter(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect();
    let folder = if cleaned.is_empty() {
        sanitize_slug(name)
    } else {
        cleaned
    };
    home_dir().join("Videos").join(folder)
}

async fn get_series_row_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Series>, AppError> {
    sqlx::query_as::<_, Series>(
        "SELECT id, key, name, save_dir, description, banner_path, banner_source, \
         created_at, updated_at, source_key, source_url, check_updates, update_count, \
         last_checked_at FROM series WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

async fn get_series_row_by_key(pool: &SqlitePool, key: &str) -> Result<Option<Series>, AppError> {
    sqlx::query_as::<_, Series>(
        "SELECT id, key, name, save_dir, description, banner_path, banner_source, \
         created_at, updated_at, source_key, source_url, check_updates, update_count, \
         last_checked_at FROM series WHERE key = ?1",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

async fn get_episode_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Episode>, AppError> {
    sqlx::query_as::<_, Episode>(&format!(
        "SELECT {EPISODE_COLUMNS} FROM episodes WHERE id = ?1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

async fn ensure_series(pool: &SqlitePool, series_id: i64) -> Result<(), AppError> {
    if get_series_row_by_id(pool, series_id).await?.is_none() {
        return Err(AppError::db("That series no longer exists."));
    }
    Ok(())
}

async fn next_available_key(pool: &SqlitePool, base: &str) -> Result<String, AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE key LIKE ?1 || '%'")
        .bind(base)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let mut suffix = count + 1;
    loop {
        let candidate = format!("{base}_{suffix}");
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE key = ?1")
            .bind(&candidate)
            .fetch_one(pool)
            .await
            .map_err(db_error)?;
        if exists == 0 {
            return Ok(candidate);
        }
        suffix += 1;
    }
}

// ---- series CRUD -------------------------------------------------------------

pub async fn create_series(
    pool: &SqlitePool,
    name: &str,
    save_dir: Option<&str>,
    description: &str,
) -> Result<Series, AppError> {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return Err(AppError::db("A series needs a name."));
    }
    let mut key = sanitize_slug(clean_name);

    if let Some(existing) = get_series_row_by_key(pool, &key).await? {
        if existing.name.eq_ignore_ascii_case(clean_name) {
            return Ok(existing);
        }
        key = next_available_key(pool, &key).await?;
    }

    let dir = match save_dir.map(str::trim).filter(|value| !value.is_empty()) {
        Some(raw) => expand_dir(raw),
        None => default_save_dir(clean_name),
    };
    fs::create_dir_all(&dir)
        .map_err(|err| AppError::io_with("Could not create the series folder.", err.to_string()))?;
    let save_dir_text = dir.to_string_lossy().to_string();
    let stamp = now_iso8601();

    let row = sqlx::query_as::<_, Series>(
        "INSERT INTO series (key, name, save_dir, description, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
         RETURNING id, key, name, save_dir, description, banner_path, banner_source, \
                  created_at, updated_at, source_key, source_url, check_updates, \
                  update_count, last_checked_at",
    )
    .bind(&key)
    .bind(clean_name)
    .bind(&save_dir_text)
    .bind(description.trim())
    .bind(&stamp)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(row)
}

pub async fn list_series(pool: &SqlitePool) -> Result<Vec<SeriesSummary>, AppError> {
    let sql = format!("{SUMMARY_SELECT} GROUP BY s.id ORDER BY s.name ASC");
    sqlx::query_as::<_, SeriesSummary>(&sql)
        .fetch_all(pool)
        .await
        .map_err(db_error)
}

/// Look up one series summary by its database id (used by detail commands).
pub async fn get_series_summary_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<SeriesSummary>, AppError> {
    let sql = format!("{SUMMARY_SELECT} WHERE s.id = ?1 GROUP BY s.id");
    sqlx::query_as::<_, SeriesSummary>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)
}

pub async fn get_series(pool: &SqlitePool, query: &str) -> Result<Option<SeriesSummary>, AppError> {
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(None);
    }

    if needle.chars().all(|ch| ch.is_ascii_digit())
        && let Ok(id) = needle.parse::<i64>()
    {
        let sql = format!("{SUMMARY_SELECT} WHERE s.id = ?1 GROUP BY s.id");
        if let Some(row) = sqlx::query_as::<_, SeriesSummary>(&sql)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?
        {
            return Ok(Some(row));
        }
    }

    let lowered = needle.to_lowercase();
    let sql = format!(
        "{SUMMARY_SELECT} WHERE s.key = ?1 OR LOWER(s.name) = ?2 \
         GROUP BY s.id ORDER BY s.name ASC LIMIT 1"
    );
    if let Some(row) = sqlx::query_as::<_, SeriesSummary>(&sql)
        .bind(&lowered)
        .bind(&lowered)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
    {
        return Ok(Some(row));
    }

    let pattern = format!("%{lowered}%");
    let sql = format!(
        "{SUMMARY_SELECT} WHERE LOWER(s.name) LIKE ?1 OR LOWER(s.key) LIKE ?1 \
         GROUP BY s.id ORDER BY s.name ASC LIMIT 1"
    );
    sqlx::query_as::<_, SeriesSummary>(&sql)
        .bind(&pattern)
        .fetch_optional(pool)
        .await
        .map_err(db_error)
}

pub async fn update_series(
    pool: &SqlitePool,
    id: i64,
    name: Option<&str>,
    save_dir: Option<&str>,
    description: Option<&str>,
) -> Result<bool, AppError> {
    let Some(current) = get_series_row_by_id(pool, id).await? else {
        return Ok(false);
    };
    let mut next_name = current.name;
    let mut next_dir = current.save_dir;
    let mut next_description = current.description;
    let mut touched = false;

    if let Some(value) = name.map(str::trim).filter(|value| !value.is_empty()) {
        next_name = value.to_string();
        touched = true;
    }
    if let Some(value) = save_dir.map(str::trim).filter(|value| !value.is_empty()) {
        let dir = expand_dir(value);
        fs::create_dir_all(&dir).map_err(|err| {
            AppError::io_with("Could not create the series folder.", err.to_string())
        })?;
        next_dir = dir.to_string_lossy().to_string();
        touched = true;
    }
    if let Some(value) = description {
        next_description = value.trim().to_string();
        touched = true;
    }
    if !touched {
        return Ok(false);
    }

    // Banners are owned by the dedicated setter: an edit to the series
    // details must never reset a legacy or custom banner.
    sqlx::query(
        "UPDATE series SET name = ?1, save_dir = ?2, description = ?3, \
         banner_path = ?4, banner_source = ?5, updated_at = ?6 WHERE id = ?7",
    )
    .bind(&next_name)
    .bind(&next_dir)
    .bind(&next_description)
    .bind(&current.banner_path)
    .bind(&current.banner_source)
    .bind(now_iso8601())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(true)
}

/// Set (or replace) a series banner. Used by the explicit user command.
pub async fn set_series_banner(
    pool: &SqlitePool,
    id: i64,
    path: &str,
    source: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE series SET banner_path = ?1, banner_source = ?2, updated_at = ?3 WHERE id = ?4",
    )
    .bind(path)
    .bind(source)
    .bind(now_iso8601())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

/// Remember where a series came from, so updates can be re-checked later.
pub async fn set_series_source(
    pool: &SqlitePool,
    id: i64,
    source_key: &str,
    source_url: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE series SET source_key = ?1, source_url = ?2, check_updates = 1, \
         updated_at = ?3 WHERE id = ?4",
    )
    .bind(source_key)
    .bind(source_url)
    .bind(now_iso8601())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

/// Opt a series in or out of the Home updates check.
pub async fn set_series_check_updates(
    pool: &SqlitePool,
    id: i64,
    enabled: bool,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE series SET check_updates = ?1, update_count = CASE WHEN ?1 = 0 THEN 0 \
         ELSE update_count END, updated_at = ?2 WHERE id = ?3",
    )
    .bind(if enabled { 1_i64 } else { 0_i64 })
    .bind(now_iso8601())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

/// Record the outcome of an update check for one series.
pub async fn record_update_check(
    pool: &SqlitePool,
    id: i64,
    new_episodes: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE series SET update_count = ?1, last_checked_at = ?2, updated_at = ?2 WHERE id = ?3",
    )
    .bind(new_episodes)
    .bind(now_iso8601())
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

/// A series that asked to be re-checked, with the page to resolve.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SeriesUpdateTarget {
    pub id: i64,
    pub name: String,
    pub source_key: String,
    pub source_url: String,
}

/// Every series the updates check should resolve (opted in and reachable).
pub async fn series_update_targets(pool: &SqlitePool) -> Result<Vec<SeriesUpdateTarget>, AppError> {
    sqlx::query_as::<_, SeriesUpdateTarget>(
        "SELECT id, name, source_key, source_url FROM series \
         WHERE check_updates = 1 AND source_url <> '' ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)
}

/// The episode page URLs already stored for a series, used for de-duplication.
pub async fn episode_urls(pool: &SqlitePool, series_id: i64) -> Result<Vec<String>, AppError> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT url FROM episodes WHERE series_id = ?1 AND url <> ''")
            .bind(series_id)
            .fetch_all(pool)
            .await
            .map_err(db_error)?;
    Ok(rows.into_iter().map(|(url,)| url).collect())
}

/// Every queued episode id in the order the user added them, with no cap.
/// Used to resume a download queue after a restart.
/// Series that still need a banner and have a remote page to fetch it from.
pub async fn series_without_banners(
    pool: &SqlitePool,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, source_key, source_url FROM series \
         WHERE (banner_path IS NULL OR banner_path = '') AND source_url <> '' \
         ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    Ok(rows)
}

pub async fn queued_episode_ids(pool: &SqlitePool) -> Result<Vec<i64>, AppError> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT id FROM episodes WHERE status = 'queued' ORDER BY id ASC")
            .fetch_all(pool)
            .await
            .map_err(db_error)?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Backfill a banner from the legacy cache. A custom choice always wins, and
/// an already-present banner is left alone so re-runs are idempotent.
pub async fn backfill_series_banner(
    pool: &SqlitePool,
    id: i64,
    path: &str,
    source: &str,
) -> Result<bool, AppError> {
    let Some(current) = get_series_row_by_id(pool, id).await? else {
        return Ok(false);
    };
    if current.banner_source == "custom" || current.banner_path.is_some() {
        return Ok(false);
    }
    set_series_banner(pool, id, path, source).await
}

pub async fn delete_series(pool: &SqlitePool, id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM series WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

// ---- episode CRUD ------------------------------------------------------------

pub async fn add_episode(
    pool: &SqlitePool,
    series_id: i64,
    url: &str,
    title: &str,
    episode_number: Option<i64>,
) -> Result<Option<Episode>, AppError> {
    let page_url = url.trim();
    if page_url.is_empty() {
        return Ok(None);
    }
    let video_id = extract_video_id(page_url);
    if video_id.is_empty() {
        return Ok(None);
    }
    let Some(series) = get_series_row_by_id(pool, series_id).await? else {
        return Err(AppError::db("That series no longer exists."));
    };

    let already_on_disk = archive_ids(&series.save_dir).contains(&video_id);
    let status = if already_on_disk {
        "downloaded"
    } else {
        "not_started"
    };
    let stamp = now_iso8601();
    let downloaded_at = already_on_disk.then(|| stamp.clone());

    let row = sqlx::query_as::<_, Episode>(&format!(
        "INSERT INTO episodes \
             (series_id, video_id, url, episode_number, title, status, downloaded_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(series_id, video_id) DO UPDATE SET \
             title = CASE WHEN excluded.title <> '' THEN excluded.title ELSE episodes.title END, \
             episode_number = COALESCE(excluded.episode_number, episodes.episode_number) \
         RETURNING {EPISODE_COLUMNS}"
    ))
    .bind(series_id)
    .bind(&video_id)
    .bind(page_url)
    .bind(episode_number)
    .bind(title.trim())
    .bind(status)
    .bind(downloaded_at)
    .bind(&stamp)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(Some(row))
}

pub async fn add_episodes_bulk(
    pool: &SqlitePool,
    series_id: i64,
    urls: &[String],
) -> Result<BulkAddResult, AppError> {
    ensure_series(pool, series_id).await?;
    let stamp = now_iso8601();
    let total = urls.len() as i64;
    let mut added = 0i64;
    let mut tx = pool.begin().await.map_err(db_error)?;
    for raw in urls {
        let page_url = raw.trim();
        if page_url.is_empty() || page_url.starts_with('#') {
            continue;
        }
        let video_id = extract_video_id(page_url);
        if video_id.is_empty() {
            continue;
        }
        let result = sqlx::query(
            "INSERT INTO episodes (series_id, video_id, url, title, status, created_at) \
             VALUES (?1, ?2, ?3, '', 'not_started', ?4) \
             ON CONFLICT(series_id, video_id) DO NOTHING",
        )
        .bind(series_id)
        .bind(&video_id)
        .bind(page_url)
        .bind(&stamp)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        added += result.rows_affected() as i64;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(BulkAddResult {
        added,
        skipped: total.saturating_sub(added),
        total,
    })
}

pub async fn add_episode_entries(
    pool: &SqlitePool,
    series_id: i64,
    entries: &[EpisodeEntry],
) -> Result<EntriesAddResult, AppError> {
    ensure_series(pool, series_id).await?;
    let existing: Vec<String> =
        sqlx::query_scalar("SELECT video_id FROM episodes WHERE series_id = ?1")
            .bind(series_id)
            .fetch_all(pool)
            .await
            .map_err(db_error)?;
    let mut seen: HashSet<String> = existing.into_iter().collect();

    let stamp = now_iso8601();
    let mut added = 0i64;
    let mut updated = 0i64;
    let mut total = 0i64;
    let mut tx = pool.begin().await.map_err(db_error)?;

    for entry in entries {
        let page_url = entry.url.trim();
        if page_url.is_empty() || page_url.starts_with('#') {
            continue;
        }
        let video_id = extract_video_id(page_url);
        if video_id.is_empty() {
            continue;
        }
        total += 1;
        let number = entry.episode_number.filter(|value| *value > 0);
        let title = entry.title.trim();
        let downloaded = entry.status.as_deref() == Some("downloaded");
        let status = if downloaded {
            "downloaded"
        } else {
            "not_started"
        };
        let downloaded_at = downloaded.then(|| stamp.clone());

        sqlx::query(
            "INSERT INTO episodes \
                 (series_id, video_id, url, episode_number, title, status, downloaded_at, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(series_id, video_id) DO UPDATE SET \
                 url = excluded.url, \
                 title = CASE WHEN excluded.title <> '' THEN excluded.title ELSE episodes.title END, \
                 episode_number = COALESCE(excluded.episode_number, episodes.episode_number), \
                 status = CASE WHEN excluded.status = 'downloaded' \
                     THEN 'downloaded' ELSE episodes.status END, \
                 downloaded_at = CASE WHEN excluded.status = 'downloaded' \
                     THEN COALESCE(episodes.downloaded_at, excluded.downloaded_at) \
                     ELSE episodes.downloaded_at END",
        )
        .bind(series_id)
        .bind(&video_id)
        .bind(page_url)
        .bind(number)
        .bind(title)
        .bind(status)
        .bind(downloaded_at)
        .bind(&stamp)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        if seen.insert(video_id) {
            added += 1;
        } else {
            updated += 1;
        }
    }
    tx.commit().await.map_err(db_error)?;
    Ok(EntriesAddResult {
        added,
        updated,
        total,
    })
}

pub async fn list_episodes(
    pool: &SqlitePool,
    series_id: i64,
    status: Option<&str>,
) -> Result<Vec<Episode>, AppError> {
    match status.map(str::trim).filter(|value| !value.is_empty()) {
        Some(status) => {
            let sql = format!(
                "SELECT {EPISODE_COLUMNS} FROM episodes \
                 WHERE series_id = ?1 AND status = ?2 ORDER BY id ASC"
            );
            sqlx::query_as::<_, Episode>(&sql)
                .bind(series_id)
                .bind(status)
                .fetch_all(pool)
                .await
                .map_err(db_error)
        }
        None => {
            let sql = format!(
                "SELECT {EPISODE_COLUMNS} FROM episodes WHERE series_id = ?1 ORDER BY id ASC"
            );
            sqlx::query_as::<_, Episode>(&sql)
                .bind(series_id)
                .fetch_all(pool)
                .await
                .map_err(db_error)
        }
    }
}

/// Insert a new episode for a series, returning the inserted row.
pub async fn create_episode(
    pool: &SqlitePool,
    series_id: i64,
    video_id: &str,
    url: &str,
    episode_number: Option<i64>,
    title: &str,
    status: &str,
) -> Result<Episode, AppError> {
    ensure_series(pool, series_id).await?;
    let stamp = now_iso8601();
    let row = sqlx::query_as::<_, Episode>(&format!(
        "INSERT INTO episodes \
             (series_id, video_id, url, episode_number, title, status, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         RETURNING {EPISODE_COLUMNS}"
    ))
    .bind(series_id)
    .bind(video_id.trim())
    .bind(url.trim())
    .bind(episode_number)
    .bind(title.trim())
    .bind(status)
    .bind(&stamp)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(row)
}

/// Update an episode's editable fields, returning the updated row.
/// Only the provided fields are changed; a `url` change recomputes `video_id`.
#[allow(clippy::too_many_arguments)]
pub async fn update_episode(
    pool: &SqlitePool,
    id: i64,
    title: Option<&str>,
    status: Option<&str>,
    video_id: Option<&str>,
    url: Option<&str>,
    episode_number: Option<i64>,
) -> Result<Episode, AppError> {
    let Some(current) = get_episode_by_id(pool, id).await? else {
        return Err(AppError::db("That episode no longer exists."));
    };
    let mut next_title = current.title;
    let mut next_status = current.status;
    let mut next_video_id = current.video_id;
    let mut next_url = current.url;
    let mut next_number = current.episode_number;
    let mut touched = false;

    if let Some(value) = title.map(str::trim).filter(|value| !value.is_empty()) {
        next_title = value.to_string();
        touched = true;
    }
    if let Some(value) = status {
        next_status = value.to_string();
        touched = true;
    }
    if let Some(value) = video_id.map(str::trim).filter(|value| !value.is_empty()) {
        next_video_id = value.to_string();
        touched = true;
    }
    if let Some(value) = url.map(str::trim).filter(|value| !value.is_empty()) {
        next_url = value.to_string();
        touched = true;
    }
    if let Some(value) = episode_number.filter(|value| *value > 0) {
        next_number = Some(value);
        touched = true;
    }
    if !touched {
        return Err(AppError::db("No fields to update."));
    }

    // When the URL changed but an explicit video_id was not supplied, recompute
    // the stable media id from the new page URL.
    let final_video_id = if url.is_some() && video_id.is_none() {
        extract_video_id(&next_url)
    } else {
        next_video_id
    };

    let row = sqlx::query_as::<_, Episode>(&format!(
        "UPDATE episodes SET title = ?1, status = ?2, video_id = ?3, \
         url = ?4, episode_number = ?5 WHERE id = ?6 \
         RETURNING {EPISODE_COLUMNS}"
    ))
    .bind(&next_title)
    .bind(&next_status)
    .bind(&final_video_id)
    .bind(&next_url)
    .bind(next_number)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(row)
}

/// Delete an episode and return the deleted row.
pub async fn delete_episode(pool: &SqlitePool, id: i64) -> Result<Episode, AppError> {
    let Some(current) = get_episode_by_id(pool, id).await? else {
        return Err(AppError::db("That episode no longer exists."));
    };
    let result = sqlx::query("DELETE FROM episodes WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err(AppError::db("That episode no longer exists."));
    }
    Ok(current)
}
/// Episodes from across the library, optionally filtered by status.
///
/// Used by Home and Downloads; the limit is clamped so a UI request can never
/// pull the entire catalog into the webview.
pub async fn episodes_with_series(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: Option<i64>,
) -> Result<Vec<EpisodeWithSeries>, AppError> {
    let cap = limit.unwrap_or(50).clamp(1, 200);
    let mut builder = QueryBuilder::<Sqlite>::new(format!(
        "SELECT {CROSS_LIBRARY_EPISODE_COLUMNS} FROM episodes e \
         JOIN series s ON s.id = e.series_id"
    ));

    if let Some(status) = status.map(str::trim).filter(|value| !value.is_empty()) {
        builder.push(" WHERE e.status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY COALESCE(e.downloaded_at, e.created_at) DESC, e.id DESC LIMIT ");
    builder.push_bind(cap);

    builder
        .build_query_as::<EpisodeWithSeries>()
        .fetch_all(pool)
        .await
        .map_err(db_error)
}

pub async fn episodes_by_video_ids(
    pool: &SqlitePool,
    video_ids: &[String],
) -> Result<Vec<Episode>, AppError> {
    let cleaned: Vec<&str> = video_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    for chunk in cleaned.chunks(400) {
        let mut builder = QueryBuilder::<Sqlite>::new(format!(
            "SELECT {EPISODE_COLUMNS} FROM episodes WHERE video_id IN ("
        ));
        let mut separated = builder.separated(", ");
        for video_id in chunk {
            separated.push_bind(*video_id);
        }
        separated.push_unseparated(") ORDER BY series_id ASC, id ASC");
        let batch = builder
            .build_query_as::<Episode>()
            .fetch_all(pool)
            .await
            .map_err(db_error)?;
        rows.extend(batch);
    }
    Ok(rows)
}

pub async fn mark_episode_status(
    pool: &SqlitePool,
    series_id: i64,
    video_id: &str,
    status: &str,
) -> Result<bool, AppError> {
    let result = if status == "downloaded" {
        sqlx::query(
            "UPDATE episodes SET status = ?1, downloaded_at = COALESCE(downloaded_at, ?2) \
             WHERE series_id = ?3 AND video_id = ?4",
        )
        .bind(status)
        .bind(now_iso8601())
        .bind(series_id)
        .bind(video_id)
        .execute(pool)
        .await
        .map_err(db_error)?
    } else {
        sqlx::query("UPDATE episodes SET status = ?1 WHERE series_id = ?2 AND video_id = ?3")
            .bind(status)
            .bind(series_id)
            .bind(video_id)
            .execute(pool)
            .await
            .map_err(db_error)?
    };
    Ok(result.rows_affected() > 0)
}

/// Set one episode's status by row id (used by the download pipeline).
pub async fn set_episode_status_by_id(
    pool: &SqlitePool,
    id: i64,
    status: &str,
) -> Result<bool, AppError> {
    let result = if status == "downloaded" {
        sqlx::query(
            "UPDATE episodes SET status = ?1, downloaded_at = COALESCE(downloaded_at, ?2) \
             WHERE id = ?3",
        )
        .bind(status)
        .bind(now_iso8601())
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?
    } else {
        sqlx::query("UPDATE episodes SET status = ?1 WHERE id = ?2")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await
            .map_err(db_error)?
    };
    Ok(result.rows_affected() > 0)
}

/// Persist the finished media path so /watch survives restarts and queue clears.
pub async fn set_episode_file_path(
    pool: &SqlitePool,
    id: i64,
    path: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query("UPDATE episodes SET file_path = ?1 WHERE id = ?2")
        .bind(path)
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

/// Resolve a downloaded episode's media file for playback.
///
/// The recorded path is preferred; for episodes downloaded before `file_path`
/// existed we fall back to matching a media file in the series folder by its
/// stable video id.
pub async fn downloaded_episode_path(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<String>, AppError> {
    let row: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT e.file_path, e.video_id, s.save_dir FROM episodes e \
         JOIN series s ON s.id = e.series_id \
         WHERE e.id = ?1 AND e.status = 'downloaded'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;

    let Some((recorded, video_id, save_dir)) = row else {
        return Ok(None);
    };
    if let Some(path) = recorded.filter(|value| !value.trim().is_empty())
        && std::fs::metadata(&path)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
    {
        return Ok(Some(path));
    }

    const MEDIA_EXTENSIONS: [&str; 5] = [".mp4", ".mkv", ".webm", ".mov", ".m4v"];
    let video_needle = video_id.to_lowercase();
    let entries = match std::fs::read_dir(expand_dir(&save_dir)) {
        Ok(entries) => entries,
        Err(_) => return Ok(None),
    };
    let found = entries.filter_map(Result::ok).find_map(|entry| {
        let path = entry.path();
        if !path.is_file() {
            return None;
        }
        let name = path.file_name()?.to_string_lossy().to_lowercase();
        if name.contains(&video_needle)
            && MEDIA_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
            && std::fs::metadata(&path)
                .map(|meta| meta.len() > 0)
                .unwrap_or(false)
        {
            return Some(path.to_string_lossy().to_string());
        }
        None
    });
    Ok(found)
}

/// Episode plus its parent series, used to build a download job.
pub async fn episode_job_context(
    pool: &SqlitePool,
    episode_id: i64,
) -> Result<Option<(Episode, Series)>, AppError> {
    let Some(episode) = get_episode_by_id(pool, episode_id).await? else {
        return Ok(None);
    };
    let Some(series) = get_series_row_by_id(pool, episode.series_id).await? else {
        return Ok(None);
    };
    Ok(Some((episode, series)))
}

pub async fn update_episode_fields(
    pool: &SqlitePool,
    id: i64,
    title: Option<&str>,
    episode_number: Option<i64>,
    url: Option<&str>,
) -> Result<bool, AppError> {
    let Some(current) = get_episode_by_id(pool, id).await? else {
        return Ok(false);
    };
    let mut next_title = current.title;
    let mut next_number = current.episode_number;
    let mut next_url = current.url;
    let mut next_video_id = current.video_id;
    let mut touched = false;

    if let Some(value) = title {
        next_title = value.trim().to_string();
        touched = true;
    }
    if let Some(value) = episode_number.filter(|value| *value > 0) {
        next_number = Some(value);
        touched = true;
    }
    if let Some(value) = url.map(str::trim).filter(|value| !value.is_empty()) {
        next_url = value.to_string();
        next_video_id = extract_video_id(value);
        touched = true;
    }
    if !touched {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE episodes SET title = ?1, episode_number = ?2, url = ?3, video_id = ?4 \
         WHERE id = ?5",
    )
    .bind(&next_title)
    .bind(next_number)
    .bind(&next_url)
    .bind(&next_video_id)
    .bind(id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(true)
}

// ---- aggregates --------------------------------------------------------------

pub async fn series_poster_ids(pool: &SqlitePool) -> Result<Vec<SeriesPoster>, AppError> {
    sqlx::query_as::<_, SeriesPoster>(
        "SELECT e.series_id, e.video_id FROM episodes e \
         WHERE e.id IN (SELECT MIN(id) FROM episodes GROUP BY series_id) \
         ORDER BY e.series_id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)
}

pub async fn library_stats(pool: &SqlitePool) -> Result<LibraryStats, AppError> {
    sqlx::query_as::<_, LibraryStats>(
        "SELECT \
             (SELECT COUNT(*) FROM series) AS series_count, \
             (SELECT COUNT(*) FROM episodes) AS episode_count, \
             (SELECT COUNT(*) FROM episodes WHERE status = 'downloaded') AS downloaded_count, \
             (SELECT COUNT(*) FROM episodes WHERE status = 'not_started') AS pending_count, \
             (SELECT COUNT(*) FROM episodes WHERE status = 'queued') AS queued_count, \
             (SELECT COUNT(*) FROM episodes WHERE status = 'failed') AS failed_count",
    )
    .fetch_one(pool)
    .await
    .map_err(db_error)
}

// ---- archive + disk reconciliation ------------------------------------------

/// IDs recorded in `<save_dir>/downloaded.txt` (last token of each line).
pub fn archive_ids(save_dir: &str) -> HashSet<String> {
    let path = expand_dir(save_dir).join("downloaded.txt");
    let Ok(contents) = fs::read_to_string(path) else {
        return HashSet::new();
    };
    contents
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

/// Finished media files in a series folder.
pub fn media_files(save_dir: &str) -> Vec<PathBuf> {
    let dir = expand_dir(save_dir);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let lowered = name.to_lowercase();
        if !MEDIA_EXTENSIONS
            .iter()
            .any(|extension| lowered.ends_with(extension))
        {
            continue;
        }
        if is_intermediate_media_name(name) {
            continue;
        }
        let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        if size < MIN_MEDIA_BYTES {
            continue;
        }
        files.push(path);
    }
    files
}

pub fn media_stems(save_dir: &str) -> HashSet<String> {
    media_files(save_dir)
        .iter()
        .filter_map(|path| path.file_name().and_then(|value| value.to_str()))
        .map(|name| normalize_stem(media_stem(name)))
        .collect()
}

/// video_id → normalized title stem, so a row can be matched to its file.
pub fn episode_file_stems(episodes: &[Episode]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for episode in episodes {
        if episode.video_id.trim().is_empty() || episode.title.trim().is_empty() {
            continue;
        }
        let stem = normalize_stem(&safe_title(&episode.title, 150));
        map.insert(episode.video_id.clone(), stem);
    }
    map
}

fn is_valid_video_id(video_id: &str) -> bool {
    let trimmed = video_id.trim();
    !trimmed.is_empty() && !trimmed.contains("://") && !trimmed.contains(char::is_whitespace)
}

/// Make sure each existing series folder has a `downloaded.txt` marker.
pub async fn repair_archives(pool: &SqlitePool) -> Result<ArchiveRepairResult, AppError> {
    let dirs: Vec<String> = sqlx::query_scalar("SELECT save_dir FROM series ORDER BY id")
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut folders_scanned = 0i64;
    let mut archives_created = 0i64;
    for raw in dirs {
        let dir = expand_dir(&raw);
        if !dir.is_dir() {
            continue;
        }
        folders_scanned += 1;
        let archive = dir.join("downloaded.txt");
        if !archive.exists() && fs::write(&archive, b"").is_ok() {
            archives_created += 1;
        }
    }
    Ok(ArchiveRepairResult {
        folders_scanned,
        archives_created,
    })
}

/// Reconcile one series' episode statuses with the archive file and the disk.
pub async fn sync_series_with_archive(pool: &SqlitePool, series_id: i64) -> Result<i64, AppError> {
    let Some(series) = get_series_row_by_id(pool, series_id).await? else {
        return Ok(0);
    };
    let dir = expand_dir(&series.save_dir);
    if !dir.is_dir() {
        return Ok(0);
    }
    let episodes = list_episodes(pool, series_id, None).await?;
    if episodes.is_empty() {
        return Ok(0);
    }

    let archive = archive_ids(&series.save_dir);
    let on_disk = media_stems(&series.save_dir);
    let file_stems = episode_file_stems(&episodes);

    let mut upgrades = Vec::new();
    let mut downgrades = Vec::new();
    for episode in &episodes {
        if !is_valid_video_id(&episode.video_id) {
            continue;
        }
        let has_file = file_stems
            .get(&episode.video_id)
            .is_some_and(|stem| on_disk.contains(stem));
        let in_archive = archive.contains(&episode.video_id);
        if (has_file || in_archive) && episode.status != "downloaded" {
            upgrades.push(episode);
        } else if !has_file && !in_archive && episode.status == "downloaded" {
            downgrades.push(episode);
        }
    }
    if upgrades.is_empty() && downgrades.is_empty() {
        return Ok(0);
    }

    let stamp = now_iso8601();
    let mut changed = 0i64;
    let mut tx = pool.begin().await.map_err(db_error)?;
    for episode in upgrades {
        let result = sqlx::query(
            "UPDATE episodes SET status = 'downloaded', \
                 downloaded_at = COALESCE(downloaded_at, ?1) WHERE id = ?2",
        )
        .bind(&stamp)
        .bind(episode.id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        changed += result.rows_affected() as i64;
    }
    for episode in downgrades {
        let result = sqlx::query(
            "UPDATE episodes SET status = 'not_started', downloaded_at = NULL WHERE id = ?1",
        )
        .bind(episode.id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        changed += result.rows_affected() as i64;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(changed)
}

pub async fn sync_all_archives(pool: &SqlitePool) -> Result<i64, AppError> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM series ORDER BY id")
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut total = 0i64;
    for id in ids {
        match sync_series_with_archive(pool, id).await {
            Ok(changed) => total += changed,
            Err(err) => log::warn!("could not sync series {id} with its archive: {err}"),
        }
    }
    Ok(total)
}

// ---- legacy import -----------------------------------------------------------
// Used by the one-shot migration from the legacy `donghua-stream` catalog.
// These helpers preserve the legacy `key` and `video_id` values so existing
// archive files (`downloaded.txt`) and on-disk media keep matching.

fn normalize_status(status: &str) -> &'static str {
    match status.trim().to_lowercase().as_str() {
        "downloaded" => "downloaded",
        "queued" => "queued",
        "failed" => "failed",
        "pending" | "not_started" | "ready" => "not_started",
        _ => "not_started",
    }
}

/// Create a series while preserving a legacy key. Returns the existing row
/// when the key is already present.
pub async fn create_series_with_key(
    pool: &SqlitePool,
    key: &str,
    name: &str,
    save_dir: &str,
    description: &str,
) -> Result<Series, AppError> {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return Err(AppError::db("A series needs a name."));
    }
    let clean_key = key.trim();
    if clean_key.is_empty() {
        return create_series(pool, clean_name, Some(save_dir), description).await;
    }
    if let Some(existing) = get_series_row_by_key(pool, clean_key).await? {
        return Ok(existing);
    }
    let dir = expand_dir(save_dir);
    fs::create_dir_all(&dir)
        .map_err(|err| AppError::io_with("Could not create the series folder.", err.to_string()))?;
    let save_dir_text = dir.to_string_lossy().to_string();
    let stamp = now_iso8601();
    let row = sqlx::query_as::<_, Series>(
        "INSERT INTO series (key, name, save_dir, description, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
         RETURNING id, key, name, save_dir, description, banner_path, banner_source, \
                  created_at, updated_at, source_key, source_url, check_updates, \
                  update_count, last_checked_at",
    )
    .bind(clean_key)
    .bind(clean_name)
    .bind(&save_dir_text)
    .bind(description.trim())
    .bind(&stamp)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(row)
}

/// Look up a series by its exact key.
pub async fn find_series_by_key(pool: &SqlitePool, key: &str) -> Result<Option<Series>, AppError> {
    get_series_row_by_key(pool, key).await
}

/// Insert or update an episode using the exact legacy identifiers.
/// Returns `true` when a new row was inserted.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_imported_episode(
    pool: &SqlitePool,
    series_id: i64,
    video_id: &str,
    url: &str,
    episode_number: Option<i64>,
    title: &str,
    status: &str,
    downloaded_at: Option<&str>,
    created_at: &str,
) -> Result<bool, AppError> {
    let video_id = video_id.trim();
    let url = url.trim();
    if video_id.is_empty() || url.is_empty() {
        return Ok(false);
    }
    let normalized = normalize_status(status);
    let requested = downloaded_at.unwrap_or("").trim();
    let downloaded_value = if normalized == "downloaded" {
        Some(if requested.is_empty() {
            created_at.to_string()
        } else {
            requested.to_string()
        })
    } else {
        None
    };

    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM episodes WHERE series_id = ?1 AND video_id = ?2")
            .bind(series_id)
            .bind(video_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;

    if let Some(id) = existing {
        sqlx::query(
            "UPDATE episodes \
             SET url = ?1, episode_number = ?2, title = ?3, status = ?4, downloaded_at = ?5 \
             WHERE id = ?6",
        )
        .bind(url)
        .bind(episode_number)
        .bind(title)
        .bind(normalized)
        .bind(&downloaded_value)
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO episodes \
         (series_id, video_id, url, episode_number, title, status, downloaded_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(series_id)
    .bind(video_id)
    .bind(url)
    .bind(episode_number)
    .bind(title)
    .bind(normalized)
    .bind(&downloaded_value)
    .bind(created_at)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(true)
}

// ---- watch progress -----------------------------------------------------

/// Persist the playback position for an episode. Written on pause, seek and
/// near the end so a later session can resume.
pub async fn save_watch_progress(
    pool: &SqlitePool,
    episode_id: i64,
    position_sec: f64,
    duration_sec: f64,
    completed: bool,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "INSERT INTO watch_progress (episode_id, position_sec, duration_sec, updated_at, completed) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(episode_id) DO UPDATE SET \
             position_sec = excluded.position_sec, \
             duration_sec = excluded.duration_sec, \
             updated_at = excluded.updated_at, \
             completed = excluded.completed",
    )
    .bind(episode_id)
    .bind(position_sec.max(0.0))
    .bind(duration_sec.max(0.0))
    .bind(now_iso8601())
    .bind(if completed { 1_i64 } else { 0_i64 })
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

/// Read the saved playback position for an episode, if any.
pub async fn get_watch_progress(
    pool: &SqlitePool,
    episode_id: i64,
) -> Result<Option<crate::db::models::WatchProgress>, AppError> {
    sqlx::query_as::<_, crate::db::models::WatchProgress>(
        "SELECT episode_id, position_sec, duration_sec, updated_at, completed \
         FROM watch_progress WHERE episode_id = ?1",
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)
}

/// Forget the saved position (e.g. after the episode is fully watched).
pub async fn clear_watch_progress(pool: &SqlitePool, episode_id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM watch_progress WHERE episode_id = ?1")
        .bind(episode_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(result.rows_affected() > 0)
}

/// All saved positions, newest first — used for a "continue watching" list.
pub async fn all_watch_progress(
    pool: &SqlitePool,
) -> Result<Vec<crate::db::models::WatchProgress>, AppError> {
    sqlx::query_as::<_, crate::db::models::WatchProgress>(
        "SELECT episode_id, position_sec, duration_sec, updated_at, completed \
         FROM watch_progress ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)
}
