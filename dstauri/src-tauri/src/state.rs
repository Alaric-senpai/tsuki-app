//! Shared application state managed by Tauri.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config::{
    Config, ConfigStore, PLAYER_ENGINES, SAFE_SEARCH_VALUES, UPDATE_SOURCES, now_iso8601,
};
use crate::db::Db;
use crate::downloader::DownloadManager;
use crate::error::AppError;

pub struct AppState {
    config_store: ConfigStore,
    config: Mutex<Config>,
    db: Db,
    data_dir: PathBuf,
    downloads: DownloadManager,
}

impl AppState {
    pub fn new(
        config_store: ConfigStore,
        config: Config,
        db: Db,
        data_dir: PathBuf,
        downloads: DownloadManager,
    ) -> Self {
        Self {
            config_store,
            config: Mutex::new(config),
            db,
            data_dir,
            downloads,
        }
    }

    pub fn config_store(&self) -> &ConfigStore {
        &self.config_store
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    /// Sequential yt-dlp queue owned by Rust.
    pub fn downloads(&self) -> &DownloadManager {
        &self.downloads
    }

    /// Tell the frontend to re-read catalog data.
    pub fn notify_library_changed(&self) {
        self.downloads.notify_library_changed();
    }

    /// App data directory; banner cache paths are relative to this.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn config(&self) -> Result<Config, AppError> {
        self.config
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| AppError::internal("The configuration lock is poisoned."))
    }

    /// Persist one setting and update the in-memory copy.
    pub fn update_setting(&self, key: &str, value: &str) -> Result<Config, AppError> {
        let mut guard = self
            .config
            .lock()
            .map_err(|_| AppError::internal("The configuration lock is poisoned."))?;
        let updated = self.config_store.update_setting(key, value)?;
        *guard = updated.clone();
        Ok(updated)
    }

    /// Apply a validated change to the config while holding the lock, then
    /// persist it. A failed validation leaves both copies untouched.
    fn update_config<F>(&self, change: F) -> Result<Config, AppError>
    where
        F: FnOnce(&mut Config) -> Result<(), AppError>,
    {
        let mut guard = self
            .config
            .lock()
            .map_err(|_| AppError::internal("The configuration lock is poisoned."))?;
        let mut updated = guard.clone();
        change(&mut updated)?;
        self.config_store.save(&updated)?;
        *guard = updated.clone();
        Ok(updated)
    }

    pub fn update_player_config(
        &self,
        engine: Option<String>,
        volume: Option<f64>,
        external_player: Option<String>,
        remember_position: Option<bool>,
        auto_play_next: Option<bool>,
    ) -> Result<Config, AppError> {
        self.update_config(|config| {
            if let Some(value) = engine {
                config.player.engine = validated_choice("player engine", &value, PLAYER_ENGINES)?;
            }
            if let Some(value) = volume {
                if !value.is_finite() {
                    return Err(AppError::config("Volume must be a finite number."));
                }
                config.player.volume = value.clamp(0.0, 1.0);
            }
            if let Some(value) = external_player {
                config.player.external_player = value.trim().to_string();
            }
            if let Some(value) = remember_position {
                config.player.remember_position = value;
            }
            if let Some(value) = auto_play_next {
                config.player.auto_play_next = value;
            }
            Ok(())
        })
    }

    pub fn update_explore_config(
        &self,
        result_limit: Option<u32>,
        cache_minutes: Option<u32>,
        update_source: Option<String>,
    ) -> Result<Config, AppError> {
        self.update_config(|config| {
            if let Some(value) = result_limit {
                config.explore.result_limit = value.clamp(1, 50);
            }
            if let Some(value) = cache_minutes {
                config.explore.cache_minutes = value.clamp(0, 1440);
            }
            if let Some(value) = update_source {
                let candidate = value.trim().to_lowercase();
                // YouTube has no episode list to diff against, so it is never
                // a valid update source; anything unknown keeps the current one.
                if UPDATE_SOURCES.contains(&candidate.as_str()) {
                    config.explore.update_source = candidate;
                }
            }
            Ok(())
        })
    }

    pub fn update_youtube_config(
        &self,
        enabled: Option<bool>,
        api_key: Option<String>,
        region_code: Option<String>,
        language: Option<String>,
        safe_search: Option<String>,
    ) -> Result<Config, AppError> {
        self.update_config(|config| {
            if let Some(value) = enabled {
                config.youtube.enabled = value;
            }
            if let Some(value) = api_key {
                config.youtube.api_key = value.trim().to_string();
            }
            if let Some(value) = region_code {
                config.youtube.region_code = normalized_code("region code", &value, true)?;
            }
            if let Some(value) = language {
                config.youtube.language = normalized_code("language", &value, false)?;
            }
            if let Some(value) = safe_search {
                config.youtube.safe_search =
                    validated_choice("safe search", &value, SAFE_SEARCH_VALUES)?;
            }
            Ok(())
        })
    }

    /// Record that the legacy catalog import has run.
    pub fn complete_legacy_import(&self) -> Result<(), AppError> {
        let mut guard = self
            .config
            .lock()
            .map_err(|_| AppError::internal("The configuration lock is poisoned."))?;
        let mut updated = guard.clone();
        updated.meta.legacy_data_imported = true;
        updated.meta.legacy_data_imported_at = Some(now_iso8601());
        self.config_store.save(&updated)?;
        *guard = updated;
        Ok(())
    }
}

fn validated_choice(label: &str, raw: &str, allowed: &[&str]) -> Result<String, AppError> {
    let value = raw.trim().to_lowercase();
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::config(format!("Unknown {label}: {raw}")))
    }
}

fn normalized_code(label: &str, raw: &str, uppercase: bool) -> Result<String, AppError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(AppError::config(format!("The {label} cannot be empty.")));
    }
    if uppercase {
        Ok(value.to_uppercase())
    } else {
        Ok(value.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_choices_and_codes() {
        assert_eq!(
            validated_choice("player engine", " MPV ", PLAYER_ENGINES).expect("engine"),
            "mpv"
        );
        assert!(validated_choice("player engine", "vlc", PLAYER_ENGINES).is_err());
        assert_eq!(
            validated_choice("safe search", "STRICT", SAFE_SEARCH_VALUES).expect("safe search"),
            "strict"
        );
        assert_eq!(
            normalized_code("region code", "de", true).expect("region"),
            "DE"
        );
        assert_eq!(
            normalized_code("language", " EN ", false).expect("language"),
            "en"
        );
        assert!(normalized_code("region code", " ", true).is_err());
    }
}
