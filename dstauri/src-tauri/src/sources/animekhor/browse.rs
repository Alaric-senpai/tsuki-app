//! Catalog, filter and series parsing for animekhor.org.
//!
//! The site is a WordPress install, so the payload is ordinary HTML rather
//! than the Next.js RSC streams used by AnimeCube. Pure parsers live here and
//! never touch the network; async wrappers use the shared source cache.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::Regex;

use super::{BASE, SOURCE_KEY};
use crate::config::now_iso8601;
use crate::error::AppError;
use crate::sources::browse::{
    CatalogFilters, CatalogPage, Genre, SourceCard, SourceEpisode, SourceSeries,
};
use crate::sources::donghuastream::browse::CatalogQuery;
use crate::sources::{cache, fetch_text};

const NAMESPACE: &str = "animekhor";

/// Cached compiled regexes. The parsers below are pure functions; compiling a
/// pattern once per process keeps repeated catalog pages cheap.
fn re(pattern: &str) -> Regex {
    static CACHE: OnceLock<Mutex<HashMap<String, Regex>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = map.lock() {
        if let Some(found) = map.get(pattern) {
            return found.clone();
        }
        let compiled = Regex::new(pattern).expect("valid animekhor parser regex");
        map.insert(pattern.to_string(), compiled.clone());
        return compiled;
    }
    Regex::new(pattern).expect("valid animekhor parser regex")
}

fn decode_entities(input: &str) -> String {
    let mut value = input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#8211;", "–")
        .replace("&#8216;", "‘")
        .replace("&#8217;", "’")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    // Numeric entities other than the common quotes above.
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

/// Strip markup and normalise whitespace for visible text fields.
fn text_of(input: &str) -> String {
    let without_tags = re(r"(?is)<[^>]+>").replace_all(input, " ").to_string();
    let decoded = decode_entities(&without_tags);
    re(r"\s+").replace_all(decoded.trim(), " ").to_string()
}

fn attr_of(fragment: &str, name: &str) -> String {
    let pattern = format!(r#"(?is)\b{name}\s*=\s*"([^"]*)""#);
    re(&pattern)
        .captures(fragment)
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

fn enc(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.trim().as_bytes()).collect()
}

fn slug_from_url(url: &str) -> String {
    re(r"(?i)/anime/([^/?#]+)/?")
        .captures(url)
        .map(|caps| caps[1].to_string())
        .unwrap_or_default()
}

/// Read poster cards from a catalog/search page.
pub fn parse_cards(html: &str) -> Vec<SourceCard> {
    let pattern = re(r#"(?is)<a\s+class="series"\s+href="([^"]+)"\s+rel="(\d+)"[^>]*>(.*?)</a>"#);
    let image_pattern = re(r#"(?is)<img\b[^>]*>"#);
    let mut cards = Vec::new();
    for caps in pattern.captures_iter(html) {
        let url = decode_entities(&caps[1]).trim().to_string();
        let id = caps[2].to_string();
        let slug = slug_from_url(&url);
        if slug.is_empty() {
            continue;
        }
        let inner = &caps[3];
        let primary = attr_of(inner, "title");
        let alternate = if primary.is_empty() {
            attr_of(inner, "alt")
        } else {
            primary
        };
        let title = if alternate.is_empty() {
            text_of(inner)
        } else {
            alternate
        };
        if title.is_empty() {
            continue;
        }
        let thumbnail = image_pattern
            .captures(inner)
            .map(|image| {
                let tag = &image[0];
                let lazy = attr_of(tag, "data-src");
                if lazy.is_empty() {
                    attr_of(tag, "src")
                } else {
                    lazy
                }
            })
            .unwrap_or_default();
        cards.push(SourceCard {
            id,
            title,
            url,
            thumbnail,
            status: String::new(),
            kind: String::new(),
            episode_text: String::new(),
            source: SOURCE_KEY.to_string(),
        });
    }
    cards
}

/// Read the episode list from a series page.
pub fn parse_episodes(html: &str) -> Vec<SourceEpisode> {
    let list = re(r#"(?is)<div\s+class="eplister".*?<ul>(.*?)</ul>"#)
        .captures(html)
        .map(|caps| caps[1].to_string())
        .unwrap_or_default();
    if list.is_empty() {
        return Vec::new();
    }
    let item = re(r#"(?is)<li\b[^>]*>\s*<a\s+href="([^"]+)"[^>]*>(.*?)</a>"#);
    let number = re(r#"(?is)<div\s+class="epl-num"[^>]*>(.*?)</div>"#);
    let title = re(r#"(?is)<div\s+class="epl-title"[^>]*>(.*?)</div>"#);
    let date = re(r#"(?is)<div\s+class="epl-date"[^>]*>(.*?)</div>"#);
    let mut episodes = Vec::new();
    for caps in item.captures_iter(&list) {
        let url = decode_entities(&caps[1]).trim().to_string();
        if url.is_empty() {
            continue;
        }
        let number_text = number
            .captures(&caps[2])
            .map(|found| text_of(&found[1]))
            .unwrap_or_default();
        let title_text = title
            .captures(&caps[2])
            .map(|found| text_of(&found[1]))
            .unwrap_or_else(|| format!("Episode {number_text}"));
        let date_text = date
            .captures(&caps[2])
            .map(|found| text_of(&found[1]))
            .unwrap_or_default();
        episodes.push(SourceEpisode {
            url,
            title: title_text,
            episode_number: number_text
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<i64>()
                .ok(),
            number_text,
            date: date_text,
        });
    }
    episodes
}

fn metadata_fields(html: &str) -> HashMap<String, String> {
    let block = re(r#"(?is)<div\s+class="spe"[^>]*>(.*?)</div>"#)
        .captures(html)
        .map(|caps| caps[1].to_string())
        .unwrap_or_default();
    let span = re(r"(?is)<span[^>]*>\s*<b>([^<]+):</b>(.*?)</span>");
    let mut fields = HashMap::new();
    for caps in span.captures_iter(&block) {
        fields.insert(
            text_of(&caps[1]).trim_end_matches(':').to_lowercase(),
            text_of(&caps[2]),
        );
    }
    fields
}

fn parse_genres(html: &str) -> Vec<String> {
    let block = re(r#"(?is)<div\s+class="genxed"[^>]*>(.*?)</div>"#)
        .captures(html)
        .map(|caps| caps[1].to_string())
        .unwrap_or_default();
    let link = re(r#"(?is)<a\b[^>]*>(.*?)</a>"#);
    link.captures_iter(&block)
        .map(|caps| text_of(&caps[1]))
        .filter(|value| !value.is_empty())
        .collect()
}

/// Read one series page into the shared Explore payload.
pub fn parse_series(html: &str, page_url: &str) -> Option<SourceSeries> {
    let title = re(r#"(?is)<h1\s+class="entry-title"[^>]*>(.*?)</h1>"#)
        .captures(html)
        .map(|caps| text_of(&caps[1]))
        .unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    let fields = metadata_fields(html);
    let description = re(r#"(?is)<div\s+class="desc[^"]*"[^>]*>(.*?)</div>"#)
        .captures(html)
        .map(|caps| text_of(&caps[1]))
        .unwrap_or_default();
    let thumbnail = [
        r#"(?is)<meta\s+property="og:image"\s+content="([^"]+)""#,
        r#"(?is)<meta\s+content="([^"]+)"\s+property="og:image""#,
    ]
    .iter()
    .find_map(|pattern| re(pattern).captures(html))
    .map(|caps| decode_entities(&caps[1]))
    .unwrap_or_default();
    let episodes = parse_episodes(html);
    let get = |key: &str| fields.get(key).cloned().unwrap_or_default();

    Some(SourceSeries {
        source: SOURCE_KEY.to_string(),
        id: slug_from_url(page_url),
        title,
        url: page_url.to_string(),
        thumbnail,
        description,
        status: get("status"),
        kind: get("type"),
        studio: get("studio"),
        network: get("network"),
        released: get("released"),
        duration: get("duration"),
        season: String::new(),
        genres: parse_genres(html),
        episode_count: episodes.len(),
        episodes,
        recommendations: Vec::new(),
    })
}

pub fn catalog_url(query: &CatalogQuery) -> String {
    let page = query.page.max(1);
    let prefix = if page > 1 {
        format!("{BASE}/page/{page}/")
    } else {
        format!("{BASE}/")
    };
    let search = query.search.trim();
    if !search.is_empty() {
        return format!("{prefix}?s={}", enc(search));
    }
    let genre = query.genre.trim().trim_matches('/');
    if !genre.is_empty()
        && genre
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        if page > 1 {
            return format!("{BASE}/genres/{genre}/page/{page}/");
        }
        return format!("{BASE}/genres/{genre}/");
    }
    prefix.trim_end_matches('/').to_string() + "/"
}

fn has_next_page(html: &str) -> bool {
    re(r#"(?is)<a[^>]+(?:rel=["']next["']|class=["'][^"']*\bnext page-numbers\b)"#).is_match(html)
}

/// Genres are harvested from the site's `/genres/` links; it exposes no
/// dedicated filter endpoint, so orders/statuses/kinds stay empty.
pub fn parse_filters(html: &str) -> CatalogFilters {
    let link = re(r#"(?is)<a\s+href="[^"]*/genres/([^/"']+)/"[^>]*>(.*?)</a>"#);
    let mut genres = Vec::new();
    for caps in link.captures_iter(html) {
        let slug = decode_entities(&caps[1]);
        let label = text_of(&caps[2]);
        if slug.is_empty() || label.is_empty() {
            continue;
        }
        if !genres.iter().any(|item: &Genre| item.slug == slug) {
            genres.push(Genre { slug, label });
        }
    }
    CatalogFilters {
        genres,
        orders: Vec::new(),
        statuses: Vec::new(),
        kinds: Vec::new(),
    }
}

pub async fn filters(cache_minutes: u32, refresh: bool) -> Result<CatalogFilters, AppError> {
    let key = "filters";
    if !refresh && let Some(hit) = cache::read::<CatalogFilters>(NAMESPACE, key, cache_minutes) {
        return Ok(hit.value);
    }
    let html = fetch_text(&format!("{BASE}/")).await?;
    let parsed = parse_filters(&html);
    if parsed.genres.is_empty() {
        return Err(AppError::source_with(
            "animekhor.org did not list any genres. The site layout may have changed.",
            BASE.to_string(),
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
    let key = format!("{mode}:{url}");
    if !refresh && let Some(hit) = cache::read::<CatalogPage>(NAMESPACE, &key, cache_minutes) {
        let mut page = hit.value;
        page.cached = true;
        page.age_seconds = hit.age_seconds;
        return Ok(page);
    }

    let html = fetch_text(&url).await?;
    let mut cards = parse_cards(&html);
    let page_number = query.page.max(1);
    if cards.is_empty() && mode == "catalog" && page_number == 1 {
        return Err(AppError::source_with(
            "animekhor.org's catalog did not contain any series. The site layout may have changed.",
            url,
        ));
    }
    let raw_count = cards.len();
    let has_more = has_next_page(&html) || (query.limit > 0 && raw_count >= query.limit);
    if query.limit > 0 {
        cards.truncate(query.limit);
    }
    let page = CatalogPage {
        source: SOURCE_KEY.to_string(),
        mode: mode.to_string(),
        query: search,
        page: page_number,
        has_more,
        total: cards.len(),
        cards,
        cached: false,
        age_seconds: 0,
        fetched_at: now_iso8601(),
    };
    cache::write(NAMESPACE, &key, &page, cache_minutes);
    Ok(page)
}

pub async fn series(id: &str, cache_minutes: u32, refresh: bool) -> Result<SourceSeries, AppError> {
    let slug = id
        .trim()
        .trim_start_matches(format!("{BASE}/anime/").as_str())
        .trim_matches('/')
        .to_string();
    if slug.is_empty() {
        return Err(AppError::source_with(
            "That is not an animekhor.org series id.",
            id.to_string(),
        ));
    }
    let url = format!("{BASE}/anime/{slug}/");
    let key = format!("series:{url}");
    if !refresh && let Some(hit) = cache::read::<SourceSeries>(NAMESPACE, &key, cache_minutes) {
        return Ok(hit.value);
    }
    let html = fetch_text(&url).await?;
    let parsed = parse_series(&html, &url).ok_or_else(|| {
        AppError::source_with(
            "animekhor.org did not return series details. The site layout may have changed.",
            url.clone(),
        )
    })?;
    cache::write(NAMESPACE, &key, &parsed, cache_minutes);
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = include_str!("fixtures/home.html");
    const DETAIL: &str = include_str!("fixtures/detail.html");

    #[test]
    fn cards_are_read_from_the_live_catalog_fixture() {
        let cards = parse_cards(HOME);
        let card = cards
            .iter()
            .find(|item| item.url.contains("renegade-immortal"))
            .expect("catalog card");
        assert_eq!(card.title, "Renegade Immortal");
        assert_eq!(card.id, "28305");
        assert!(card.thumbnail.contains("Renegade-Immortal-Poster-l.jpg"));
        assert_eq!(card.source, SOURCE_KEY);
    }

    #[test]
    fn series_metadata_and_genres_are_read_from_the_live_fixture() {
        let series =
            parse_series(DETAIL, "https://animekhor.org/anime/renegade-immortal/").expect("series");
        assert_eq!(series.status, "Ongoing");
        assert_eq!(series.kind, "ONA");
        assert!(series.genres.iter().any(|genre| genre == "Action"));
        assert!(series.episodes.len() > 100);
    }

    #[test]
    fn episodes_are_read_with_numbers_titles_and_dates() {
        let episodes = parse_episodes(DETAIL);
        let first = episodes.first().expect("first episode");
        assert_eq!(first.episode_number, Some(168));
        assert_eq!(first.title, "Episode 168");
        assert_eq!(first.date, "November 22, 2026");
        assert!(first.url.contains("/renegade-immortal-episode-168-"));
    }
}
