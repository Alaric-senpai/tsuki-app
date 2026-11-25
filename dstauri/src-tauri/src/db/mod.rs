//! SQLite library database (replaces the legacy PostgreSQL catalog).
//!
//! The pool is created once at startup and shared through [`crate::state::AppState`].
//! Migrations under `src-tauri/migrations` are applied automatically.

use std::fs;
use std::path::Path;
use std::time::Duration;

use sqlx::ConnectOptions;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::error::AppError;

pub mod models;
pub mod queries;

/// Handle around the SQLite pool.
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Open (or create) the database file and apply pending migrations.
    pub async fn connect(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                AppError::io_with(
                    "Could not create the application data directory.",
                    err.to_string(),
                )
            })?;
            restrict_dir(parent);
        }

        // Statement logging is deliberately off: sqlx otherwise emits one
        // debug line per query, which floods the log during startup and sync.
        // Only warnings from the driver itself are interesting.
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .log_statements(log::LevelFilter::Off)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|err| {
                AppError::db_with("Could not open the library database.", err.to_string())
            })?;

        let applied_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|err| {
                AppError::db_with(
                    "Could not apply the library database migrations.",
                    err.to_string(),
                )
            })?;

        let applied_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap_or(applied_before);

        if applied_after > applied_before {
            log::info!(
                "applied {} migration(s), {} total",
                applied_after - applied_before,
                applied_after
            );
        }

        restrict_file(path);
        log::info!("library database ready at {}", path.display());
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(unix)]
fn restrict_dir(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn restrict_dir(_path: &Path) {}

#[cfg(unix)]
fn restrict_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_file(_path: &Path) {}
