//! Catalog, filter and series parsing for lmanime.com.
//!
//! Same WordPress theme as animekhor.org, so the parsers are regex based and
//! pure. Captured fixtures from the live site cover cards, series metadata,
//! episodes and the player markup used by the resolver.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use regex::Regex;

use super::{BASE, SOURCE_KEY};
use crate::config::now_iso8601;
use crate::error::AppError;
use crate::sources::browse::{
    CatalogFilters, CatalogPage, Genre, SourceCard, SourceEpisode, SourceSeries,
};
use crate::sources::donghuastream::browse::{CatalogQuery, has_next_page};
use crate::sources::{cache, fetch_text};

const NAMESPACE: &str = "lmanime";

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
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#8211;", "–")
        .replace("&#8216;", "‘")
        .replace("&#8217;", "’")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
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
        return decode_entities(input).trim().to_string();
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

fn slug_from_url(url: &str) -> String {
    let Some(compiled) = re(r"(?i)/([^/?#]+)/?$") else {
        return String::new();
    };
    compiled
        .captures(url.trim_end_matches('/'))
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

/// Canonical series URL from the detail page `<head>`; `/?p=<id>` and search
/// links are not stable library URLs.
fn canonical_url(html: &str) -> String {
    let patterns = [
        r#"(?is)<link\s+rel="canonical"\s+href="([^"]+)""#,
        r#"(?is)<link\s+href="([^"]+)"\s+rel="canonical""#,
    ];
    patterns
        .iter()
        .find_map(|pattern| re(pattern).and_then(|compiled| compiled.captures(html)))
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

/// Poster cards in the `<article class="bs">` archive grid.
pub fn parse_cards(html: &str) -> Vec<SourceCard> {
    let Some(article) = re(r#"(?is)<article\s+class="bs".*?</article>"#) else {
        return Vec::new();
    };
    let Some(link) = re(r#"(?is)<a\s+href="([^"]+)"[^>]*itemprop="url"[^>]*>"#) else {
        return Vec::new();
    };
    let Some(image) = re(r"(?is)<img\b[^>]*>") else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for found in article.captures_iter(html) {
        let block = &found[0];
        let Some(link_tag) = link.captures(block) else {
            continue;
        };
        let href = decode_entities(&link_tag[1]).trim().to_string();
        let slug = slug_from_url(&href);
        if slug.is_empty() {
            continue;
        }
        let url = format!("{BASE}/{slug}/");
        if !seen.insert(url.clone()) {
            continue;
        }
        let title = attr_of(&link_tag[0], "title");
        let title = if title.is_empty() {
            first_text(r#"(?is)<div class="tt">(.*?)<h2"#, block)
        } else {
            title
        };
        if title.is_empty() {
            continue;
        }
        let thumbnail = image
            .captures(block)
            .map(|found| {
                let tag = &found[0];
                let lazy = attr_of(tag, "data-src");
                if lazy.is_empty() {
                    attr_of(tag, "src")
                } else {
                    lazy
                }
            })
            .unwrap_or_default();
        let id = attr_of(&link_tag[0], "rel");
        cards.push(SourceCard {
            id: if id.is_empty() { slug } else { id },
            title,
            url,
            thumbnail,
            status: first_text(r#"(?is)<span class="epx">(.*?)</span>"#, block),
            kind: first_text(r#"(?is)<div class="typez[^"]*">(.*?)</div>"#, block),
            episode_text: first_text(r#"(?is)<span class="epx">(.*?)</span>"#, block),
            source: SOURCE_KEY.to_string(),
        });
    }
    cards
}

/// Episode list from a series page's `eplister` widget.
pub fn parse_episodes(html: &str) -> Vec<SourceEpisode> {
    let Some(list) = re(r#"(?is)<div\s+class="eplister".*?<ul>(.*?)</ul>"#)
        .and_then(|compiled| compiled.captures(html))
        .map(|caps| caps[1].to_string())
    else {
        return Vec::new();
    };
    if list.is_empty() {
        return Vec::new();
    }
    let Some(item) = re(r#"(?is)<li\b[^>]*>\s*<a\s+href="([^"]+)"[^>]*>(.*?)</a>"#) else {
        return Vec::new();
    };
    let Some(number) = re(r#"(?is)<div\s+class="epl-num"[^>]*>(.*?)</div>"#) else {
        return Vec::new();
    };
    let Some(title) = re(r#"(?is)<div\s+class="epl-title"[^>]*>(.*?)</div>"#) else {
        return Vec::new();
    };
    let Some(date) = re(r#"(?is)<div\s+class="epl-date"[^>]*>(.*?)</div>"#) else {
        return Vec::new();
    };
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
    let Some(block) = re(r#"(?is)<div\s+class="spe"[^>]*>(.*?)</div>"#)
        .and_then(|compiled| compiled.captures(html))
        .map(|caps| caps[1].to_string())
    else {
        return HashMap::new();
    };
    let Some(span) = re(r"(?is)<span[^>]*>\s*<b>([^<]+):</b>(.*?)</span>") else {
        return HashMap::new();
    };
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
    let Some(block) = re(r#"(?is)<div\s+class="genxed"[^>]*>(.*?)</div>"#)
        .and_then(|compiled| compiled.captures(html))
        .map(|caps| caps[1].to_string())
    else {
        return Vec::new();
    };
    let Some(link) = re(r"(?is)<a\b[^>]*>(.*?)</a>") else {
        return Vec::new();
    };
    link.captures_iter(&block)
        .map(|caps| text_of(&caps[1]))
        .filter(|value| !value.is_empty())
        .collect()
}

/// One series page into the shared Explore payload.
pub fn parse_series(html: &str, page_url: &str) -> Option<SourceSeries> {
    let title = first_text(r"(?is)<h1[^>]*>(.*?)</h1>", html);
    if title.is_empty() {
        return None;
    }
    let fields = metadata_fields(html);
    let description = first_text(r#"(?is)<div\s+class="desc[^"]*"[^>]*>(.*?)</div>"#, html);
    let thumbnail = [
        r#"(?is)<meta\s+property="og:image"\s+content="([^"]+)""#,
        r#"(?is)<meta\s+content="([^"]+)"\s+property="og:image""#,
    ]
    .iter()
    .find_map(|pattern| re(pattern).and_then(|compiled| compiled.captures(html)))
    .map(|caps| decode_entities(&caps[1]))
    .unwrap_or_default();
    let episodes = parse_episodes(html);
    let get = |key: &str| fields.get(key).cloned().unwrap_or_default();
    let canonical = canonical_url(html);
    let url = if canonical.is_empty() {
        page_url.to_string()
    } else {
        canonical
    };
    let id = slug_from_url(&url);
    Some(SourceSeries {
        source: SOURCE_KEY.to_string(),
        id,
        title,
        url,
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

/// Real pagination patterns verified on the live site:
/// `/anime/page/N/`, `/genres/<slug>/page/N/`, `/page/N/?s=…`.
pub fn catalog_url(query: &CatalogQuery) -> String {
    let page = query.page.max(1);
    let search = query.search.trim();
    if !search.is_empty() {
        if page > 1 {
            return format!("{BASE}/page/{page}/?s={}", enc(search));
        }
        return format!("{BASE}/?s={}", enc(search));
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
    if page > 1 {
        format!("{BASE}/anime/page/{page}/")
    } else {
        format!("{BASE}/anime/?status=&type=&order=update")
    }
}

pub fn series_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        let slug = slug_from_url(value);
        if slug.is_empty() {
            return None;
        }
        return Some(format!("{BASE}/{slug}/"));
    }
    let id = value.trim_matches('/');
    if id.is_empty() || id.contains('/') || id.contains('?') || id.contains('#') || id.contains(' ')
    {
        return None;
    }
    Some(format!("{BASE}/{id}/"))
}

pub fn parse_filters(html: &str) -> CatalogFilters {
    let Some(link) = re(r#"(?is)<a\s+href="[^"]*/genres/([^/"']+)/"[^>]*>(.*?)</a>"#) else {
        return CatalogFilters {
            genres: Vec::new(),
            orders: Vec::new(),
            statuses: Vec::new(),
            kinds: Vec::new(),
        };
    };
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
    let url = format!("{BASE}/genres/");
    let html = fetch_text(&url).await?;
    let parsed = parse_filters(&html);
    if parsed.genres.is_empty() {
        return Err(AppError::source_with(
            "lmanime.com did not list any genres. The site layout may have changed.",
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

    let html = fetch_text(&url).await?;
    let mut cards = parse_cards(&html);
    if cards.is_empty() && mode == "catalog" && page == 1 {
        return Err(AppError::source_with(
            "lmanime.com's catalog did not contain any series. The site layout may have changed.",
            url,
        ));
    }
    let has_more = !cards.is_empty() && has_next_page(&html);
    if query.limit > 0 {
        cards.truncate(query.limit);
    }
    let result = CatalogPage {
        source: SOURCE_KEY.to_string(),
        mode: mode.to_string(),
        query: search,
        page,
        has_more,
        total: cards.len(),
        cards,
        cached: false,
        age_seconds: 0,
        fetched_at: now_iso8601(),
    };
    cache::write(NAMESPACE, &key, &result, cache_minutes);
    Ok(result)
}

pub async fn series(id: &str, cache_minutes: u32, refresh: bool) -> Result<SourceSeries, AppError> {
    let url = series_url(id).ok_or_else(|| {
        AppError::source_with("That is not an lmanime.com series id.", id.to_string())
    })?;
    let key = format!("series:{url}");
    if !refresh && let Some(hit) = cache::read::<SourceSeries>(NAMESPACE, &key, cache_minutes) {
        return Ok(hit.value);
    }
    let html = fetch_text(&url).await?;
    let parsed = parse_series(&html, &url).ok_or_else(|| {
        AppError::source_with(
            "lmanime.com did not return series details. The site layout may have changed.",
            url.clone(),
        )
    })?;
    cache::write(NAMESPACE, &key, &parsed, cache_minutes);
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = include_str!("../../../tests/fixtures/lmanime_catalog.html");
    const SERIES: &str = include_str!("../../../tests/fixtures/lmanime_series.html");

    #[test]
    fn cards_are_read_from_the_live_archive_grid() {
        let cards = parse_cards(CATALOG);
        assert_eq!(cards.len(), 4);
        assert_eq!(cards[0].id, "14548");
        assert_eq!(cards[0].title, "Tales of Herding Gods");
        assert_eq!(cards[0].url, format!("{BASE}/tales-of-herding-gods/"));
        assert_eq!(cards[0].kind, "ONA");
        assert_eq!(cards[0].status, "Ongoing");
        assert!(
            cards[0]
                .thumbnail
                .starts_with("https://lmanime.com/wp-content/uploads/")
        );
        assert_eq!(cards[0].source, SOURCE_KEY);
    }

    #[test]
    fn series_metadata_genres_and_episodes_are_read() {
        let series = parse_series(
            SERIES,
            &format!("{BASE}/a-record-of-a-mortals-journey-to-immortality/"),
        )
        .expect("series from fixture");
        assert_eq!(
            series.title,
            "A Record of A Mortal’s Journey to Immortality"
        );
        assert_eq!(series.id, "a-record-of-a-mortals-journey-to-immortality");
        assert_eq!(series.status, "Completed");
        assert_eq!(series.kind, "TV");
        assert_eq!(series.released, "2020");
        assert!(series.genres.iter().any(|genre| genre == "Action"));
        assert_eq!(series.episode_count, 3);
        let first = &series.episodes[0];
        assert_eq!(first.episode_number, Some(17));
        assert_eq!(first.title, "A Mortal’s Journey To Immortality Episode 17");
        assert_eq!(first.date, "May 23, 2023");
    }

    #[test]
    fn episode_lists_are_optional() {
        assert!(parse_episodes("<h1>No episodes</h1>").is_empty());
    }

    #[test]
    fn catalog_urls_use_the_real_pagination_patterns() {
        assert_eq!(
            catalog_url(&CatalogQuery::default()),
            format!("{BASE}/anime/?status=&type=&order=update")
        );
        let paged = CatalogQuery {
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&paged), format!("{BASE}/anime/page/2/"));
        let genre = CatalogQuery {
            genre: "action".to_string(),
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&genre), format!("{BASE}/genres/action/page/2/"));
        let search = CatalogQuery {
            search: "renegade immortal".to_string(),
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&search),
            format!("{BASE}/page/2/?s=renegade+immortal")
        );
        let injected = CatalogQuery {
            genre: "../etc".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&injected),
            format!("{BASE}/anime/?status=&type=&order=update")
        );
    }

    #[test]
    fn bare_slugs_and_urls_build_stable_series_urls() {
        assert_eq!(
            series_url("a-record-of-a-mortals-journey-to-immortality"),
            Some(format!(
                "{BASE}/a-record-of-a-mortals-journey-to-immortality/"
            ))
        );
        assert_eq!(
            series_url("https://lmanime.com/tales-of-herding-gods/"),
            Some(format!("{BASE}/tales-of-herding-gods/"))
        );
        assert_eq!(series_url("../etc"), None);
        assert_eq!(series_url(""), None);
    }
}
