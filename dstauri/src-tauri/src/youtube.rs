//! YouTube Data API v3 client with a disk cache.
//!
//! Ported from the legacy `youtube_api.py`. The desktop app never calls this on
//! Explore mount: only an explicit user search reaches the API. API keys stay
//! in the URL query only (never in cache filenames or error details).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::error::AppError;

const API_ROOT: &str = "https://www.googleapis.com/youtube/v3";
const USER_AGENT: &str = "DonghuaStream/1.0";

/// A normalized video, ready for the Explore poster grid.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    #[ts(as = "String")]
    pub channel: String,
    pub channel_id: String,
    pub published_at: String,
    pub thumbnail: String,
    pub url: String,
    #[ts(as = "f64")]
    pub duration_sec: i64,
    pub duration: String,
    #[ts(as = "f64")]
    pub views: i64,
    #[ts(as = "f64")]
    pub likes: i64,
    #[ts(as = "f64")]
    pub comments: i64,
    pub views_text: String,
    pub age_text: String,
    pub live: bool,
}

/// One rail of the Explore discovery view.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
pub struct YouTubeRail {
    pub title: String,
    pub query: String,
    pub videos: Vec<YouTubeVideo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../lib/ipc/bindings/")]
pub struct YouTubeDiscovery {
    pub rails: Vec<YouTubeRail>,
    pub errors: Vec<String>,
    pub fetched_at: String,
}

/// Parse an ISO-8601 duration (`PT24M10S`, `P1DT2S`, …) into seconds.
pub fn duration_seconds(raw: &str) -> i64 {
    if !raw.starts_with('P') {
        return 0;
    }
    let mut seconds = 0i64;
    let mut number = String::new();
    let bytes = raw.as_bytes();
    let mut index = 1usize;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_ascii_digit() {
            number.push(ch);
            index += 1;
            continue;
        }
        let value = number.parse::<i64>().unwrap_or(0);
        number.clear();
        seconds += match ch {
            'D' => value * 86_400,
            'H' => value * 3_600,
            'M' => value * 60,
            'S' => value,
            // T is a separator with no numeric component. It still has to
            // advance the cursor: `continue` here loops on `PT…` forever.
            'T' => 0,
            _ => return 0,
        };
        index += 1;
    }
    seconds
}

pub fn format_duration(seconds: i64) -> String {
    if seconds <= 0 {
        return "—".to_string();
    }
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

pub fn format_views(views: i64) -> String {
    if views <= 0 {
        return "no views yet".to_string();
    }
    if views >= 1_000_000_000 {
        return format!("{:.1}B", views as f64 / 1_000_000_000.0);
    }
    if views >= 1_000_000 {
        return format!("{:.1}M", views as f64 / 1_000_000.0);
    }
    if views >= 1_000 {
        return format!("{:.1}K", views as f64 / 1_000.0);
    }
    views.to_string()
}

/// Rough relative age against `now` (unix seconds). Kept pure for tests.
pub fn human_age(published: i64, now: i64) -> String {
    let delta = (now - published).max(0);
    if delta < 3_600 {
        return "today".to_string();
    }
    if delta < 86_400 {
        return format!("{} hours ago", delta / 3_600);
    }
    if delta < 172_800 {
        return "yesterday".to_string();
    }
    if delta < 604_800 {
        return format!("{} days ago", delta / 86_400);
    }
    if delta < 2_592_000 {
        return format!("{} weeks ago", delta / 604_800);
    }
    if delta < 31_536_000 {
        return format!("{} months ago", delta / 2_592_000);
    }
    format!("{} years ago", delta / 31_536_000)
}

pub fn best_thumbnail(item: &Value) -> String {
    let thumbs = item.get("snippet").and_then(|v| v.get("thumbnails"));
    for key in ["maxres", "standard", "high", "medium", "default"] {
        let url = thumbs
            .and_then(|v| v.get(key))
            .and_then(|v| v.get("url"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if !url.is_empty() {
            return url.to_string();
        }
    }
    String::new()
}

fn value_str(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or("").to_string()
}

fn value_i64(value: Option<&Value>) -> i64 {
    value
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_i64())
        })
        .unwrap_or(0)
}

fn parse_published(raw: &str) -> i64 {
    // ISO-8601 UTC; the fallback keeps a malformed value harmless.
    chrono_kind_seconds(raw)
}

fn chrono_kind_seconds(raw: &str) -> i64 {
    // A tiny RFC3339 parser: `YYYY-MM-DDTHH:MM:SSZ`. Anything else → 0.
    if raw.len() < 20 {
        return 0;
    }
    let date: Vec<i64> = raw[0..10]
        .split('-')
        .filter_map(|p| p.parse().ok())
        .collect();
    let time: Vec<i64> = raw[11..19]
        .split(':')
        .filter_map(|p| p.parse().ok())
        .collect();
    if date.len() != 3 || time.len() != 3 {
        return 0;
    }
    let days = days_from_civil(date[0], date[1], date[2]);
    days * 86_400 + time[0] * 3_600 + time[1] * 60 + time[2]
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Merge a `search.list` item with its `videos.list` detail (when present).
pub fn normalize_video(item: &Value, details: Option<&Value>, now: i64) -> YouTubeVideo {
    let snippet = item.get("snippet");
    let detail_snippet = details.and_then(|d| d.get("snippet"));
    let content = details.and_then(|d| d.get("contentDetails"));
    let stats = details.and_then(|d| d.get("statistics"));

    let id = value_str(
        item.get("id")
            .and_then(|v| v.get("videoId"))
            .or_else(|| item.get("id")),
    );
    let published = value_str(
        detail_snippet
            .or(snippet)
            .and_then(|v| v.get("publishedAt")),
    );
    let title = value_str(detail_snippet.or(snippet).and_then(|v| v.get("title")));
    let channel = value_str(
        detail_snippet
            .or(snippet)
            .and_then(|v| v.get("channelTitle")),
    );
    let channel_id = value_str(detail_snippet.or(snippet).and_then(|v| v.get("channelId")));
    let duration_api = value_str(content.and_then(|v| v.get("duration")));
    let duration_sec = duration_seconds(&duration_api);

    YouTubeVideo {
        thumbnail: best_thumbnail(item),
        url: format!("https://www.youtube.com/watch?v={id}"),
        views_text: format_views(value_i64(stats.and_then(|v| v.get("viewCount")))),
        age_text: human_age(parse_published(&published), now),
        id,
        title,
        channel,
        channel_id,
        published_at: published,
        duration_sec,
        duration: format_duration(duration_sec),
        views: value_i64(stats.and_then(|v| v.get("viewCount"))),
        likes: value_i64(stats.and_then(|v| v.get("likeCount"))),
        comments: value_i64(stats.and_then(|v| v.get("commentCount"))),
        live: duration_api.is_empty()
            && value_str(content.and_then(|v| v.get("liveBroadcastContent"))) == "live",
    }
}

/// Cache key hash; the API key is deliberately excluded.
pub fn cache_key(endpoint: &str, params: &BTreeMap<String, String>) -> String {
    let mut query = String::new();
    for (index, (key, value)) in params.iter().filter(|(k, _)| *k != "key").enumerate() {
        if index > 0 {
            query.push('&');
        }
        query.push_str(key);
        query.push('=');
        query.push_str(value);
    }
    let digest = Sha256::digest(format!("{endpoint}?{query}").as_bytes());
    let mut text = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

fn friendly_http_error(status: reqwest::StatusCode, body: &str) -> AppError {
    let reason = body
        .parse::<Value>()
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("errors"))
                .and_then(Value::as_array)
                .and_then(|a| a.first().cloned())
        })
        .and_then(|e| e.get("reason").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    let (message, detail) = match status.as_u16() {
        400 | 403 if reason == "quotaExceeded" || reason == "dailyLimitExceeded" => (
            "The YouTube API quota for this key is exhausted. Try again tomorrow.",
            reason,
        ),
        400 | 403 if reason == "keyInvalid" || reason == "badRequest" => (
            "The YouTube API key was rejected. Check it in Settings.",
            reason,
        ),
        403 => (
            "YouTube refused this request. Check the API key in Settings.",
            reason,
        ),
        _ => (
            "YouTube returned an error.",
            format!("HTTP {status}: {body}"),
        ),
    };
    AppError::config_with(message, detail)
}

#[derive(Clone)]
pub struct YouTubeClient {
    api_key: String,
    region: String,
    language: String,
    safe_search: String,
    cache_dir: PathBuf,
    cache_seconds: u64,
    http: Client,
}

impl YouTubeClient {
    pub fn new(
        api_key: String,
        region: String,
        language: String,
        safe_search: String,
        cache_dir: PathBuf,
        cache_minutes: u64,
    ) -> Self {
        Self {
            api_key,
            region,
            language,
            safe_search,
            cache_dir,
            cache_seconds: cache_minutes * 60,
            http: Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    /// Fetch one endpoint from cache or the API. `params` must not include the key.
    async fn get(
        &self,
        endpoint: &str,
        mut params: BTreeMap<String, String>,
    ) -> Result<Value, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::config(
                "Set a YouTube API key in Settings to search YouTube.",
            ));
        }
        params.insert("key".to_string(), self.api_key.clone());
        params.insert("part".to_string(), "snippet".to_string());

        let mut cache_file = self.cache_dir.clone();
        let _ = fs::create_dir_all(&cache_file);
        cache_file.push(format!("{endpoint}-{}.json", cache_key(endpoint, &params)));

        let cache_fresh = self.cache_seconds > 0
            && fs::metadata(&cache_file)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age.as_secs() < self.cache_seconds);
        let cached = cache_fresh
            .then(|| fs::read_to_string(&cache_file).ok())
            .flatten()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        if let Some(value) = cached {
            return Ok(value);
        }

        let response = self
            .http
            .get(format!("{API_ROOT}/{endpoint}"))
            .query(&params)
            .send()
            .await
            .map_err(|err| {
                AppError::config_with(
                    "Could not reach YouTube. Check your connection and try again.",
                    err.to_string(),
                )
            })?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(friendly_http_error(status, &body));
        }
        let value: Value = serde_json::from_str(&body).map_err(|err| {
            AppError::internal_with("YouTube sent a malformed response.", err.to_string())
        })?;

        // A cache write failure is not worth failing a search.
        if self.cache_seconds > 0 {
            if let Some(parent) = cache_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp = cache_file.with_extension("tmp");
            if fs::write(&tmp, &body).is_ok() {
                let _ = fs::rename(&tmp, &cache_file);
            }
        }
        Ok(value)
    }

    /// Search, then fetch details for all results with exactly one videos.list.
    pub async fn search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<YouTubeVideo>, AppError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let limit = max_results.clamp(1, 50);
        let mut params = BTreeMap::new();
        params.insert("q".to_string(), query.trim().to_string());
        params.insert("type".to_string(), "video".to_string());
        params.insert("maxResults".to_string(), limit.to_string());
        params.insert("order".to_string(), "relevance".to_string());
        params.insert("regionCode".to_string(), self.region.clone());
        params.insert("relevanceLanguage".to_string(), self.language.clone());
        params.insert("safeSearch".to_string(), self.safe_search.clone());
        params.insert("videoEmbeddable".to_string(), "true".to_string());
        let page = self.get("search", params).await?;
        let items = page
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let details = self.details(&items).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let detail = details
                    .iter()
                    .find(|d| {
                        d.get("id").and_then(Value::as_str)
                            == item
                                .get("id")
                                .and_then(|v| v.get("videoId"))
                                .and_then(Value::as_str)
                    })
                    .or_else(|| details.get(index));
                normalize_video(item, detail, now)
            })
            .collect())
    }

    async fn details(&self, items: &[Value]) -> Result<Vec<Value>, AppError> {
        let ids: Vec<String> = items
            .iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(|v| v.get("videoId"))
                    .and_then(Value::as_str)
            })
            .take(50)
            .map(str::to_string)
            .collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), ids.join(","));
        params.insert(
            "part".to_string(),
            "snippet,contentDetails,statistics".to_string(),
        );
        let page = self.get("videos", params).await?;
        Ok(page
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn video(&self, id: &str) -> Result<Option<YouTubeVideo>, AppError> {
        if id.is_empty() {
            return Ok(None);
        }
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), id.to_string());
        params.insert(
            "part".to_string(),
            "snippet,contentDetails,statistics".to_string(),
        );
        let page = self.get("videos", params).await?;
        let item = page
            .get("items")
            .and_then(Value::as_array)
            .and_then(|a| a.first());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(item.map(|value| normalize_video(value, Some(value), now)))
    }

    /// Trending chart. Used by the discovery rail, never on mount.
    pub async fn trending(&self, max_results: usize) -> Result<Vec<YouTubeVideo>, AppError> {
        let mut params = BTreeMap::new();
        params.insert("chart".to_string(), "mostPopular".to_string());
        params.insert("videoCategoryId".to_string(), "1".to_string());
        params.insert(
            "maxResults".to_string(),
            max_results.clamp(1, 50).to_string(),
        );
        params.insert("regionCode".to_string(), self.region.clone());
        params.insert(
            "part".to_string(),
            "snippet,contentDetails,statistics".to_string(),
        );
        let page = self.get("videos", params).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(page
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|item| normalize_video(item, Some(item), now))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_parsing_matches_legacy() {
        assert_eq!(duration_seconds("PT24M10S"), 1450);
        assert_eq!(duration_seconds("PT1H2M3S"), 3723);
        assert_eq!(duration_seconds("P1DT2S"), 86_402);
        assert_eq!(duration_seconds("nonsense"), 0);
        assert_eq!(format_duration(0), "—");
        assert_eq!(format_duration(1450), "24:10");
        assert_eq!(format_duration(3723), "1:02:03");
    }

    #[test]
    fn view_formatting_matches_legacy() {
        assert_eq!(format_views(0), "no views yet");
        assert_eq!(format_views(999), "999");
        assert_eq!(format_views(1500), "1.5K");
        assert_eq!(format_views(2_300_000), "2.3M");
        assert_eq!(format_views(1_100_000_000), "1.1B");
    }

    #[test]
    fn age_formatting_matches_legacy() {
        let now = 1_800_000_000;
        assert_eq!(human_age(now - 60, now), "today");
        assert_eq!(human_age(now - 7_200, now), "2 hours ago");
        assert_eq!(human_age(now - 100_000, now), "yesterday");
        assert_eq!(human_age(now - 300_000, now), "3 days ago");
        assert_eq!(human_age(now - 40_000_000, now), "1 years ago");
    }

    #[test]
    fn thumbnail_prefers_the_largest_available() {
        let item: Value = serde_json::from_str(
            r#"{"snippet":{"thumbnails":{"default":{"url":"d"},"high":{"url":"h"}}}}"#,
        )
        .unwrap();
        assert_eq!(best_thumbnail(&item), "h");
    }

    #[test]
    fn normalization_merges_details_over_search() {
        let search: Value = serde_json::from_str(
            r#"{"id":{"videoId":"abc"},"snippet":{"title":"Search title","channelTitle":"Channel","publishedAt":"2024-01-02T03:04:05Z","thumbnails":{"high":{"url":"thumb"}}}}"#,
        )
        .unwrap();
        let detail: Value = serde_json::from_str(
            r#"{"id":"abc","snippet":{"title":"Final title","channelTitle":"Channel"},"contentDetails":{"duration":"PT24M10S"},"statistics":{"viewCount":"1500","likeCount":"42","commentCount":"3"}}"#,
        )
        .unwrap();
        let video = normalize_video(&search, Some(&detail), 0);
        assert_eq!(video.id, "abc");
        assert_eq!(video.title, "Final title");
        assert_eq!(video.duration_sec, 1450);
        assert_eq!(video.duration, "24:10");
        assert_eq!(video.views_text, "1.5K");
        assert_eq!(video.views, 1500);
        assert_eq!(video.likes, 42);
        assert_eq!(video.url, "https://www.youtube.com/watch?v=abc");
    }

    #[test]
    fn cache_key_excludes_the_api_key() {
        let mut a = BTreeMap::new();
        a.insert("q".to_string(), "donghua".to_string());
        a.insert("key".to_string(), "SECRET".to_string());
        let mut b = BTreeMap::new();
        b.insert("q".to_string(), "donghua".to_string());
        b.insert("key".to_string(), "DIFFERENT".to_string());
        assert_eq!(cache_key("search", &a), cache_key("search", &b));
        assert_ne!(cache_key("search", &a), cache_key("videos", &a));
    }

    #[test]
    fn quota_errors_are_friendly() {
        let err = friendly_http_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"{"error":{"errors":[{"reason":"quotaExceeded"}]}}"#,
        );
        let text = err.to_string();
        assert!(text.contains("quota"), "{text}");
        assert!(!text.contains("SECRET"));
    }
}
