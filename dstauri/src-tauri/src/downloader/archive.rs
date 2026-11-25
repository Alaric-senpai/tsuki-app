//! `downloaded.txt` archive handling.
//!
//! Ported from the archive helpers in the legacy `downloader.py`. The archive
//! lives next to the media in each series folder and records
//! `<tag> <video_id>` lines. It must survive folder renames and re-syncs, so
//! reads are tolerant and writes are idempotent.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::db::queries;
use crate::error::AppError;
use crate::media_ids;

pub const ARCHIVE_FILE: &str = "downloaded.txt";

/// Create `<save_dir>/downloaded.txt` before the first save.
pub fn ensure_archive_file(save_dir: &str) -> Result<PathBuf, AppError> {
    let dir = queries::expand_dir(save_dir);
    fs::create_dir_all(&dir).map_err(|err| {
        AppError::io_with(
            "Could not create the series folder.",
            format!("{}: {err}", dir.display()),
        )
    })?;
    let path = dir.join(ARCHIVE_FILE);
    if !path.exists() {
        fs::write(&path, b"").map_err(|err| {
            AppError::io_with(
                "Could not create the download archive.",
                format!("{}: {err}", path.display()),
            )
        })?;
    }
    Ok(path)
}

/// Append a video ID to the archive when it is not recorded yet.
///
/// Returns `true` when a line was written. The legacy engine refuses to record
/// anything that is not a bare identifier (a raw URL means ID extraction
/// failed, and recording it would poison every later sync).
pub fn add_archive_id(save_dir: &str, video_id: &str) -> Result<bool, AppError> {
    let video_id = video_id.trim();
    if video_id.is_empty() || video_id.contains("://") || video_id.contains(char::is_whitespace) {
        return Ok(false);
    }
    let path = ensure_archive_file(save_dir)?;
    if queries::archive_ids(save_dir).contains(video_id) {
        return Ok(false);
    }
    let tag = media_ids::archive_tag_for_id(video_id);
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(|err| {
            AppError::io_with(
                "Could not update the download archive.",
                format!("{}: {err}", path.display()),
            )
        })?;
    writeln!(file, "{tag} {video_id}").map_err(|err| {
        AppError::io_with(
            "Could not write to the download archive.",
            format!("{}: {err}", path.display()),
        )
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_lines_round_trip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let save_dir = dir.path().to_string_lossy().to_string();

        assert!(ensure_archive_file(&save_dir).is_ok());
        assert!(add_archive_id(&save_dir, "dhs-abc123").expect("first write"));
        assert!(!add_archive_id(&save_dir, "dhs-abc123").expect("duplicate"));

        let contents = fs::read_to_string(dir.path().join(ARCHIVE_FILE)).expect("archive contents");
        assert_eq!(contents.trim(), "donghuastream dhs-abc123");
    }

    #[test]
    fn raw_urls_are_never_archived() {
        let dir = tempfile::tempdir().expect("temp dir");
        let save_dir = dir.path().to_string_lossy().to_string();
        assert!(!add_archive_id(&save_dir, "https://example.com/x").expect("ignored"));
        assert!(!add_archive_id(&save_dir, "  ").expect("ignored"));
    }
}
