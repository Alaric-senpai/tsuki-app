//! Catalog, filter and series parsing for the live animecube.live site.
//!
//! The site is a Next.js app. Its public JSON endpoints (`/api/catalog`,
//! `/api/search`, `/api/watch/<code>/context`) carry the same data as the
//! rendered HTML, so the parsers below work on either shape. Both are pure
//! functions with captured fixtures; no network access happens here.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use regex::Regex;
use serde_json::Value;

use super::{BASE, SOURCE_KEY};
use crate::config::now_iso8601;
use crate::error::AppError;
use crate::sources::browse::{
    CatalogFilters, CatalogPage, FilterOption, Genre, SourceCard, SourceEpisode, SourceSeries,
};
use crate::sources::donghuastream::browse::CatalogQuery;
use crate::sources::{cache, fetch_text};

const NAMESPACE: &str = "animecube_live";

/// Compiled patterns are cached per process. Invalid patterns (all of ours are
/// constants) are cached as `None` and treated as "no match" — never a panic.
fn re(pattern: &str) -> Option<Regex> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Regex>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = match map.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(cached) = map.get(pattern) {
        return cached.clone();
    }
    let compiled = Regex::new(pattern).ok();
    map.insert(pattern.to_string(), compiled.clone());
    compiled
}

fn enc(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.trim().as_bytes()).collect()
}

fn decode_entities(input: &str) -> String {
    let mut value = input
        .replace("&#x27;", "'")
        .replace("&#039;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    while let Some(start) = value.find("&#") {
        let Some(relative_end) = value[start..].find(';') else {
            break;
        };
        let end = start + relative_end;
        let digits = &value[start + 2..end];
        let replacement = digits
            .strip_prefix('x')
            .or_else(|| digits.strip_prefix('X'))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| digits.parse::<u32>().ok())
            .and_then(char::from_u32)
            .map(|ch| ch.to_string())
            .unwrap_or_default();
        value.replace_range(start..=end, &replacement);
    }
    value
}

fn text_of(input: &str) -> String {
    let Some(tags) = re(r"(?is)<[^>]+>") else {
        return input.trim().to_string();
    };
    let without_tags = tags.replace_all(input, " ");
    let decoded = decode_entities(&without_tags);
    let Some(ws) = re(r"\s+") else {
        return decoded.trim().to_string();
    };
    ws.replace_all(decoded.trim(), " ").to_string()
}

fn attr_of(fragment: &str, name: &str) -> String {
    let pattern = format!(r#"(?is)\b{name}\s*=\s*"([^"]*)""#);
    let Some(compiled) = re(&pattern) else {
        return String::new();
    };
    compiled
        .captures(fragment)
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

fn first_text(pattern: &str, html: &str) -> String {
    let Some(compiled) = re(pattern) else {
        return String::new();
    };
    compiled
        .captures(html)
        .map(|caps| text_of(&caps[1]))
        .unwrap_or_default()
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

/// One catalog card rendered by the live `/catalog` grid. The poster links to
/// the *latest* episode (`/watch/<code>`); the stable series id is the anime
/// code embedded in the image path (`/series/<animeCode>/…`).
pub fn parse_cards_html(html: &str) -> Vec<SourceCard> {
    let Some(article) = re(r#"(?is)<article class="group relative min-w-0".*?</article>"#) else {
        return Vec::new();
    };
    let Some(watch) = re(r#"(?is)href="/watch/([^"/?#]+)""#) else {
        return Vec::new();
    };
    let Some(series_code) = re(r#"(?i)/series/([^/?#]+)/"#) else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for found in article.captures_iter(html) {
        let block = &found[0];
        let Some(code) = watch.captures(block).map(|caps| caps[1].to_string()) else {
            continue;
        };
        let url = format!("{BASE}/watch/{code}");
        if !seen.insert(url.clone()) {
            continue;
        }
        let title = attr_of(block, "alt");
        let title = if title.is_empty() {
            first_text(r"(?is)<h2[^>]*>(.*?)</h2>", block)
        } else {
            title
        };
        if title.is_empty() {
            continue;
        }
        let thumbnail = attr_of(block, "src");
        let status = first_text(
            r#"(?is)<span class="poster-label"[^>]*>(.*?)</span>"#,
            block,
        );
        let id = series_code
            .captures(block)
            .map(|caps| caps[1].to_string())
            .unwrap_or_else(|| code.clone());
        cards.push(SourceCard {
            id,
            title,
            url,
            thumbnail,
            status,
            kind: String::new(),
            episode_text: String::new(),
            source: SOURCE_KEY.to_string(),
        });
    }
    cards
}

/// Cards from the public `/api/catalog` and `/api/search` item arrays.
pub fn parse_cards_json(items: &[Value]) -> Vec<SourceCard> {
    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let code = {
            let found = text(item.get("animeCode"));
            if found.is_empty() {
                text(item.get("id"))
            } else {
                found
            }
        };
        if code.is_empty() || !seen.insert(code.clone()) {
            continue;
        }
        let title = text(item.get("title"));
        if title.is_empty() {
            continue;
        }
        let episode_text = {
            let found = text(item.get("latestEpisodeDisplay"));
            if found.is_empty() {
                item.get("reasons")
                    .and_then(Value::as_array)
                    .and_then(|reasons| reasons.first())
                    .and_then(|rank| rank.get("rank"))
                    .map(|rank| format!("Trending #{}", text(Some(rank))))
                    .unwrap_or_default()
            } else {
                found
            }
        };
        cards.push(SourceCard {
            id: code.clone(),
            title,
            url: format!("{BASE}/title/{}", enc(&code)),
            thumbnail: text(item.get("coverImage")),
            status: text(item.get("status")),
            kind: text(item.get("cardKind")),
            episode_text,
            source: SOURCE_KEY.to_string(),
        });
    }
    cards
}

fn values_of(item: &Value, keys: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for key in keys {
        match item.get(*key) {
            Some(Value::Array(list)) => {
                for value in list {
                    let found = text(Some(value));
                    if !found.is_empty()
                        && !out
                            .iter()
                            .any(|old: &String| old.eq_ignore_ascii_case(&found))
                    {
                        out.push(found);
                    }
                }
            }
            Some(Value::String(value)) => {
                let found = value.trim().to_string();
                if !found.is_empty()
                    && !out
                        .iter()
                        .any(|old: &String| old.eq_ignore_ascii_case(&found))
                {
                    out.push(found);
                }
            }
            Some(Value::Object(map)) => {
                for value in map.values() {
                    match value {
                        Value::Array(list) => {
                            for entry in list {
                                let found = text(Some(entry));
                                if !found.is_empty()
                                    && !out
                                        .iter()
                                        .any(|old: &String| old.eq_ignore_ascii_case(&found))
                                {
                                    out.push(found);
                                }
                            }
                        }
                        _ => {
                            let found = text(Some(value));
                            if !found.is_empty()
                                && !out
                                    .iter()
                                    .any(|old: &String| old.eq_ignore_ascii_case(&found))
                            {
                                out.push(found);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Filters come from the catalog payload; the live site has no separate
/// filter endpoint. Genres/classification are real, order is not exposed.
pub fn parse_filters_json(items: &[Value]) -> CatalogFilters {
    let mut genres: Vec<Genre> = Vec::new();
    let mut statuses: Vec<FilterOption> = Vec::new();
    let mut kinds: Vec<FilterOption> = Vec::new();
    for item in items {
        for label in values_of(item, &["genres", "classification"]) {
            if !genres
                .iter()
                .any(|genre| genre.label.eq_ignore_ascii_case(&label))
            {
                let slug = label.to_ascii_lowercase().replace(' ', "-");
                genres.push(Genre { slug, label });
            }
        }
        let status = text(item.get("status"));
        if !status.is_empty()
            && !statuses
                .iter()
                .any(|option| option.label.eq_ignore_ascii_case(&status))
        {
            let value = status.to_ascii_lowercase().replace(' ', "-");
            statuses.push(FilterOption {
                value,
                label: status,
            });
        }
        let kind = text(item.get("cardKind"));
        if !kind.is_empty()
            && !kinds
                .iter()
                .any(|option| option.label.eq_ignore_ascii_case(&kind))
        {
            kinds.push(FilterOption {
                value: kind.to_ascii_lowercase(),
                label: kind,
            });
        }
    }
    CatalogFilters {
        genres,
        orders: Vec::new(),
        statuses,
        kinds,
    }
}

/// Convert the `/api/watch/<code>/context` payload into the shared series shape.
pub fn parse_series_json(context: &Value, page_url: &str) -> Option<SourceSeries> {
    let data = context.get("data").unwrap_or(context);
    let anime = data.get("anime")?;
    let title = text(anime.get("title"));
    if title.is_empty() {
        return None;
    }
    let id = {
        let found = text(anime.get("animeCode"));
        if found.is_empty() {
            text(anime.get("id"))
        } else {
            found
        }
    };
    let mut episodes = Vec::new();
    let mut seen = HashSet::new();
    if let Some(groups) = data.get("episodeGroups").and_then(Value::as_array) {
        for group in groups {
            let Some(list) = group.get("episodes").and_then(Value::as_array) else {
                continue;
            };
            for item in list {
                let code = text(item.get("episodeCode"));
                if code.is_empty() {
                    continue;
                }
                let url = format!("{BASE}/watch/{}", enc(&code));
                if !seen.insert(url.clone()) {
                    continue;
                }
                let number = item
                    .get("number")
                    .and_then(Value::as_i64)
                    .or_else(|| text(item.get("number")).parse::<i64>().ok());
                let number_text = {
                    let found = text(item.get("numberLabel"));
                    if found.is_empty() {
                        number.map(|value| value.to_string()).unwrap_or_default()
                    } else {
                        found
                    }
                };
                let episode_title = {
                    let found = text(item.get("title"));
                    if found.is_empty() {
                        format!("Episode {number_text}")
                    } else {
                        found
                    }
                };
                episodes.push(SourceEpisode {
                    url,
                    title: episode_title,
                    episode_number: number,
                    number_text,
                    date: text(item.get("publishedAt")),
                });
            }
        }
    }
    episodes.sort_by_key(|episode| (episode.episode_number.unwrap_or(0), episode.url.clone()));
    let classification = anime.get("classification").cloned().unwrap_or_default();
    let studio = values_of(&classification, &["studios", "studio"])
        .first()
        .cloned()
        .unwrap_or_default();
    let released = {
        let found = text(anime.get("releaseDate"));
        if found.is_empty() {
            text(anime.get("year"))
        } else {
            found
        }
    };
    Some(SourceSeries {
        source: SOURCE_KEY.to_string(),
        id: id.clone(),
        title,
        url: if id.is_empty() {
            page_url.to_string()
        } else {
            format!("{BASE}/title/{}", enc(&id))
        },
        thumbnail: text(anime.get("coverImage")),
        description: text(anime.get("description")),
        status: text(anime.get("status")),
        kind: text(anime.get("cardKind")),
        studio,
        network: String::new(),
        released,
        duration: String::new(),
        season: String::new(),
        genres: values_of(&classification, &["genres", "tags"]),
        episode_count: episodes.len(),
        episodes,
        recommendations: Vec::new(),
    })
}

/// Real URL patterns, verified against the live site (2026-11-28):
/// * `/catalog` renders the poster grid, `/api/catalog` returns its JSON;
/// * `/api/search?q=…` returns the same item shape;
/// * `/watch/<episodeCode>` is an episode, `/title/<animeCode>` a series.
pub fn catalog_url(query: &CatalogQuery) -> String {
    let search = query.search.trim();
    if !search.is_empty() {
        return format!("{BASE}/api/search?q={}", enc(search));
    }
    format!("{BASE}/api/catalog")
}

/// Accept a bare anime code, a bare episode code, or any full source URL.
pub fn code_from(raw: &str) -> String {
    let value = raw.trim();
    let Some(compiled) = re(r"(?i)/(?:watch|title|anime)/([^/?#]+)") else {
        return value.to_string();
    };
    compiled
        .captures(value)
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_else(|| value.to_string())
}

pub async fn filters(cache_minutes: u32, refresh: bool) -> Result<CatalogFilters, AppError> {
    let key = "filters";
    if !refresh && let Some(hit) = cache::read::<CatalogFilters>(NAMESPACE, key, cache_minutes) {
        return Ok(hit.value);
    }
    let url = format!("{BASE}/api/catalog");
    let body = fetch_text(&url).await?;
    let payload: Value = serde_json::from_str(&body).map_err(|err| {
        AppError::source_with(
            "animecube.live returned an unreadable catalog for its filters.",
            format!("{url}: {err}"),
        )
    })?;
    let items = payload
        .get("data")
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let parsed = parse_filters_json(&items);
    if parsed.genres.is_empty() && parsed.statuses.is_empty() {
        return Err(AppError::source_with(
            "animecube.live did not expose any filters. The site layout may have changed.",
            url,
        ));
    }
    cache::write(NAMESPACE, key, &parsed, cache_minutes);
    Ok(parsed)
}

pub async fn catalog(
    query: &CatalogQuery,
    cache_minutes: u32,
    refresh: bool,
) -> Result<CatalogPage, AppError> {
    let url = catalog_url(query);
    let search = query.search.trim().to_string();
    let mode = if search.is_empty() {
        "catalog"
    } else {
        "search"
    };
    let page = query.page.max(1);
    let key = format!("{mode}:{page}:{url}");
    if !refresh && let Some(hit) = cache::read::<CatalogPage>(NAMESPACE, &key, cache_minutes) {
        let mut result = hit.value;
        result.cached = true;
        result.age_seconds = hit.age_seconds;
        return Ok(result);
    }

    let body = fetch_text(&url).await?;
    let mut cards: Vec<SourceCard> = match serde_json::from_str::<Value>(&body) {
        Ok(payload) => {
            let items = payload
                .get("data")
                .and_then(|data| data.get("items"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            parse_cards_json(&items)
        }
        Err(_) => parse_cards_html(&body),
    };
    if cards.is_empty() && mode == "catalog" && page == 1 {
        return Err(AppError::source_with(
            "animecube.live did not return any series. The site layout may have changed.",
            url,
        ));
    }
    let size = if query.limit > 0 {
        query.limit
    } else {
        cards.len().max(1)
    };
    let start = page.saturating_sub(1) * size;
    let has_more = start.saturating_add(size) < cards.len();
    let total = cards.len();
    let cards = cards.drain(start..).take(size).collect::<Vec<_>>();
    let result = CatalogPage {
        source: SOURCE_KEY.to_string(),
        mode: mode.to_string(),
        query: search,
        page,
        has_more,
        cards,
        total,
        cached: false,
        age_seconds: 0,
        fetched_at: now_iso8601(),
    };
    cache::write(NAMESPACE, &key, &result, cache_minutes);
    Ok(result)
}

pub async fn series(id: &str, cache_minutes: u32, refresh: bool) -> Result<SourceSeries, AppError> {
    let code = code_from(id);
    if code.is_empty() {
        return Err(AppError::source_with(
            "That is not an animecube.live series id.",
            id.to_string(),
        ));
    }
    let key = format!("series:{code}");
    if !refresh && let Some(hit) = cache::read::<SourceSeries>(NAMESPACE, &key, cache_minutes) {
        return Ok(hit.value);
    }

    // `/title/<code>` is the stable series URL but carries no episode list in
    // static HTML. The watch context does, so try the code as an episode first,
    // then look up the catalog for the series' primary episode code.
    let mut episode_code = code.clone();
    let mut body = fetch_text(&format!("{BASE}/api/watch/{}/context", enc(&episode_code))).await;
    if body.is_err() {
        let catalog_url = format!("{BASE}/api/catalog");
        let catalog_body = fetch_text(&catalog_url).await?;
        let payload: Value = serde_json::from_str(&catalog_body).map_err(|err| {
            AppError::source_with(
                "animecube.live returned an unreadable catalog.",
                format!("{catalog_url}: {err}"),
            )
        })?;
        let items = payload
            .get("data")
            .and_then(|data| data.get("items"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let primary = items
            .iter()
            .find(|item| {
                let found = text(item.get("animeCode"));
                (!found.is_empty() && found == code) || text(item.get("id")) == code
            })
            .map(|item| text(item.get("primaryWatchEpisodeCode")))
            .unwrap_or_default();
        if primary.is_empty() {
            return Err(AppError::source_with(
                "animecube.live did not find that series.",
                id.to_string(),
            ));
        }
        episode_code = primary;
        body = fetch_text(&format!("{BASE}/api/watch/{}/context", enc(&episode_code))).await;
    }
    let context_url = format!("{BASE}/api/watch/{}/context", enc(&episode_code));
    let body = body?;
    let context: Value = serde_json::from_str(&body).map_err(|err| {
        AppError::source_with(
            "animecube.live returned unreadable series details.",
            format!("{context_url}: {err}"),
        )
    })?;
    let parsed =
        parse_series_json(&context, &format!("{BASE}/title/{}", enc(&code))).ok_or_else(|| {
            AppError::source_with(
                "animecube.live did not return series details. The site layout may have changed.",
                context_url,
            )
        })?;
    cache::write(NAMESPACE, &key, &parsed, cache_minutes);
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG_HTML: &str = include_str!("../../../tests/fixtures/animecube_live_catalog.html");
    const CATALOG_JSON: &str = include_str!("../../../tests/fixtures/animecube_live_catalog.json");
    const SEARCH_JSON: &str = include_str!("../../../tests/fixtures/animecube_live_search.json");
    const CONTEXT_JSON: &str = include_str!("../../../tests/fixtures/animecube_live_context.json");

    #[test]
    fn cards_are_read_from_the_live_catalog_grid() {
        let cards = parse_cards_html(CATALOG_HTML);
        assert_eq!(cards.len(), 4);
        let first = &cards[0];
        assert_eq!(first.id, "4p22cq6vyt");
        assert_eq!(first.title, "A Record of a Mortal's Journey to Immortality");
        assert_eq!(first.url, format!("{BASE}/watch/saccrkx"));
        assert!(first.thumbnail.starts_with("https://image.animecube.live/"));
        assert_eq!(first.status, "Airing");
        assert_eq!(first.source, SOURCE_KEY);
    }

    #[test]
    fn json_catalog_and_search_cards_use_stable_title_urls() {
        let payload: Value = serde_json::from_str(CATALOG_JSON).expect("catalog json");
        let items = payload["data"]["items"].as_array().expect("items");
        let cards = parse_cards_json(items);
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].id, "4p22cq6vyt");
        assert_eq!(cards[0].url, format!("{BASE}/title/4p22cq6vyt"));
        assert_eq!(cards[0].source, SOURCE_KEY);

        let payload: Value = serde_json::from_str(SEARCH_JSON).expect("search json");
        let items = payload["data"]["items"].as_array().expect("search items");
        let cards = parse_cards_json(items);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].id, "2jrf22qsfz");
    }

    #[test]
    fn filters_are_harvested_from_catalog_items() {
        let payload: Value = serde_json::from_str(CATALOG_JSON).expect("catalog json");
        let items = payload["data"]["items"].as_array().expect("items");
        let filters = parse_filters_json(items);
        assert!(!filters.genres.is_empty());
        assert!(!filters.statuses.is_empty());
        assert!(filters.orders.is_empty());
    }

    #[test]
    fn a_live_watch_context_yields_series_and_episodes() {
        let context: Value = serde_json::from_str(CONTEXT_JSON).expect("context json");
        let series = parse_series_json(&context, &format!("{BASE}/title/4p22cq6vyt"))
            .expect("series from context");
        assert_eq!(series.source, SOURCE_KEY);
        assert_eq!(series.id, "4p22cq6vyt");
        assert_eq!(
            series.title,
            "A Record of a Mortal's Journey to Immortality"
        );
        assert_eq!(series.url, format!("{BASE}/title/4p22cq6vyt"));
        assert_eq!(series.episodes.len(), 4);
        assert_eq!(series.episodes[0].url, format!("{BASE}/watch/saccrkx"));
        assert_eq!(series.episodes[0].episode_number, Some(1));
    }

    #[test]
    fn real_url_patterns_and_codes() {
        assert_eq!(
            catalog_url(&CatalogQuery::default()),
            format!("{BASE}/api/catalog")
        );
        let query = CatalogQuery {
            search: "renegade immortal".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&query),
            format!("{BASE}/api/search?q=renegade+immortal")
        );
        assert_eq!(code_from("https://animecube.live/watch/ckvqqzn"), "ckvqqzn");
        assert_eq!(
            code_from("https://animecube.live/title/4p22cq6vyt"),
            "4p22cq6vyt"
        );
        assert_eq!(code_from("  4p22cq6vyt "), "4p22cq6vyt");
    }
}
