//! Row/response types for the library database.
//!
//! Everything is serialized as camelCase for the TypeScript frontend.

#![allow(dead_code)] // entry/result types are consumed by later phases

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Series {
    #[ts(as = "f64")]
    pub id: i64,
    pub key: String,
    pub name: String,
    pub save_dir: String,
    pub description: String,
    pub banner_path: Option<String>,
    pub banner_source: String,
    pub created_at: String,
    pub updated_at: String,
    /// Source the series was imported from (`donghuastream`, …); empty if manual.
    pub source_key: String,
    /// Canonical remote page, used when re-checking for new episodes.
    pub source_url: String,
    /// Whether the Home updates check should re-resolve this series.
    #[ts(as = "f64")]
    pub check_updates: i64,
    /// Episodes that were missing remotely at the last check.
    #[ts(as = "f64")]
    pub update_count: i64,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SeriesSummary {
    #[ts(as = "f64")]
    pub id: i64,
    pub key: String,
    pub name: String,
    pub save_dir: String,
    pub description: String,
    pub banner_path: Option<String>,
    pub banner_source: String,
    pub created_at: String,
    pub updated_at: String,
    #[ts(as = "f64")]
    pub total_episodes: i64,
    #[ts(as = "f64")]
    pub downloaded_episodes: i64,
    #[ts(as = "f64")]
    pub pending_episodes: i64,
    #[ts(as = "f64")]
    pub failed_episodes: i64,
    #[ts(as = "f64")]
    pub queued_episodes: i64,
    pub source_key: String,
    pub source_url: String,
    #[ts(as = "f64")]
    pub check_updates: i64,
    #[ts(as = "f64")]
    pub update_count: i64,
    pub last_checked_at: Option<String>,
}

/// IPC shape for the library surfaces: a summary with the banner inlined as
/// a data URL so the webview never needs filesystem access.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SeriesSummaryView {
    #[ts(as = "f64")]
    pub id: i64,
    pub key: String,
    pub name: String,
    pub save_dir: String,
    pub description: String,
    pub banner_url: Option<String>,
    pub banner_source: String,
    pub created_at: String,
    pub updated_at: String,
    #[ts(as = "f64")]
    pub total_episodes: i64,
    #[ts(as = "f64")]
    pub downloaded_episodes: i64,
    #[ts(as = "f64")]
    pub pending_episodes: i64,
    #[ts(as = "f64")]
    pub failed_episodes: i64,
    #[ts(as = "f64")]
    pub queued_episodes: i64,
    pub source_key: String,
    pub source_url: String,
    pub check_updates: bool,
    #[ts(as = "f64")]
    pub update_count: i64,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    #[ts(as = "f64")]
    pub id: i64,
    #[ts(as = "f64")]
    pub series_id: i64,
    pub video_id: String,
    pub url: String,
    #[ts(as = "Option<f64>")]
    pub episode_number: Option<i64>,
    pub title: String,
    pub status: String,
    pub downloaded_at: Option<String>,
    pub created_at: String,
}

/// Episode row joined with its parent series name for cross-library surfaces.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct EpisodeWithSeries {
    #[ts(as = "f64")]
    pub id: i64,
    #[ts(as = "f64")]
    pub series_id: i64,
    pub series_name: String,
    pub video_id: String,
    pub url: String,
    #[ts(as = "Option<f64>")]
    pub episode_number: Option<i64>,
    pub title: String,
    pub status: String,
    pub downloaded_at: Option<String>,
    pub created_at: String,
}

/// One entry for the bulk episode import used by the add wizard.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct EpisodeEntry {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    #[ts(as = "Option<f64>")]
    pub episode_number: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct WatchProgress {
    #[ts(as = "f64")]
    pub episode_id: i64,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub updated_at: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct SeriesPoster {
    #[ts(as = "f64")]
    pub series_id: i64,
    pub video_id: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    #[ts(as = "f64")]
    pub series_count: i64,
    #[ts(as = "f64")]
    pub episode_count: i64,
    #[ts(as = "f64")]
    pub downloaded_count: i64,
    #[ts(as = "f64")]
    pub pending_count: i64,
    #[ts(as = "f64")]
    pub queued_count: i64,
    #[ts(as = "f64")]
    pub failed_count: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct BulkAddResult {
    #[ts(as = "f64")]
    pub added: i64,
    #[ts(as = "f64")]
    pub skipped: i64,
    #[ts(as = "f64")]
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct EntriesAddResult {
    #[ts(as = "f64")]
    pub added: i64,
    #[ts(as = "f64")]
    pub updated: i64,
    #[ts(as = "f64")]
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRepairResult {
    #[ts(as = "f64")]
    pub folders_scanned: i64,
    #[ts(as = "f64")]
    pub archives_created: i64,
}
