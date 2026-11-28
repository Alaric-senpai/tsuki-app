//! Catalog, filter and series parsing for dongsub.net.
//!
//! The site is a Blogger install. Its A-Z list / "Jadwal Rilis" widget links
//! stable series posts, `/search?q=` and `/search/label/…` list episode posts,
//! and `/feeds/posts/default/-/<label>?alt=json` returns the full episode list
//! for a series. Pure regex/JSON parsers live here; all I/O uses `fetch_text`.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use regex::Regex;
use serde_json::Value;

use super::{BASE, SOURCE_KEY};
use crate::config::now_iso8601;
use crate::error::AppError;
use crate::sources::browse::{
    CatalogFilters, CatalogPage, Genre, SourceCard, SourceEpisode, SourceSeries,
};
use crate::sources::donghuastream::browse::CatalogQuery;
use crate::sources::{cache, fetch_text};

const NAMESPACE: &str = "dongsub";

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
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
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
        return decode_entities(input).trim().to_string();
    };
    let without_tags = tags.replace_all(input, " ");
    let decoded = decode_entities(&without_tags);
    let Some(ws) = re(r"\s+") else {
        return decoded.trim().to_string();
    };
    ws.replace_all(decoded.trim(), " ").to_string()
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

fn attr_of(fragment: &str, name: &str) -> String {
    let pattern = format!(r#"(?i)\b{name}\s*=\s*['"]([^'"]*)['"]"#);
    let Some(compiled) = re(&pattern) else {
        return String::new();
    };
    compiled
        .captures(fragment)
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

fn slug_from_url(url: &str) -> String {
    let Some(compiled) = re(r"(?i)/(\d{4}/\d{2}/([^/?#]+?))\.html") else {
        return String::new();
    };
    compiled
        .captures(url)
        .map(|caps| caps[2].to_string())
        .unwrap_or_default()
}

fn post_url(raw: &str) -> String {
    let trimmed = decode_entities(raw).trim().to_string();
    let Some(compiled) = re(r"(?i)^https?://(?:www\.)?dongsub\.net(/.*)$") else {
        return trimmed;
    };
    compiled
        .captures(&trimmed)
        .map(|caps| format!("{BASE}{}", &caps[1]))
        .unwrap_or(trimmed)
}

/// Series links from the A-Z list / "Jadwal Rilis" widget. The arrow prefix
/// (`&#10157;`) is not part of the title.
pub fn parse_series_cards(html: &str) -> Vec<SourceCard> {
    let Some(anchor) = re(
        r#"(?is)<a\s+href=['"](https?://(?:www\.)?dongsub\.net/\d{4}/\d{2}/[^'"]+\.html)['"][^>]*>(.*?)</a>"#,
    ) else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for caps in anchor.captures_iter(html) {
        // Blogger's schedule widget prefixes titles with `&#10157;` (➭).
        let title = text_of(&caps[2])
            .trim_start_matches(|ch: char| !ch.is_alphanumeric())
            .trim()
            .to_string();
        if title.is_empty() || title.starts_with("Continue reading") {
            continue;
        }
        let url = post_url(&caps[1]);
        let id = slug_from_url(&url);
        if id.is_empty() || !seen.insert(url.clone()) {
            continue;
        }
        cards.push(SourceCard {
            id,
            title,
            url,
            thumbnail: String::new(),
            status: String::new(),
            kind: String::new(),
            episode_text: String::new(),
            source: SOURCE_KEY.to_string(),
        });
    }
    cards
}

/// Episode cards from Blogger's search/label post list.
pub fn parse_post_cards(html: &str) -> Vec<SourceCard> {
    let Some(article) = re(r#"(?is)<article\s+class=['"]post['"][^>]*>.*?</article>"#) else {
        return Vec::new();
    };
    let Some(title_link) = re(
        r#"(?is)<h3\s+class=['\"]post-title['\"][^>]*>\s*<a\s+href=['"]([^'"]+)['"][^>]*>(.*?)</a>"#,
    ) else {
        return Vec::new();
    };
    let Some(image) = re(r"(?is)<img\b[^>]*>") else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for found in article.captures_iter(html) {
        let block = &found[0];
        let Some(link) = title_link.captures(block) else {
            continue;
        };
        let title = text_of(&link[2]);
        if title.is_empty() {
            continue;
        }
        let url = post_url(&link[1]);
        if !seen.insert(url.clone()) {
            continue;
        }
        let thumbnail = image
            .captures(block)
            .map(|found| {
                let lazy = attr_of(&found[0], "data-src");
                if lazy.is_empty() {
                    attr_of(&found[0], "src")
                } else {
                    lazy
                }
            })
            .unwrap_or_default();
        let (number_text, _) = episode_number(&title);
        cards.push(SourceCard {
            id: slug_from_url(&url),
            title,
            url,
            thumbnail,
            status: String::new(),
            kind: String::new(),
            episode_text: number_text,
            source: SOURCE_KEY.to_string(),
        });
    }
    cards
}

/// `(raw label, numeric value)` from a post title like
/// `Renegade Immortal Episode 168 Subtitles`.
fn episode_number(title: &str) -> (String, Option<i64>) {
    let Some(compiled) = re(r"(?i)episode[\s_-]*(\d+(?:\s*(?:to|-)\s*\d+)?)") else {
        return (String::new(), None);
    };
    let Some(caps) = compiled.captures(title) else {
        return (String::new(), None);
    };
    let label = caps[1].to_string();
    let number = label
        .split(|ch: char| !ch.is_ascii_digit())
        .next()
        .unwrap_or_default()
        .parse::<i64>()
        .ok();
    (label, number)
}

/// The label used by the series page's episode loader.
pub fn parse_label(html: &str) -> String {
    let Some(compiled) = re(r#"(?is)var\s+label_episode\s*=\s*["']([^"']+)["']"#) else {
        return String::new();
    };
    compiled
        .captures(html)
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

/// Episodes from a Blogger label page's static post list (fallback parser).
pub fn parse_episodes_html(html: &str) -> Vec<SourceEpisode> {
    let Some(article) = re(r#"(?is)<article\s+class=['"]post['"][^>]*>.*?</article>"#) else {
        return Vec::new();
    };
    let Some(link) = re(
        r#"(?is)<h3\s+class=['\"]post-title['\"][^>]*>\s*<a\s+href=['"]([^'"]+)['"][^>]*>(.*?)</a>"#,
    ) else {
        return Vec::new();
    };
    let Some(date) = re(r#"(?is)<time[^>]*datetime=['"]([^'"]+)['"]"#) else {
        return Vec::new();
    };
    let mut episodes = Vec::new();
    let mut seen = HashSet::new();
    for found in article.captures_iter(html) {
        let block = &found[0];
        let Some(caps) = link.captures(block) else {
            continue;
        };
        let url = post_url(&caps[1]);
        if !seen.insert(url.clone()) {
            continue;
        }
        let title = text_of(&caps[2]);
        let (number_text, number) = episode_number(&title);
        let published = date
            .captures(block)
            .map(|found| found[1].to_string())
            .unwrap_or_default();
        episodes.push(SourceEpisode {
            url,
            title,
            episode_number: number,
            number_text,
            date: published,
        });
    }
    episodes
}

/// Episodes from Blogger's JSON feed. The HTML label page only shows the most
/// recent posts, the feed returns the whole series with `start-index` paging.
pub fn parse_feed_entries(feed: &Value) -> Vec<SourceEpisode> {
    let Some(entries) = feed
        .get("feed")
        .and_then(|value| value.get("entry"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut episodes = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let url = entry
            .get("link")
            .and_then(Value::as_array)
            .and_then(|links| {
                links.iter().find(|link| {
                    link.get("rel").and_then(Value::as_str).unwrap_or_default() == "alternate"
                })
            })
            .and_then(|link| link.get("href"))
            .and_then(Value::as_str)
            .map(post_url)
            .unwrap_or_default();
        if url.is_empty() || !seen.insert(url.clone()) {
            continue;
        }
        let title = entry
            .get("title")
            .and_then(|value| value.get("$t"))
            .and_then(Value::as_str)
            .map(text_of)
            .unwrap_or_default();
        let (number_text, number) = episode_number(&title);
        episodes.push(SourceEpisode {
            url,
            title,
            episode_number: number,
            number_text,
            date: entry
                .get("published")
                .and_then(|value| value.get("$t"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    episodes
}

/// Series metadata from one of the static `/YYYY/MM/<slug>.html` posts.
pub fn parse_series(html: &str, page_url: &str) -> Option<SourceSeries> {
    let title = first_text(r#"(?is)<h1\s+class=['"]post-title[^>]*>(.*?)</h1>"#, html);
    let title = if title.is_empty() {
        first_text(r"(?is)<title[^>]*>(.*?)</title>", html)
            .split(" - ")
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        title
    };
    if title.is_empty() {
        return None;
    }
    let body = re(r#"(?is)<div\s+class=['"]post-body[^>]*>(.*?)<span>\s*<a\s+name=['"]more"#)
        .map(|compiled| {
            compiled
                .captures(html)
                .map(|caps| caps[1].to_string())
                .unwrap_or_else(|| html.to_string())
        })
        .unwrap_or_else(|| html.to_string());
    let thumbnail = {
        let pattern = format!(r#"(?is)<img\b[^>]*\bsrc=['"]([^'"]+)['"]"#);
        re(&pattern)
            .and_then(|compiled| compiled.captures(&body))
            .map(|caps| decode_entities(&caps[1]))
            .unwrap_or_default()
    };
    let description = first_text(
        r#"(?is)<div\s+class=['"]sinoposis['"][^>]*>(.*?)</div>"#,
        &body,
    );
    let id = slug_from_url(page_url);
    Some(SourceSeries {
        source: SOURCE_KEY.to_string(),
        id,
        title,
        url: page_url.to_string(),
        thumbnail,
        description,
        status: String::new(),
        kind: String::new(),
        studio: String::new(),
        network: String::new(),
        released: String::new(),
        duration: String::new(),
        season: String::new(),
        genres: Vec::new(),
        episode_count: 0,
        episodes: Vec::new(),
        recommendations: Vec::new(),
    })
}

/// Genre chips from the home page's `/search/label/…` links, ignoring the
/// categories that are not genres (Episode, Erotica, Hentai…).
pub fn parse_filters(html: &str) -> CatalogFilters {
    let Some(link) = re(r#"(?is)<a\s+href=['"][^'"]*/search/label/([^?'"/]+)['"][^>]*>(.*?)</a>"#)
    else {
        return CatalogFilters {
            genres: Vec::new(),
            orders: Vec::new(),
            statuses: Vec::new(),
            kinds: Vec::new(),
        };
    };
    let ignore = ["episode", "erotica", "hentai", "ecchi"];
    let mut genres = Vec::new();
    for caps in link.captures_iter(html) {
        let label = text_of(&caps[2]);
        let slug = decode_entities(&caps[1]);
        if label.is_empty()
            || slug.is_empty()
            || ignore.iter().any(|word| label.eq_ignore_ascii_case(word))
        {
            continue;
        }
        if !genres
            .iter()
            .any(|genre: &Genre| genre.label.eq_ignore_ascii_case(&label))
        {
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

/// Real URL patterns, verified against the live site:
/// * `/p/az-list.html` lists stable series posts;
/// * `/search?q=…` and `/search/label/<label>?max-results=N` list episodes;
/// * `/feeds/posts/default/-/<label>?alt=json` returns a series' episodes.
pub fn catalog_url(query: &CatalogQuery) -> String {
    let page = query.page.max(1);
    let search = query.search.trim();
    if !search.is_empty() {
        return format!("{BASE}/search?q={}", enc(search));
    }
    let genre = query.genre.trim().trim_matches('/');
    if !genre.is_empty() {
        let start = page.saturating_sub(1) * 10;
        return format!(
            "{BASE}/search/label/{}?&max-results=10&start-index={}",
            enc(genre),
            start + 1
        );
    }
    format!("{BASE}/p/az-list.html")
}

pub fn feed_url(label: &str) -> String {
    format!(
        "{BASE}/feeds/posts/default/-/{}?alt=json&max-results=50",
        enc(label)
    )
}

fn page_slice<T: Clone>(items: &[T], page: usize, limit: usize) -> Vec<T> {
    let size = if limit > 0 { limit } else { items.len().max(1) };
    let start = page.saturating_sub(1) * size;
    items.iter().skip(start).take(size).cloned().collect()
}

fn has_more<T>(items: &[T], page: usize, limit: usize) -> bool {
    let size = if limit > 0 { limit } else { items.len().max(1) };
    page.saturating_sub(1) * size + size < items.len()
}

pub async fn filters(cache_minutes: u32, refresh: bool) -> Result<CatalogFilters, AppError> {
    let key = "filters";
    if !refresh && let Some(hit) = cache::read::<CatalogFilters>(NAMESPACE, key, cache_minutes) {
        return Ok(hit.value);
    }
    let url = format!("{BASE}/");
    let html = fetch_text(&url).await?;
    let parsed = parse_filters(&html);
    if parsed.genres.is_empty() {
        return Err(AppError::source_with(
            "dongsub.net did not list any genres. The site layout may have changed.",
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
    let mut cards = if mode == "catalog" && query.genre.trim().is_empty() {
        parse_series_cards(&html)
    } else {
        parse_post_cards(&html)
    };
    if cards.is_empty() {
        // A layout change must not silently look like "no results".
        cards = parse_series_cards(&html);
    }
    if cards.is_empty() && mode == "catalog" && page == 1 {
        return Err(AppError::source_with(
            "dongsub.net did not return any series. The site layout may have changed.",
            url,
        ));
    }
    let total = cards.len();
    let has_more = has_more(&cards, page, query.limit);
    let cards = page_slice(&cards, page, query.limit);
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

/// `id` may be a full series URL, a bare slug (`renegade-immortal`) or a
/// `label:Renegade Immortal` selector. The A-Z list maps slugs to real URLs
/// because the `/YYYY/MM/` part varies per series.
pub async fn series(id: &str, cache_minutes: u32, refresh: bool) -> Result<SourceSeries, AppError> {
    let raw = id.trim().trim_start_matches("label:");
    if raw.is_empty() {
        return Err(AppError::source_with(
            "That is not a dongsub.net series id.",
            id.to_string(),
        ));
    }
    let key = format!("series:{raw}");
    if !refresh && let Some(hit) = cache::read::<SourceSeries>(NAMESPACE, &key, cache_minutes) {
        return Ok(hit.value);
    }

    let mut label = String::new();
    let page_url = if raw.starts_with("http://") || raw.starts_with("https://") {
        post_url(raw)
    } else if raw.contains(' ') {
        label = raw.to_string();
        format!("{BASE}/p/az-list.html")
    } else {
        let list_url = format!("{BASE}/p/az-list.html");
        let html = fetch_text(&list_url).await?;
        let cards = parse_series_cards(&html);
        cards
            .iter()
            .find(|card| card.id == raw)
            .map(|card| card.url.clone())
            .unwrap_or_else(|| {
                label = raw.replace('-', " ");
                list_url.clone()
            })
    };

    let html = fetch_text(&page_url).await?;
    let mut parsed = parse_series(&html, &page_url).ok_or_else(|| {
        AppError::source_with(
            "dongsub.net did not return series details. The site layout may have changed.",
            page_url.clone(),
        )
    })?;
    if label.is_empty() {
        label = parse_label(&html);
    }
    if label.is_empty() {
        label = parsed.title.clone();
    }
    let url = feed_url(&label);
    let body = fetch_text(&url).await?;
    let feed: Value = serde_json::from_str(&body).map_err(|err| {
        AppError::source_with(
            "dongsub.net returned an unreadable episode list.",
            format!("{url}: {err}"),
        )
    })?;
    parsed.episodes = parse_feed_entries(&feed);
    parsed.episode_count = parsed.episodes.len();
    if label.contains(' ') {
        parsed.genres = label
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .take(4)
            .map(|word| word.to_string())
            .collect();
    }
    cache::write(NAMESPACE, &key, &parsed, cache_minutes);
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = include_str!("../../../tests/fixtures/dongsub_series_list.html");
    const POSTS: &str = include_str!("../../../tests/fixtures/dongsub_posts.html");
    const SERIES: &str = include_str!("../../../tests/fixtures/dongsub_series_post.html");
    const FEED: &str = include_str!("../../../tests/fixtures/dongsub_feed.json");

    #[test]
    fn series_cards_come_from_the_schedule_widget() {
        let cards = parse_series_cards(LIST);
        assert_eq!(cards.len(), 6);
        assert_eq!(cards[0].id, "renegade-immortal");
        assert_eq!(cards[0].title, "Renegade Immortal");
        assert_eq!(
            cards[0].url,
            format!("{BASE}/2025/05/renegade-immortal.html")
        );
        assert_eq!(cards[0].source, SOURCE_KEY);
    }

    #[test]
    fn blog_post_cards_have_titles_thumbnails_and_episode_badges() {
        let cards = parse_post_cards(POSTS);
        assert_eq!(cards.len(), 4);
        assert_eq!(
            cards[0].title,
            "Battle Through the Heavens: Battle of the Two Emperors MOVIE Subtitles"
        );
        assert!(
            cards[0]
                .thumbnail
                .starts_with("https://blogger.googleusercontent.com/")
        );
        assert_eq!(cards[1].episode_text, "32-33");
    }

    #[test]
    fn a_series_post_yields_metadata_and_its_episode_label() {
        let series = parse_series(SERIES, &format!("{BASE}/2025/05/renegade-immortal.html"))
            .expect("series from post");
        assert_eq!(series.title, "Renegade Immortal");
        assert_eq!(series.id, "renegade-immortal");
        assert!(series.thumbnail.contains("blogger.googleusercontent.com"));
        assert!(series.description.contains("Wang Lin"));
        assert_eq!(parse_label(SERIES), "Renegade Immortal");
    }

    #[test]
    fn blogger_feed_entries_become_ordered_episodes() {
        let feed: Value = serde_json::from_str(FEED).expect("feed json");
        let episodes = parse_feed_entries(&feed);
        assert_eq!(episodes.len(), 3);
        assert_eq!(episodes[0].episode_number, Some(168));
        assert_eq!(episodes[0].number_text, "168");
        assert!(episodes[0].url.contains("renegade-immortal-episode-168"));
        assert!(episodes[0].date.starts_with("2026-11-22"));
    }

    #[test]
    fn search_and_label_urls_match_the_live_site() {
        let query = CatalogQuery {
            search: "renegade".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&query), format!("{BASE}/search?q=renegade"));
        let query = CatalogQuery {
            genre: "Renegade Immortal".to_string(),
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&query),
            format!("{BASE}/search/label/Renegade+Immortal?&max-results=10&start-index=11")
        );
        assert_eq!(
            catalog_url(&CatalogQuery::default()),
            format!("{BASE}/p/az-list.html")
        );
        assert_eq!(
            feed_url("Renegade Immortal"),
            format!("{BASE}/feeds/posts/default/-/Renegade+Immortal?alt=json&max-results=50")
        );
    }
}
