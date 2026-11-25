//! Disk usage of the per-series download folders.
//!
//! Pure filesystem walk, run on a tokio task by the command handler. Kept out
//! of the database layer because no SQL is involved.

use std::path::Path;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::db::queries;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesUsage {
    pub series_id: i64,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    pub series: Vec<SeriesUsage>,
    pub total_bytes: u64,
    pub total_files: u64,
}

/// Sum the size of every series folder, largest first.
///
/// Missing or unreadable folders count as empty: a deleted download folder is
/// not an error the user needs to see just to open the Downloads page.
pub async fn scan_disk_usage(pool: &SqlitePool) -> Result<DiskUsage, AppError> {
    let series = queries::list_series(pool).await?;
    let mut rows = Vec::with_capacity(series.len());
    let mut total_bytes = 0u64;
    let mut total_files = 0u64;
    for item in series {
        let dir = queries::expand_dir(&item.save_dir);
        let (bytes, files) = walk(&dir, 0);
        total_bytes += bytes;
        total_files += files;
        rows.push(SeriesUsage {
            series_id: item.id,
            name: item.name,
            path: dir.to_string_lossy().to_string(),
            bytes,
            files,
        });
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.bytes));
    Ok(DiskUsage {
        series: rows,
        total_bytes,
        total_files,
    })
}

/// Depth-limited walk that never follows symlinks (a link into a parent folder
/// would otherwise loop forever and inflate the totals).
fn walk(path: &Path, depth: usize) -> (u64, u64) {
    const MAX_DEPTH: usize = 12;
    if depth > MAX_DEPTH {
        return (0, 0);
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return (0, 0);
    };
    let mut bytes = 0u64;
    let mut files = 0u64;
    for entry in entries.flatten() {
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            let (nested_bytes, nested_files) = walk(&entry.path(), depth + 1);
            bytes += nested_bytes;
            files += nested_files;
        } else if metadata.is_file() {
            files += 1;
            bytes += metadata.len();
        }
    }
    (bytes, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_counts_nested_files_and_skips_symlinks() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(root.path().join("season/eps")).expect("nested dirs");
        std::fs::write(root.path().join("poster.jpg"), b"12345").expect("poster");
        std::fs::write(root.path().join("season/eps/ep.mkv"), b"1234567890").expect("episode");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc", root.path().join("etc-link")).expect("symlink");

        let (bytes, files) = walk(root.path(), 0);
        assert_eq!(files, 2);
        assert_eq!(bytes, 15);
    }

    #[test]
    fn missing_folder_is_empty() {
        let root = tempfile::tempdir().expect("temp dir");
        let missing = root.path().join("does-not-exist");
        assert_eq!(walk(&missing, 0), (0, 0));
    }
}
