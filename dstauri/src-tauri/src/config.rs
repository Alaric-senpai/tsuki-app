//! Application configuration: a JSON file in the OS config directory.
//!
//! Mirrors the legacy `app_config.py` semantics (defaults merged, unknown
//! settings dropped, atomic private writes) minus the PostgreSQL block.
//! Settings are kept as strings so the legacy shape survives a round-trip.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::AppError;

pub const CONFIG_VERSION: u32 = 3;
pub const CONFIG_FILE: &str = "config.json";
pub const LEGACY_APP_DIR: &str = "donghua-stream";
pub const LEGACY_CONFIG_FILE: &str = "config.json";

/// Every setting the app understands, with its shipped default.
pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("global_save_dir", "~/Videos"),
    ("default_quality", "720p"),
    ("browser_cookies", "none"),
    ("max_retries", "3"),
    ("default_container", "mp4"),
    ("sub_langs", "en"),
    ("rate_limit_mb", "0"),
    ("per_series_subfolder", "1"),
    ("concurrent_downloads", "3"),
    ("background_downloads", "0"),
    // Application updater: checks run on startup by default; the checkbox in
    // the update dialog can opt out of both checks and background downloads.
    ("auto_check_app_updates", "1"),
    ("auto_download_app_updates", "1"),
];

pub(crate) const SAFE_SEARCH_VALUES: &[&str] = &["none", "moderate", "strict"];
pub(crate) const PLAYER_ENGINES: &[&str] = &["auto", "mpv", "html", "external"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YouTubeConfig {
    /// Opt-in: YouTube is an optional source and stays off until enabled.
    pub enabled: bool,
    pub api_key: String,
    pub region_code: String,
    pub language: String,
    pub safe_search: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExploreConfig {
    pub result_limit: u32,
    pub cache_minutes: u32,
    /// Source used by the Home "Updates" section when checking for new
    /// episodes. YouTube is never valid here.
    pub update_source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerConfig {
    pub engine: String,
    pub volume: f64,
    pub external_player: String,
    pub remember_position: bool,
    pub auto_play_next: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Meta {
    pub updated_at: Option<String>,
    pub legacy_settings_imported: bool,
    pub imported_legacy_config_at: Option<String>,
    /// True once the legacy catalog JSON has been imported into SQLite.
    pub legacy_data_imported: bool,
    pub legacy_data_imported_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Config {
    pub version: u32,
    pub youtube: YouTubeConfig,
    pub settings: BTreeMap<String, String>,
    pub explore: ExploreConfig,
    pub player: PlayerConfig,
    pub meta: Meta,
}

impl Default for Config {
    fn default() -> Self {
        normalize(Value::Null)
    }
}

/// Settings defaults as an ordered map.
pub fn default_settings() -> BTreeMap<String, String> {
    DEFAULT_SETTINGS
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

/// Coerce any legacy shape into a valid configuration.
pub fn normalize(raw: Value) -> Config {
    let root = raw.as_object().cloned().unwrap_or_default();
    let youtube_raw = section(&root, "youtube");
    let explore_raw = section(&root, "explore");
    let player_raw = section(&root, "player");
    let meta_raw = section(&root, "meta");

    let youtube = YouTubeConfig {
        enabled: bool_value(youtube_raw.get("enabled"), false),
        api_key: text(youtube_raw.get("api_key"), ""),
        region_code: text(youtube_raw.get("region_code"), "US").to_uppercase(),
        language: text(youtube_raw.get("language"), "en").to_lowercase(),
        safe_search: safe_search(youtube_raw.get("safe_search")),
    };

    let explore = ExploreConfig {
        result_limit: clamp_int(explore_raw.get("result_limit"), 18, 1, 50),
        cache_minutes: clamp_int(explore_raw.get("cache_minutes"), 20, 0, 1440),
        update_source: update_source(explore_raw.get("update_source")),
    };

    let player = PlayerConfig {
        engine: engine(player_raw.get("engine")),
        volume: number(player_raw.get("volume"))
            .filter(|value| value.is_finite())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0),
        external_player: text(player_raw.get("external_player"), ""),
        remember_position: bool_value(player_raw.get("remember_position"), true),
        auto_play_next: bool_value(player_raw.get("auto_play_next"), true),
    };

    let mut settings = default_settings();
    if let Some(stored) = root.get("settings").and_then(Value::as_object) {
        for (key, value) in stored {
            if let Some(slot) = settings.get_mut(key)
                && let Some(coerced) = coerce(value)
            {
                *slot = coerced;
            }
        }
    }

    let meta = Meta {
        updated_at: meta_raw
            .get("updated_at")
            .and_then(Value::as_str)
            .map(str::to_string),
        legacy_settings_imported: bool_value(meta_raw.get("legacy_settings_imported"), false),
        imported_legacy_config_at: meta_raw
            .get("imported_legacy_config_at")
            .and_then(Value::as_str)
            .map(str::to_string),
        legacy_data_imported: bool_value(meta_raw.get("legacy_data_imported"), false),
        legacy_data_imported_at: meta_raw
            .get("legacy_data_imported_at")
            .and_then(Value::as_str)
            .map(str::to_string),
    };

    Config {
        version: CONFIG_VERSION,
        youtube,
        settings,
        explore,
        player,
        meta,
    }
}

fn section(root: &Map<String, Value>, key: &str) -> Map<String, Value> {
    root.get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn text(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                fallback.to_string()
            } else {
                trimmed.to_string()
            }
        }
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => fallback.to_string(),
    }
}

fn coerce(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(raw) => Some(raw.trim().to_string()),
        Value::Bool(flag) => Some(if *flag { "1" } else { "0" }.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn clamp_int(value: Option<&Value>, fallback: u32, min: u32, max: u32) -> u32 {
    number(value)
        .filter(|parsed| parsed.is_finite())
        .map(|parsed| parsed.round().clamp(min as f64, max as f64) as u32)
        .unwrap_or(fallback)
}

fn bool_value(value: Option<&Value>, fallback: bool) -> bool {
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_some_and(|parsed| parsed != 0.0),
        Some(Value::String(raw)) => match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => fallback,
        },
        _ => fallback,
    }
}

fn safe_search(value: Option<&Value>) -> String {
    let candidate = text(value, "moderate").to_lowercase();
    if SAFE_SEARCH_VALUES.contains(&candidate.as_str()) {
        candidate
    } else {
        "moderate".to_string()
    }
}

/// Sources the update checker may read. YouTube intentionally excluded.
pub const UPDATE_SOURCES: [&str; 2] = ["donghuastream", "animecube"];

fn update_source(value: Option<&Value>) -> String {
    let candidate = text(value, "donghuastream").to_lowercase();
    if UPDATE_SOURCES.contains(&candidate.as_str()) {
        candidate
    } else {
        "donghuastream".to_string()
    }
}

fn engine(value: Option<&Value>) -> String {
    let candidate = text(value, "auto").to_lowercase();
    if PLAYER_ENGINES.contains(&candidate.as_str()) {
        candidate
    } else {
        "auto".to_string()
    }
}

/// Owns the config directory; all reads and writes go through here.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    dir: PathBuf,
}

impl ConfigStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(CONFIG_FILE)
    }

    /// Read the config, falling back to defaults when the file is missing.
    pub fn load(&self) -> Result<Config, AppError> {
        let path = self.path();
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(normalize(Value::Null));
            }
            Err(err) => {
                return Err(AppError::io_with(
                    format!("Could not read the configuration at {}.", path.display()),
                    err.to_string(),
                ));
            }
        };

        let value: Value = serde_json::from_str(&raw).map_err(|err| {
            AppError::config_with(
                "The configuration file is not valid JSON.",
                format!("{}: {err}", path.display()),
            )
        })?;

        Ok(normalize(value))
    }

    /// Load the config, creating the file on first run and importing the
    /// legacy `donghua-stream` config when one exists.
    pub fn load_or_create(&self) -> Result<Config, AppError> {
        self.load_or_create_with(legacy_config_path().as_deref())
    }

    /// Testable variant of [`Self::load_or_create`] with an explicit legacy path.
    pub fn load_or_create_with(&self, legacy: Option<&Path>) -> Result<Config, AppError> {
        let path = self.path();
        if path.exists() {
            return self.load();
        }

        let mut config = normalize(Value::Null);
        if let Some(legacy_path) = legacy {
            match read_legacy(legacy_path) {
                Ok(Some(value)) => {
                    config = normalize(value);
                    config.meta.imported_legacy_config_at = Some(now_iso8601());
                    log::info!(
                        "imported legacy configuration from {}",
                        legacy_path.display()
                    );
                }
                Ok(None) => {}
                Err(err) => log::warn!(
                    "could not read legacy configuration at {}: {err}",
                    legacy_path.display()
                ),
            }
        }

        self.save(&config)?;
        Ok(config)
    }

    /// Persist a config, stamping the schema version and update time.
    pub fn save(&self, config: &Config) -> Result<(), AppError> {
        let mut outgoing = config.clone();
        outgoing.version = CONFIG_VERSION;
        outgoing.meta.updated_at = Some(now_iso8601());
        self.write(&outgoing)
    }

    /// Update one known setting and persist the result.
    pub fn update_setting(&self, key: &str, value: &str) -> Result<Config, AppError> {
        let mut config = self.load()?;
        if !config.settings.contains_key(key) {
            return Err(AppError::config(format!("Unknown setting: {key}")));
        }
        config.settings.insert(key.to_string(), value.to_string());
        self.save(&config)?;
        Ok(config)
    }

    fn write(&self, config: &Config) -> Result<(), AppError> {
        ensure_dir(&self.dir)?;

        let mut body = serde_json::to_string_pretty(config).map_err(|err| {
            AppError::internal_with("Could not serialize the configuration.", err.to_string())
        })?;
        body.push('\n');

        let target = self.path();
        let temp = self
            .dir
            .join(format!(".{}.{}.tmp", CONFIG_FILE, std::process::id()));

        let result = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&temp)?;
            restrict_file(&file)?;
            file.write_all(body.as_bytes())?;
            file.flush()?;
            file.sync_all()?;
            drop(file);

            #[cfg(windows)]
            if target.exists() {
                fs::remove_file(&target)?;
            }

            fs::rename(&temp, &target)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(0o600))?;
            }

            Ok(())
        })();

        if let Err(err) = result {
            let _ = fs::remove_file(&temp);
            return Err(AppError::io_with(
                format!("Could not save the configuration to {}.", target.display()),
                err.to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(unix)]
fn restrict_file(file: &fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_file(_file: &fs::File) -> std::io::Result<()> {
    Ok(())
}

fn ensure_dir(dir: &Path) -> Result<(), AppError> {
    if dir.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(dir).map_err(|err| {
        AppError::io_with(
            format!(
                "Could not create the configuration directory at {}.",
                dir.display()
            ),
            err.to_string(),
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }

    Ok(())
}

fn read_legacy(path: &Path) -> Result<Option<Value>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|err| err.to_string())
}

/// `$XDG_CONFIG_HOME/donghua-stream/config.json` (or `~/.config/...`).
pub fn legacy_config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(
            PathBuf::from(xdg)
                .join(LEGACY_APP_DIR)
                .join(LEGACY_CONFIG_FILE),
        );
    }
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(".config")
                .join(LEGACY_APP_DIR)
                .join(LEGACY_CONFIG_FILE)
        })
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ` without a chrono dependency.
pub fn now_iso8601() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_iso8601(seconds)
}

fn format_iso8601(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn defaults_when_no_file_exists() {
        let dir = TempDir::new().expect("temp dir");
        let store = ConfigStore::new(dir.path().join("tsuki"));
        let config = store.load().expect("load");

        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.youtube.region_code, "US");
        assert_eq!(config.youtube.safe_search, "moderate");
        assert_eq!(config.explore.result_limit, 18);
        assert_eq!(config.explore.cache_minutes, 20);
        assert_eq!(config.settings["default_quality"], "720p");
        assert_eq!(config.settings["concurrent_downloads"], "3");
        assert_eq!(config.player.engine, "auto");
        assert!(config.player.remember_position);
        assert!(!store.path().exists());
    }

    #[test]
    fn normalize_coerces_and_clamps_legacy_values() {
        let config = normalize(json!({
            "version": 2,
            "youtube": {
                "api_key": "  key-123  ",
                "region_code": "de",
                "safe_search": "STRICT"
            },
            "settings": {
                "max_retries": 5,
                "per_series_subfolder": true,
                "default_quality": "1080p",
                "unknown_key": "ignored",
                "rate_limit_mb": null
            },
            "explore": { "result_limit": 500, "cache_minutes": -4 },
            "player": { "engine": "vlc", "volume": 4.2 },
            "database": { "host": "dropped" }
        }));

        assert_eq!(config.youtube.api_key, "key-123");
        assert_eq!(config.youtube.region_code, "DE");
        assert_eq!(config.youtube.safe_search, "strict");
        assert_eq!(config.settings["max_retries"], "5");
        assert_eq!(config.settings["per_series_subfolder"], "1");
        assert_eq!(config.settings["default_quality"], "1080p");
        assert_eq!(config.settings["rate_limit_mb"], "0");
        assert!(!config.settings.contains_key("unknown_key"));
        assert_eq!(config.explore.result_limit, 50);
        assert_eq!(config.explore.cache_minutes, 0);
        assert_eq!(config.player.engine, "auto");
        assert_eq!(config.player.volume, 1.0);
        assert_eq!(config.version, CONFIG_VERSION);
    }

    #[test]
    fn save_round_trips_and_restricts_permissions() {
        let dir = TempDir::new().expect("temp dir");
        let store = ConfigStore::new(dir.path().join("tsuki"));

        let mut config = store.load().expect("load");
        config
            .settings
            .insert("default_quality".to_string(), "1080p".to_string());
        store.save(&config).expect("save");

        let loaded = store.load().expect("reload");
        assert_eq!(loaded.settings["default_quality"], "1080p");
        assert!(loaded.meta.updated_at.is_some());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = fs::metadata(store.path())
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
            let dir_mode = fs::metadata(&store.dir)
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
        }
    }

    #[test]
    fn imports_legacy_config_on_first_run() {
        let dir = TempDir::new().expect("temp dir");
        let legacy_path = dir.path().join("legacy.json");
        fs::write(
            &legacy_path,
            serde_json::to_string(&json!({
                "version": 2,
                "youtube": { "api_key": "legacy-key" },
                "settings": { "default_quality": "480p" },
                "explore": { "result_limit": 9 }
            }))
            .expect("serialize"),
        )
        .expect("write legacy");

        let store = ConfigStore::new(dir.path().join("tsuki"));
        let config = store
            .load_or_create_with(Some(&legacy_path))
            .expect("import");

        assert_eq!(config.youtube.api_key, "legacy-key");
        assert_eq!(config.settings["default_quality"], "480p");
        assert_eq!(config.explore.result_limit, 9);
        assert!(config.meta.imported_legacy_config_at.is_some());
        assert!(store.path().exists());
    }

    #[test]
    fn missing_legacy_file_is_not_an_error() {
        let dir = TempDir::new().expect("temp dir");
        let store = ConfigStore::new(dir.path().join("tsuki"));
        let legacy = dir.path().join("does-not-exist.json");
        let config = store.load_or_create_with(Some(&legacy)).expect("create");
        assert_eq!(config.settings["default_quality"], "720p");
        assert!(config.meta.imported_legacy_config_at.is_none());
    }

    #[test]
    fn update_setting_rejects_unknown_keys() {
        let dir = TempDir::new().expect("temp dir");
        let store = ConfigStore::new(dir.path().join("tsuki"));
        let error = store.update_setting("nope", "1").expect_err("must fail");
        assert_eq!(error.code(), "config");
        assert!(error.message().contains("Unknown setting"));
    }

    #[test]
    fn iso8601_formats_known_timestamps() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
