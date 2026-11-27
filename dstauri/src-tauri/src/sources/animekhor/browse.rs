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
use crate::sources::donghuastream::browse::{CatalogQuery, has_next_page};
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

/// Canonical series URL from a detail page's `<head>`. WordPress resolves a
/// numeric post id (`/?p=51197`) to this slug, but the request URL itself is
/// not canonical, so the parsed series must use the link from the HTML.
fn canonical_url(html: &str) -> String {
    let link = re(r#"(?is)<link\s+rel="canonical"\s+href="([^"]+)""#);
    let reverse = re(r#"(?is)<link\s+href="([^"]+)"\s+rel="canonical""#);
    link.captures(html)
        .or_else(|| reverse.captures(html))
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .unwrap_or_default()
}

fn first_text(pattern: &str, html: &str) -> String {
    re(pattern)
        .captures(html)
        .map(|caps| text_of(&caps[1]))
        .unwrap_or_default()
}

/// Build the detail URL for an id. The site's article cards expose a numeric
/// WordPress post id in `rel`; `/anime/{numeric}/` 404s, but `/?p={numeric}`
/// redirects to the canonical slug page. Accept slugs and full URLs as well.
pub fn series_url(raw: &str) -> Option<String> {
    let id = raw.trim();
    if id.is_empty() {
        return None;
    }
    if id.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(format!("{BASE}/?p={id}"));
    }
    let slug = slug_from_url(id);
    if slug.is_empty() {
        // A caller may hand back the numeric permalink itself.
        if let Some(post) = re(r#"[?&]p=(\d+)"#).captures(id) {
            return Some(format!("{BASE}/?p={}", &post[1]));
        }
    }
    let slug = if slug.is_empty() {
        id.trim_matches('/').to_string()
    } else {
        slug
    };
    if slug.is_empty()
        || slug.contains('/')
        || slug.contains(' ')
        || slug.contains('?')
        || slug.contains('#')
    {
        return None;
    }
    Some(format!("{BASE}/anime/{slug}/"))
}

/// Read the poster cards in the main `<article class="bs">` grid.
///
/// The old parser read `<a class="series" … rel="123">` links; on the live
/// site those only exist in the "Popular Series" sidebar widget and are the
/// same on every page, so genre and page changes appeared to do nothing and
/// numeric ids produced 404 detail URLs.
pub fn parse_cards(html: &str) -> Vec<SourceCard> {
    let article = re(r#"(?is)<article\s+class="bs".*?</article>"#);
    let link = re(r#"(?is)<a\s+href="([^"]+)"[^>]*itemprop="url"[^>]*>"#);
    let image = re(r#"(?is)<img\b[^>]*>"#);
    let mut cards = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for found in article.captures_iter(html) {
        let card_html = &found[0];
        let Some(link_tag) = link.captures(card_html) else {
            continue;
        };
        let href = decode_entities(&link_tag[1]).trim().to_string();
        let slug = slug_from_url(&href);
        if slug.is_empty() {
            // Home-page cards link to episode articles, not to series pages.
            continue;
        }
        let url = format!("{BASE}/anime/{slug}/");
        if !seen.insert(url.clone()) {
            continue;
        }
        let title = attr_of(&link_tag[0], "title");
        let title = if title.is_empty() {
            first_text(r#"(?is)<div class="tt">(.*?)<h2"#, card_html)
        } else {
            title
        };
        if title.is_empty() {
            continue;
        }
        let thumbnail = image
            .captures(card_html)
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
            status: first_text(r#"(?is)<div class="status[^"]*">(.*?)</div>"#, card_html),
            kind: first_text(r#"(?is)<div class="typez[^"]*">(.*?)</div>"#, card_html),
            episode_text: first_text(r#"(?is)<span class="epx">(.*?)</span>"#, card_html),
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
    // A numeric-id request (`/?p=123`) is answered by a 301 to the slug page;
    // use the canonical link so the library stores a stable, shareable URL.
    let canonical = canonical_url(html);
    let url = if canonical.contains("/anime/") {
        canonical
    } else {
        page_url.to_string()
    };
    let slug = slug_from_url(&url);
    let id = if slug.is_empty() {
        re(r#"[?&]p=(\d+)"#)
            .captures(page_url)
            .map(|caps| caps[1].to_string())
            .unwrap_or_default()
    } else {
        slug
    };

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

/// Real URL patterns, verified against the live site:
/// * series archive paginates with `?page=N` (`/anime/page/N/` serves page 1);
/// * genre archives paginate with `/genres/{slug}/page/N/`;
/// * search uses `/page/N/?s=…`.
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
        format!("{BASE}/anime/?page={page}")
    } else {
        format!("{BASE}/anime/")
    }
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
    let page_number = query.page.max(1);
    // The old keys cached a repeated page under the URL alone; include the
    // page number so stale responses can never be served after this fix.
    let key = format!("{mode}:{page_number}:{url}");
    if !refresh && let Some(hit) = cache::read::<CatalogPage>(NAMESPACE, &key, cache_minutes) {
        let mut page = hit.value;
        page.cached = true;
        page.age_seconds = hit.age_seconds;
        return Ok(page);
    }

    let html = fetch_text(&url).await?;
    let mut cards = parse_cards(&html);
    if cards.is_empty() && mode == "catalog" && page_number == 1 {
        return Err(AppError::source_with(
            "animekhor.org's catalog did not contain any series. The site layout may have changed.",
            url,
        ));
    }
    // Next must come from the site's pagination markup, not a full page: the
    // sidebar cards always filled the grid even when nothing changed.
    let has_more = !cards.is_empty() && has_next_page(&html);
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
    let url = series_url(id).ok_or_else(|| {
        AppError::source_with("That is not an animekhor.org series id.", id.to_string())
    })?;
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
    const PAGE1: &str = include_str!("../../../tests/fixtures/animekhor_anime_page1.html");
    const PAGE2: &str = include_str!("../../../tests/fixtures/animekhor_anime_page2.html");
    const GENRE1: &str = include_str!("../../../tests/fixtures/animekhor_genre_action.html");
    const GENRE2: &str = include_str!("../../../tests/fixtures/animekhor_genre_action_page2.html");

    #[test]
    fn cards_are_read_from_the_live_catalog_grid() {
        let cards = parse_cards(PAGE1);
        assert_eq!(cards.len(), 5);
        let card = &cards[0];
        assert_eq!(card.id, "4221");
        assert_eq!(
            card.title,
            "100,000 Levels of Body Refining : All the dogs I raise are the Emperor"
        );
        assert_eq!(
            card.url,
            "https://animekhor.org/anime/100000-levels-of-body-refining-all-the-dogs-i-raise-are-the-emperor/"
        );
        assert!(card.thumbnail.starts_with("https://i1.wp.com/"));
        assert_eq!(card.kind, "Comic");
        assert_eq!(card.episode_text, "Ongoing");
        assert_eq!(card.source, SOURCE_KEY);
    }

    #[test]
    fn sidebar_widget_links_are_not_mistaken_for_catalog_cards() {
        // The old parser only saw these identical "Popular Series" links,
        // which is why genre and page changes appeared to do nothing.
        assert!(parse_cards(HOME).is_empty());
    }

    #[test]
    fn catalog_pages_and_genres_return_distinct_cards() {
        let first = parse_cards(PAGE1);
        let second = parse_cards(PAGE2);
        assert_eq!(first[0].id, "4221");
        assert_eq!(second[0].id, "73266");
        assert!(
            first
                .iter()
                .all(|card| !second.iter().any(|other| other.id == card.id)),
            "page 2 repeated page 1 cards"
        );
        assert!(has_next_page(PAGE1));
        assert!(has_next_page(PAGE2));

        let genre1 = parse_cards(GENRE1);
        let genre2 = parse_cards(GENRE2);
        assert_eq!(genre1[0].id, "4221");
        assert_eq!(genre2[0].id, "31546");
        assert!(
            genre1
                .iter()
                .all(|card| !genre2.iter().any(|other| other.id == card.id)),
            "genre page 2 repeated page 1 cards"
        );
        assert!(has_next_page(GENRE1));
        assert!(has_next_page(GENRE2));
        assert!(!has_next_page(r#"<div class="pagination"></div>"#));
    }

    #[test]
    fn catalog_urls_use_the_real_pagination_patterns() {
        assert_eq!(
            catalog_url(&CatalogQuery::default()),
            "https://animekhor.org/anime/"
        );
        let paged = CatalogQuery {
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&paged), "https://animekhor.org/anime/?page=2");
        let genre = CatalogQuery {
            genre: "action".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&genre), "https://animekhor.org/genres/action/");
        let genre_paged = CatalogQuery {
            genre: "action".to_string(),
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&genre_paged),
            "https://animekhor.org/genres/action/page/2/"
        );
        let search_paged = CatalogQuery {
            search: "immortal".to_string(),
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&search_paged),
            "https://animekhor.org/page/2/?s=immortal"
        );
        let injected = CatalogQuery {
            genre: "../etc".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&injected), "https://animekhor.org/anime/");
    }

    #[test]
    fn numeric_ids_build_a_wordpress_permalink() {
        assert_eq!(
            series_url("51197"),
            Some("https://animekhor.org/?p=51197".to_string())
        );
        assert_eq!(
            series_url("https://animekhor.org/anime/renegade-immortal/"),
            Some("https://animekhor.org/anime/renegade-immortal/".to_string())
        );
        assert_eq!(series_url("../etc"), None);
        assert_eq!(series_url(""), None);
    }

    #[test]
    fn numeric_detail_pages_use_the_canonical_slug() {
        let series = parse_series(DETAIL, "https://animekhor.org/?p=28305").expect("series");
        assert_eq!(series.id, "renegade-immortal");
        assert_eq!(series.url, "https://animekhor.org/anime/renegade-immortal/");
        assert_eq!(series.title, "Renegade Immortal");
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
