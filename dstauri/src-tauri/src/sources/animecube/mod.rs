//! AnimeCube source (ported from the legacy `AnimeCubeSource`).
//!
//! Unlike DonghuaStream this site serves its catalog inside Next.js RSC
//! payloads. Episode sources are encrypted: the site hands out a fresh
//! `X-Obf` token and an ETag, and both the versions registry and the per-
//! episode source list are AES-256-GCM blobs keyed by `sha256(obf|etag)` or
//! `sha256(obf|version)`. All of that is resolved at *play time*; only the
//! stable anime page URL is ever stored in the library.

use std::sync::OnceLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use super::browse::{CatalogFilters, CatalogPage, SourceCard, SourceEpisode, SourceSeries};
use super::donghuastream::browse::CatalogQuery;
use super::{ResolvedMedia, cache, client};
use crate::config::now_iso8601;
use crate::error::AppError;

pub const SOURCE_KEY: &str = "animecube";
pub const ANIMECUBE_BASE: &str = "https://animecube.live";

/// Hosts that this scraper owns.
pub fn is_animecube_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "animecube.live" || host.ends_with(".animecube.live")
}

fn re(pattern: &str) -> Regex {
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Regex>>> =
        OnceLock::new();
    let map = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut map = map.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(found) = map.get(pattern) {
        return found.clone();
    }
    let compiled = Regex::new(pattern).expect("invalid AnimeCube regex");
    map.insert(pattern.to_string(), compiled.clone());
    compiled
}

/// Concatenate the `self.__next_f.push(...)` chunks that make up the RSC text.
pub fn rsc_text(page: &str) -> String {
    let mut out = String::new();
    let pattern = r#"self\.__next_f\.push\(\[1,("(?:\\.|[^"\\])*")\]\)"#;
    for caps in re(pattern).captures_iter(page) {
        if let Ok(chunk) = serde_json::from_str::<String>(&caps[1]) {
            out.push_str(&chunk);
        }
    }
    // The flight payload is double-escaped now: one `json.loads` still leaves
    // `\"anime\":` in the text, so markers need a second unescape pass.
    out.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// Read a JSON value that starts after `marker`.
pub fn json_after(text: &str, marker: &str) -> Option<Value> {
    let start = text.find(marker)? + marker.len();
    let rest = text[start..].trim_start();
    let mut de = serde_json::Deserializer::from_str(rest);
    let value = Value::deserialize(&mut de).ok()?;
    Some(value)
}

use serde::Deserialize;

/// Find the `detail.anime` object embedded in an RSC payload.
pub fn find_anime_object(rsc: &str) -> Option<Value> {
    let marker = r#""detail":{"anime":"#;
    let at = rsc.find(marker)?;
    let brace = at + rsc[at..].find('{')?;
    let mut de = serde_json::Deserializer::from_str(rsc[brace..].trim_start());
    let outer = Value::deserialize(&mut de).ok()?;
    outer.get("anime").cloned().or(Some(outer))
}

/// Decrypt the site's base64(AES-256-GCM) payload. The first 12 bytes are the
/// nonce; there is no associated data.
pub fn decrypt_payload(blob: &str, key_material: &str) -> Result<Value, AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob.trim())
        .map_err(|err| {
            AppError::source_with(
                "AnimeCube returned an unreadable source payload.",
                err.to_string(),
            )
        })?;
    if bytes.len() < 28 {
        return Err(AppError::source_with(
            "AnimeCube returned an incomplete source payload.",
            "",
        ));
    }
    let digest = Sha256::digest(key_material.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(&digest).expect("SHA-256 is always 32 bytes");
    let nonce = Nonce::try_from(&bytes[..12]).expect("a 12-byte nonce");
    let plain = cipher.decrypt(&nonce, &bytes[12..]).map_err(|err| {
        AppError::source_with(
            "AnimeCube could not decode this episode's sources.",
            err.to_string(),
        )
    })?;
    serde_json::from_slice(&plain).map_err(|err| {
        AppError::source_with(
            "AnimeCube returned an invalid source list.",
            err.to_string(),
        )
    })
}

fn quality_rank(quality: &str) -> i64 {
    match quality.to_ascii_lowercase().as_str() {
        "4k" => 5,
        "2k" => 4,
        "1080p" => 3,
        "720p" => 2,
        "480p" => 1,
        _ => 0,
    }
}

fn platform_rank(platform: &str) -> i64 {
    match platform.to_ascii_lowercase().as_str() {
        "dailymotion" => 5,
        "youtube" => 4,
        "vimeo" => 3,
        "rumble" => 2,
        _ => 0,
    }
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Build the canonical page/embed URL for one source entry. Unlisted uploads
/// expose a `privateId`, which must be preferred over `videoId`.
pub fn source_url(source: &Value) -> Option<String> {
    let platform = text(source.get("platform")).to_ascii_lowercase();
    let video = {
        let private = text(source.get("privateId"));
        if private.is_empty() {
            text(source.get("videoId"))
        } else {
            private
        }
    };
    if !video.is_empty() {
        let url = match platform.as_str() {
            "dailymotion" => format!("https://www.dailymotion.com/video/{video}"),
            "youtube" => format!("https://www.youtube.com/watch?v={video}"),
            "vimeo" => format!("https://vimeo.com/{video}"),
            "rumble" => {
                let publisher = text(source.get("publisherId"));
                if publisher.is_empty() {
                    format!("https://rumble.com/embed/{video}/")
                } else {
                    format!("https://rumble.com/embed/{video}/?pub={publisher}")
                }
            }
            _ => String::new(),
        };
        if !url.is_empty() {
            return Some(url);
        }
    }
    for key in ["url", "embedUrl", "embed_url", "src"] {
        let candidate = text(source.get(key));
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            return Some(candidate);
        }
    }
    None
}

/// Pick the best playable source, mirroring the legacy `(platform, quality,
/// subs)` ranking.
pub fn pick_source(sources: &[Value]) -> Result<(String, Value), AppError> {
    let mut best: Option<(i64, i64, i64, String, Value)> = None;
    for source in sources {
        let Some(url) = source_url(source) else {
            continue;
        };
        let platform = text(source.get("platform")).to_ascii_lowercase();
        let quality = text(source.get("quality"));
        let subs = if source.get("subtitles").is_some() {
            1
        } else {
            0
        };
        let score = (platform_rank(&platform), quality_rank(&quality), subs);
        let replace = best
            .as_ref()
            .is_none_or(|old| score > (old.0, old.1, old.2));
        if replace {
            best = Some((score.0, score.1, score.2, url, source.clone()));
        }
    }
    best.map(|(p, q, _, url, source)| {
        let _ = (p, q);
        (url, source)
    })
    .ok_or_else(|| {
        AppError::source_with(
            "AnimeCube did not return a playable source for this episode.",
            "",
        )
    })
}

/// Extract the slug from any anime page URL.
fn slug_from_url(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    let path = url.path();
    re(r"/(?:anime|title|watch)/([^/?#]+)")
        .captures(path)
        .map(|c| c[1].to_string())
        .or_else(|| {
            path.trim_matches('/')
                .rsplit('/')
                .next()
                .map(str::to_string)
        })
}

fn encoded(slug: &str) -> String {
    url::form_urlencoded::byte_serialize(slug.as_bytes()).collect()
}

fn card(item: &Value) -> Option<SourceCard> {
    let code = {
        let found = text(item.get("animeCode"));
        if found.is_empty() {
            text(item.get("id"))
        } else {
            found
        }
    };
    if code.is_empty() {
        return None;
    }
    let episode_text = {
        let found = text(item.get("latestEpisodeDisplay"));
        if found.is_empty() {
            item.get("reasons")
                .and_then(Value::as_array)
                .and_then(|list| list.first())
                .map(|reason| format!("Trending #{}", text(reason.get("rank"))))
                .unwrap_or_default()
        } else {
            found
        }
    };
    Some(SourceCard {
        id: code.clone(),
        title: text(item.get("title")),
        url: format!("{ANIMECUBE_BASE}/title/{}", encoded(&code)),
        thumbnail: text(item.get("coverImage")),
        status: text(item.get("status")),
        kind: String::new(),
        episode_text,
        source: SOURCE_KEY.to_string(),
    })
}

/// Fetch the shared catalog (cached) and extract its list of anime objects.
async fn fetch_catalog() -> Result<Vec<Value>, AppError> {
    if let Some(cached) = cache::read::<Vec<Value>>("animecube", "catalog", 60) {
        return Ok(cached.value);
    }
    let page = super::fetch_text(ANIMECUBE_BASE).await?;
    let rsc = rsc_text(&page);
    let items = json_after(&rsc, r#""sourceKind":"admin_config","items":"#)
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| AppError::source_with("AnimeCube did not return a catalog.", ""))?;
    cache::write("animecube", "catalog", &items, 60);
    Ok(items)
}

async fn anime(slug: &str) -> Result<Value, AppError> {
    if slug.trim().is_empty() {
        return Err(AppError::source_with("AnimeCube needs a title code.", ""));
    }
    let key = format!("anime:{slug}");
    if let Some(cached) = cache::read::<Value>("animecube", &key, 60) {
        return Ok(cached.value);
    }
    let url = format!("{ANIMECUBE_BASE}/title/{}", encoded(slug));
    let page = super::fetch_text(&url).await?;
    let object = find_anime_object(&rsc_text(&page))
        .ok_or_else(|| AppError::source_with("AnimeCube did not return this title.", ""))?;
    cache::write("animecube", &key, &object, 60);
    Ok(object)
}

/// Score a catalog item against every whitespace-separated query token.
fn search_score(item: &Value, tokens: &[String]) -> Option<i64> {
    if tokens.is_empty() {
        return Some(0);
    }
    let title = text(item.get("title")).to_ascii_lowercase();
    let code = {
        let found = text(item.get("animeCode"));
        if found.is_empty() {
            text(item.get("id"))
        } else {
            found
        }
    }
    .to_ascii_lowercase();
    let mut haystack = format!("{title} {code}");
    for key in ["aliases", "genres"] {
        if let Some(list) = item.get(key).and_then(Value::as_array) {
            for value in list {
                haystack.push(' ');
                haystack.push_str(&text(Some(value)).to_ascii_lowercase());
            }
        }
    }
    if let Some(classification) = item.get("classification").and_then(Value::as_object) {
        for (_, value) in classification {
            match value {
                Value::String(value) => {
                    haystack.push(' ');
                    haystack.push_str(&value.to_ascii_lowercase());
                }
                Value::Array(list) => {
                    for value in list {
                        haystack.push(' ');
                        haystack.push_str(&text(Some(value)).to_ascii_lowercase());
                    }
                }
                _ => {}
            }
        }
    }
    if !tokens.iter().all(|token| haystack.contains(token)) {
        return None;
    }
    let phrase = tokens.join(" ");
    let score = if title == phrase {
        100
    } else if title.starts_with(&phrase) {
        80
    } else if title.contains(&phrase) {
        60
    } else if !code.is_empty() && code.contains(&phrase) {
        50
    } else {
        10
    };
    Some(score)
}

pub async fn search(query: &str, max_results: usize) -> Result<Vec<SourceCard>, AppError> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let tokens: Vec<String> = query.split_whitespace().map(str::to_string).collect();
    let mut scored: Vec<(i64, String, SourceCard)> = fetch_catalog()
        .await?
        .iter()
        .filter_map(|item| {
            let score = search_score(item, &tokens)?;
            let card = card(item)?;
            Some((score, card.title.clone(), card))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(scored
        .into_iter()
        .take(max_results.max(1))
        .map(|(_, _, card)| card)
        .collect())
}

fn collect_episodes(watch_rsc: &str) -> Vec<SourceEpisode> {
    let Some(groups) =
        json_after(watch_rsc, r#""episodeGroups":"#).and_then(|value| value.as_array().cloned())
    else {
        return Vec::new();
    };
    let mut episodes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for group in groups {
        let Some(items) = group.get("episodes").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let code = text(item.get("episodeCode"));
            if code.is_empty() {
                continue;
            }
            let url = format!("{ANIMECUBE_BASE}/watch/{}", encoded(&code));
            if !seen.insert(url.clone()) {
                continue;
            }
            let number = item.get("number").and_then(Value::as_i64).or_else(|| {
                item.get("number")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse().ok())
            });
            let number_text = {
                let found = text(item.get("numberLabel"));
                if found.is_empty() {
                    number.map(|value| value.to_string()).unwrap_or_default()
                } else {
                    found
                }
            };
            let title = {
                let found = text(item.get("title"));
                if found.is_empty() {
                    format!("Episode {number_text}")
                } else {
                    found
                }
            };
            episodes.push(SourceEpisode {
                url,
                title,
                episode_number: number,
                number_text,
                date: text(item.get("publishedAt")),
            });
        }
    }
    episodes.sort_by_key(|episode| (episode.episode_number.unwrap_or(0), episode.url.clone()));
    episodes
}

fn recommendations(code: &str, genres: &[String], catalog: &[Value]) -> Vec<SourceCard> {
    let mut scored: Vec<(usize, f64, SourceCard)> = catalog
        .iter()
        .filter_map(|item| {
            let candidate = {
                let found = text(item.get("animeCode"));
                if found.is_empty() {
                    text(item.get("id"))
                } else {
                    found
                }
            };
            if candidate == code {
                return None;
            }
            let card = card(item)?;
            let item_genres = genre_values(item);
            let shared = genres
                .iter()
                .filter(|genre| {
                    item_genres
                        .iter()
                        .any(|other| other.eq_ignore_ascii_case(genre))
                })
                .count();
            let rating = item
                .get("rating")
                .and_then(Value::as_f64)
                .or_else(|| {
                    item.get("rating")
                        .and_then(Value::as_str)
                        .and_then(|v| v.parse().ok())
                })
                .unwrap_or(0.0);
            Some((shared, rating, card))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    scored
        .into_iter()
        .take(6)
        .map(|(_, _, card)| card)
        .collect()
}

pub async fn details(raw_url: &str) -> Result<SourceSeries, AppError> {
    let code = slug_from_url(raw_url)
        .ok_or_else(|| AppError::source_with("AnimeCube needs a title code.", ""))?;
    let object = anime(&code).await?;

    // Episode lists moved to the watch page. The catalog carries the primary
    // episode code, which is the natural entry point for a title.
    let catalog = fetch_catalog().await.unwrap_or_default();
    let primary = catalog
        .iter()
        .find(|item| {
            let found = text(item.get("animeCode"));
            (!found.is_empty() && found == code) || text(item.get("id")) == code
        })
        .map(|item| text(item.get("primaryWatchEpisodeCode")))
        .unwrap_or_default();
    let episodes = if primary.is_empty() {
        Vec::new()
    } else {
        let url = format!("{ANIMECUBE_BASE}/watch/{}", encoded(&primary));
        match super::fetch_text(&url).await {
            Ok(page) => collect_episodes(&rsc_text(&page)),
            Err(_) => Vec::new(),
        }
    };

    let genres = genre_values(&object);
    let recommendation_catalog: Vec<Value> = catalog;
    Ok(SourceSeries {
        source: SOURCE_KEY.to_string(),
        id: code.clone(),
        title: text(object.get("title")),
        url: format!("{ANIMECUBE_BASE}/title/{}", encoded(&code)),
        thumbnail: text(object.get("coverImage")),
        description: text(object.get("description")),
        status: text(object.get("status")),
        kind: String::new(),
        studio: text(object.get("studio")),
        network: String::new(),
        released: text(object.get("releaseDate")),
        duration: String::new(),
        season: String::new(),
        genres: genres.clone(),
        episode_count: episodes.len(),
        episodes,
        recommendations: recommendations(&code, &genres, &recommendation_catalog),
    })
}

fn genre_values(item: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(list) = item.get("genres").and_then(Value::as_array) {
        for value in list {
            let found = text(Some(value));
            if !found.is_empty() {
                out.push(found);
            }
        }
    }
    if let Some(classification) = item.get("classification").and_then(Value::as_object) {
        for (_, value) in classification {
            match value {
                Value::String(value) => out.push(value.clone()),
                Value::Array(list) => {
                    for value in list {
                        let found = text(Some(value));
                        if !found.is_empty() {
                            out.push(found);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn obf_token() -> String {
    let bytes: [u8; 16] = std::array::from_fn(|_| {
        // Not cryptographically important: it only salts the key and is sent
        // back as a header on the very next request.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u8)
            .unwrap_or(0)
            ^ (std::process::id() as u8)
    });
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Resolve an AnimeCube episode page to a direct playable URL. Called at play
/// and download time; the result is never persisted.
pub async fn resolve(raw_url: &str) -> Result<ResolvedMedia, AppError> {
    let parsed = Url::parse(raw_url)
        .map_err(|_| AppError::source_with("This AnimeCube episode link is invalid.", ""))?;
    let slug = slug_from_url(raw_url)
        .ok_or_else(|| AppError::source_with("This AnimeCube link has no series slug.", ""))?;
    let pairs: std::collections::HashMap<String, String> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let episode_id = pairs.get("episode").cloned().unwrap_or_default();
    let season_id = pairs.get("season").cloned().unwrap_or_default();
    let object = anime(&slug).await?;

    let primary_tab = object
        .get("primaryTabs")
        .and_then(Value::as_array)
        .and_then(|tabs| {
            tabs.iter().find(|tab| {
                tab.get("seasons")
                    .and_then(Value::as_array)
                    .is_some_and(|seasons| {
                        seasons
                            .iter()
                            .any(|season| text(season.get("id")) == season_id)
                    })
            })
        });
    let tab_id = primary_tab
        .and_then(|tab| text(tab.get("id")).into())
        .filter(|id: &String| !id.is_empty())
        .unwrap_or_default();

    // Versions registry: the ETag is part of the decryption key.
    let obf = obf_token();
    let versions_url = format!("{ANIMECUBE_BASE}/api/anime-sources-versions");
    let response = client()?
        .get(&versions_url)
        .header("Accept", "application/json")
        .header("X-Obf", &obf)
        .send()
        .await
        .map_err(|err| AppError::source_with("AnimeCube could not be reached.", err.to_string()))?;
    let etag = response
        .headers()
        .get("X-Registry-ETag")
        .or_else(|| response.headers().get(reqwest::header::ETAG))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    if !response.status().is_success() {
        return Err(AppError::source_with(
            format!("AnimeCube answered with HTTP {}.", response.status()),
            versions_url,
        ));
    }
    let body_text = response
        .text()
        .await
        .map_err(|err| AppError::source_with("AnimeCube could not be read.", err.to_string()))?;
    let body: Value = serde_json::from_str(&body_text).map_err(|err| {
        AppError::source_with(
            "AnimeCube returned an invalid version list.",
            err.to_string(),
        )
    })?;
    let blob = text(body.get("payload").or_else(|| body.get("data")));
    let registry = decrypt_payload(&blob, &format!("{obf}|{etag}"))?;
    let version = registry
        .get("bySeason")
        .and_then(|by_season| by_season.get(&slug))
        .and_then(|by_tab| by_tab.get(&tab_id))
        .and_then(|by_season| by_season.get(&season_id))
        .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|_| "")))
        .unwrap_or_default()
        .to_string();
    if version.is_empty() {
        return Err(AppError::source_with(
            "AnimeCube did not return a source version for this episode.",
            "",
        ));
    }

    let sources_url = format!(
        "{ANIMECUBE_BASE}/api/anime/{}/episode/{}/sources?v={}&primaryTabId={}&seasonId={}",
        encoded(&slug),
        encoded(&episode_id),
        encoded(&version),
        encoded(&tab_id),
        encoded(&season_id)
    );
    let obf = obf_token();
    let response = client()?
        .get(&sources_url)
        .header("Accept", "application/json")
        .header("X-Obf", &obf)
        .header(
            "Referer",
            format!("{ANIMECUBE_BASE}/anime/{}", encoded(&slug)),
        )
        .send()
        .await
        .map_err(|err| AppError::source_with("AnimeCube could not be reached.", err.to_string()))?;
    if !response.status().is_success() {
        return Err(AppError::source_with(
            format!("AnimeCube answered with HTTP {}.", response.status()),
            sources_url,
        ));
    }
    let body_text = response
        .text()
        .await
        .map_err(|err| AppError::source_with("AnimeCube could not be read.", err.to_string()))?;
    let body: Value = serde_json::from_str(&body_text).map_err(|err| {
        AppError::source_with(
            "AnimeCube returned an invalid source list.",
            err.to_string(),
        )
    })?;
    let blob = text(body.get("payload").or_else(|| body.get("data")));
    let decrypted = decrypt_payload(&blob, &format!("{obf}|{version}"))?;
    let sources = decrypted
        .get("sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (url, chosen) = pick_source(&sources)?;
    let is_hls = super::is_hls_url(&url);
    Ok(ResolvedMedia {
        url,
        title: text(chosen.get("title")),
        source: SOURCE_KEY.to_string(),
        video_id: episode_id,
        platform: text(chosen.get("platform")),
        is_hls,
    })
}

/// AnimeCube has no filter API — the Explore filter bar is hidden for this
/// source, so an empty set is enough to keep the shared IPC shape intact.
pub fn filters() -> CatalogFilters {
    CatalogFilters {
        genres: Vec::new(),
        orders: Vec::new(),
        statuses: Vec::new(),
        kinds: Vec::new(),
    }
}

/// Shared Explore entry point. AnimeCube's catalog is a single list, so paging
/// is applied locally; `cached` is false because each call uses the source
/// cache internally and the page itself is assembled in memory.
pub async fn catalog(query: &CatalogQuery, limit: usize) -> Result<CatalogPage, AppError> {
    let searching = !query.search.trim().is_empty();
    let all: Vec<SourceCard> = if searching {
        search(query.search.trim(), usize::MAX).await?
    } else {
        fetch_catalog()
            .await?
            .into_iter()
            .filter_map(|item| card(&item))
            .collect()
    };
    let page = query.page.max(1);
    let size = if limit == 0 { all.len() } else { limit };
    let start = page.saturating_sub(1) * size;
    let cards: Vec<SourceCard> = all.iter().skip(start).take(size).cloned().collect();
    let total = all.len();
    Ok(CatalogPage {
        source: SOURCE_KEY.to_string(),
        mode: if searching { "search" } else { "catalog" }.to_string(),
        query: query.search.trim().to_string(),
        page,
        has_more: start.saturating_add(cards.len()) < total,
        cards,
        total,
        cached: false,
        age_seconds: 0,
        fetched_at: now_iso8601(),
    })
}

/// Explore passes either a full card URL or a bare slug.
pub async fn series(id: &str) -> Result<SourceSeries, AppError> {
    let url = if id.starts_with("http://") || id.starts_with("https://") {
        id.to_string()
    } else {
        format!("{ANIMECUBE_BASE}/title/{}", encoded(id))
    };
    details(&url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsc_chunks_are_joined() {
        let page = r#"self.__next_f.push([1,"hello "]);self.__next_f.push([1,"world"])"#;
        assert_eq!(rsc_text(page), "hello world");
    }

    #[test]
    fn json_after_decodes_a_value() {
        assert_eq!(
            json_after(r#"xx "fallbackData":[1,2,3] yy"#, "\"fallbackData\":"),
            Some(serde_json::json!([1, 2, 3]))
        );
    }

    #[test]
    fn anime_object_is_found_in_rsc_noise() {
        let rsc = r#"garbage "detail":{"anime":{"animeCode":"x","title":"X"}} tail"#;
        let found = find_anime_object(rsc).expect("anime object");
        assert_eq!(text(found.get("animeCode")), "x");
    }

    #[test]
    fn encrypted_payload_round_trips() {
        let key_material = "abc123|etag";
        let digest = Sha256::digest(key_material.as_bytes());
        let cipher = Aes256Gcm::new_from_slice(&digest).expect("SHA-256 is always 32 bytes");
        let nonce = Nonce::from([7u8; 12]);
        let plain = br#"{"ok":true}"#;
        let sealed = cipher.encrypt(&nonce, plain.as_slice()).expect("encrypt");
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&sealed);
        let encoded = base64::engine::general_purpose::STANDARD.encode(blob);
        assert_eq!(
            decrypt_payload(&encoded, key_material).unwrap(),
            serde_json::json!({"ok":true})
        );
    }

    #[test]
    fn unlisted_private_id_wins() {
        let source = serde_json::json!({
            "platform": "dailymotion", "videoId": "public", "privateId": "secret"
        });
        assert_eq!(
            source_url(&source).as_deref(),
            Some("https://www.dailymotion.com/video/secret")
        );
    }

    #[test]
    fn best_source_is_picked_by_platform_then_quality() {
        let sources = vec![
            serde_json::json!({"platform": "youtube", "videoId": "y", "quality": "1080p"}),
            serde_json::json!({"platform": "dailymotion", "videoId": "d", "quality": "480p"}),
            serde_json::json!({"platform": "rumble", "videoId": "r"}),
        ];
        let (url, _) = pick_source(&sources).expect("a playable source");
        assert!(url.contains("dailymotion"));
    }

    #[test]
    fn slug_is_read_from_anime_urls() {
        for url in [
            "https://animecube.live/title/my-show",
            "https://animecube.live/watch/my-show",
            "https://animecube.live/anime/my-show?season=1",
        ] {
            assert_eq!(slug_from_url(url).as_deref(), Some("my-show"), "{url}");
        }
    }

    #[test]
    fn all_query_tokens_must_match() {
        let item = serde_json::json!({"title": "Solo Leveling", "animeCode": "solo-leveling"});
        let tokens = vec!["solo".to_string(), "leveling".to_string()];
        assert!(search_score(&item, &tokens).is_some());
        assert!(search_score(&item, &["solo".to_string(), "wrong".to_string()]).is_none());
    }
}
