//! Disk cache for Explore responses.
//!
//! Ported from the legacy `_cached()` helper in `sources.py`: one JSON file per
//! key under `<cache_dir>/<namespace>/<hash>.json`, expiry based on the file's
//! modification time, atomic writes, and **cache failures never break
//! Explore** — a cache miss simply refetches.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use serde::Serialize;
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};

/// Cache root, installed once during app setup.
static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Install the cache directory (`<app_cache_dir>/explore`).
pub fn init(dir: PathBuf) {
    let _ = ROOT.set(dir);
}

/// A cached value plus how old it is, for the UI's telemetry ticker.
pub struct Cached<T> {
    pub value: T,
    pub age_seconds: u64,
}

fn path_for(namespace: &str, key: &str) -> Option<PathBuf> {
    let root = ROOT.get()?;
    let digest = Sha1::digest(key.as_bytes());
    let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Some(root.join(namespace).join(format!("{}.json", &name[..32])))
}

/// Read a fresh cached value. `ttl_minutes == 0` disables caching entirely.
pub fn read<T: DeserializeOwned>(
    namespace: &str,
    key: &str,
    ttl_minutes: u32,
) -> Option<Cached<T>> {
    if ttl_minutes == 0 {
        return None;
    }
    let path = path_for(namespace, key)?;
    let metadata = fs::metadata(&path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?.as_secs();
    if age > u64::from(ttl_minutes) * 60 {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    let value = serde_json::from_str(&text).ok()?;
    Some(Cached {
        value,
        age_seconds: age,
    })
}

/// Best-effort atomic write; every error is swallowed on purpose.
pub fn write<T: Serialize>(namespace: &str, key: &str, value: &T, ttl_minutes: u32) {
    if ttl_minutes == 0 {
        return;
    }
    let Some(path) = path_for(namespace, key) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    restrict_dir(parent);
    let Ok(text) = serde_json::to_string(value) else {
        return;
    };
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    if fs::write(&temp, text).is_err() {
        return;
    }
    restrict_file(&temp);
    if fs::rename(&temp, &path).is_err() {
        let _ = fs::remove_file(&temp);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_and_expire() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The root is a process-wide OnceLock; point it at a unique namespace
        // instead of fighting other tests for it.
        let _ = ROOT.set(dir.path().to_path_buf());
        let namespace = format!("test-{}", std::process::id());

        write(&namespace, "key", &vec!["a".to_string()], 20);
        let hit: Cached<Vec<String>> = read(&namespace, "key", 20).expect("cache hit");
        assert_eq!(hit.value, vec!["a".to_string()]);
        assert!(hit.age_seconds < 5);

        // A zero TTL means "never cache".
        write(&namespace, "off", &1u32, 0);
        assert!(read::<u32>(&namespace, "off", 20).is_none());
        assert!(read::<Vec<String>>(&namespace, "missing", 20).is_none());
    }
}
