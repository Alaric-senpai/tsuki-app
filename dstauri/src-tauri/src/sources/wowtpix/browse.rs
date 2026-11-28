//! Catalog, filter and series parsing for wowtpix.com.
//!
//! The site uses the same WordPress "anime" theme as DonghuaStream and
//! AnimeKhor: `<article class="bs">` poster cards, an `/anime/` archive and
//! `class="eplister"` episode lists. Every parser is a pure function so the
//! synthetic fixtures can be exercised without network access.

use std::collections::HashSet;

use regex::Regex;
use url::Url;

use super::{BASE, SOURCE_KEY, is_wowtpix_host};
use crate::config::now_iso8601;
use crate::error::AppError;
use crate::sources::browse::{
    CatalogFilters, CatalogPage, FilterOption, Genre, SourceCard, SourceEpisode, SourceSeries,
};
use crate::sources::donghuastream::browse::{CatalogQuery, has_next_page};
use crate::sources::{cache, fetch_text};

const NAMESPACE: &str = "wowtpix";

/// Compile a parser pattern. Literals here are fixed; a malformed pattern is
/// treated as a parse miss rather than crashing the app.
fn rx(pattern: &str) -> Option<Regex> {
    Regex::new(pattern).ok()
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
    let Some(tags) = rx(r"(?is)<[^>]+>") else {
        return input.trim().to_string();
    };
    let without_tags = tags.replace_all(input, " ").to_string();
    let decoded = decode_entities(&without_tags);
    rx(r"\s+")
        .map(|re| re.replace_all(decoded.trim(), " ").to_string())
        .unwrap_or_else(|| decoded.trim().to_string())
}

fn attr_of(fragment: &str, name: &str) -> String {
    let pattern = format!(r#"(?is)\b{}\s*=\s*"([^"]*)""#, regex::escape(name));
    rx(&pattern)
        .and_then(|re| {
            re.captures(fragment)
                .map(|caps| decode_entities(&caps[1]).trim().to_string())
        })
        .unwrap_or_default()
}

fn first_group(pattern: &str, html: &str) -> String {
    rx(pattern)
        .and_then(|re| re.captures(html).map(|caps| caps[1].to_string()))
        .unwrap_or_default()
}

fn first_text(pattern: &str, html: &str) -> String {
    text_of(&first_group(pattern, html))
}

fn enc(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.trim().as_bytes()).collect()
}

/// Last meaningful path segment: `/anime/<slug>/` and `/<slug>/` both occur.
fn slug_from_url(url: &str) -> String {
    let path = Url::parse(url)
        .map(|parsed| parsed.path().to_string())
        .unwrap_or_else(|_| url.split('?').next().unwrap_or_default().to_string());
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(position) = segments
        .iter()
        .position(|part| part.eq_ignore_ascii_case("anime"))
        && let Some(slug) = segments.get(position + 1)
    {
        return (*slug).to_string();
    }
    segments
        .last()
        .map(|slug| (*slug).to_string())
        .unwrap_or_default()
}

/// Canonical series URL for a slug or a full (possibly `www`) page URL.
pub fn series_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (host, slug) =
        if raw.starts_with("//") || raw.starts_with("http://") || raw.starts_with("https://") {
            let absolute = crate::sources::absolute_media_url(raw);
            let parsed = Url::parse(&absolute).ok()?;
            let host = parsed.host_str()?.to_ascii_lowercase();
            if !is_wowtpix_host(&host) {
                return None;
            }
            (host, slug_from_url(&absolute))
        } else {
            let host = Url::parse(BASE).ok()?.host_str()?.to_ascii_lowercase();
            (host, raw.trim_matches('/').to_string())
        };
    if slug.is_empty()
        || !slug
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }
    Some(format!("https://{host}/anime/{slug}/"))
}

/// Poster cards in the main archive/search grid.
pub fn parse_cards(html: &str) -> Vec<SourceCard> {
    let Some(article) = rx(r#"(?is)<article\s+class="bs".*?</article>"#) else {
        return Vec::new();
    };
    let Some(link) = rx(r#"(?is)<a\b[^>]*itemprop="url"[^>]*>"#) else {
        return Vec::new();
    };
    let Some(image) = rx(r"(?is)<img\b[^>]*>") else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for found in article.captures_iter(html) {
        let card_html = &found[0];
        let Some(link_tag) = link.captures(card_html) else {
            continue;
        };
        let tag = &link_tag[0];
        let href = attr_of(tag, "href");
        let Some(url) = series_url(&href) else {
            continue;
        };
        if !seen.insert(url.clone()) {
            continue;
        }
        let title = {
            let title = attr_of(tag, "title");
            if title.is_empty() {
                first_text(r#"(?is)<h2[^>]*>(.*?)</h2>"#, card_html)
            } else {
                title
            }
        };
        if title.is_empty() {
            continue;
        }
        let thumbnail = image
            .captures(card_html)
            .map(|found| {
                let tag = &found[0];
                let lazy = attr_of(tag, "data-src");
                let src = if lazy.is_empty() {
                    attr_of(tag, "src")
                } else {
                    lazy
                };
                if src.starts_with("data:") {
                    String::new()
                } else {
                    src
                }
            })
            .unwrap_or_default();
        let rel = attr_of(tag, "rel");
        let id = if rel.is_empty() {
            slug_from_url(&url)
        } else {
            rel
        };
        cards.push(SourceCard {
            id,
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

/// Episode rows from `class="eplister"`. The first occurrence in a live page
/// may be a CSS rule, so search until a block actually contains `<li>`.
pub fn parse_episodes(html: &str) -> Vec<SourceEpisode> {
    let Some(list_re) = rx(r#"(?is)class="eplister"[^>]*>.*?<ul>(.*?)</ul>"#) else {
        return Vec::new();
    };
    let Some(list) = list_re.captures_iter(html).find_map(|caps| {
        if caps[1].contains("<li") {
            Some(caps[1].to_string())
        } else {
            None
        }
    }) else {
        return Vec::new();
    };
    let Some(item) = rx(r#"(?is)<li\b[^>]*>\s*<a\s+href="([^"]+)"[^>]*>(.*?)</a>"#) else {
        return Vec::new();
    };
    let mut episodes = Vec::new();
    for caps in item.captures_iter(&list) {
        let url = decode_entities(&caps[1]).trim().to_string();
        if url.is_empty() {
            continue;
        }
        let inner = &caps[2];
        let number_text = first_text(r#"(?is)<div\s+class="epl-num"[^>]*>(.*?)</div>"#, inner);
        let title = {
            let found = first_text(r#"(?is)<div\s+class="epl-title"[^>]*>(.*?)</div>"#, inner);
            if found.is_empty() {
                format!("Episode {number_text}")
            } else {
                found
            }
        };
        episodes.push(SourceEpisode {
            url: crate::sources::absolute_media_url(&url),
            title,
            episode_number: number_text
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<i64>()
                .ok(),
            number_text,
            date: first_text(r#"(?is)<div\s+class="epl-date"[^>]*>(.*?)</div>"#, inner),
        });
    }
    episodes
}

fn metadata_fields(html: &str) -> std::collections::HashMap<String, String> {
    let block = first_group(r#"(?is)<div\s+class="spe"[^>]*>(.*?)</div>"#, html);
    let Some(span) = rx(r#"(?is)<span[^>]*>\s*<b>([^<]+?):\s*</b>(.*?)</span>"#) else {
        return std::collections::HashMap::new();
    };
    let mut fields = std::collections::HashMap::new();
    for caps in span.captures_iter(&block) {
        fields.insert(
            text_of(&caps[1]).trim_end_matches(':').to_ascii_lowercase(),
            text_of(&caps[2]),
        );
    }
    fields
}

fn parse_genres(html: &str) -> Vec<String> {
    let block = first_group(r#"(?is)<div\s+class="genxed"[^>]*>(.*?)</div>"#, html);
    let Some(link) = rx(r#"(?is)<a\b[^>]*>(.*?)</a>"#) else {
        return Vec::new();
    };
    link.captures_iter(&block)
        .map(|caps| text_of(&caps[1]))
        .filter(|value| !value.is_empty())
        .collect()
}

fn canonical_url(html: &str) -> String {
    let direct = rx(r#"(?is)<link\s+rel="canonical"\s+href="([^"]+)""#);
    let reverse = rx(r#"(?is)<link\s+href="([^"]+)"\s+rel="canonical""#);
    direct
        .and_then(|re| re.captures(html).map(|caps| decode_entities(&caps[1])))
        .or_else(|| reverse.and_then(|re| re.captures(html).map(|caps| decode_entities(&caps[1]))))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Whole series page → shared Explore payload.
pub fn parse_series(html: &str, page_url: &str) -> Option<SourceSeries> {
    let title = first_text(r#"(?is)<h1\s+class="entry-title"[^>]*>(.*?)</h1>"#, html);
    if title.is_empty() {
        return None;
    }
    let fields = metadata_fields(html);
    let description = {
        let found = first_text(r#"(?is)<div\s+class="desc[^"]*"[^>]*>(.*?)</div>"#, html);
        if found.is_empty() {
            first_text(
                r#"(?is)<meta\s+name="description"\s+content="([^"]+)""#,
                html,
            )
        } else {
            found
        }
    };
    let thumbnail = [
        r#"(?is)<meta\s+property="og:image"\s+content="([^"]+)""#,
        r#"(?is)<meta\s+content="([^"]+)"\s+property="og:image""#,
    ]
    .iter()
    .find_map(|pattern| rx(pattern).and_then(|re| re.captures(html)))
    .map(|caps| decode_entities(&caps[1]))
    .unwrap_or_default();
    let episodes = parse_episodes(html);
    let get = |key: &str| fields.get(key).cloned().unwrap_or_default();
    let canonical = canonical_url(html);
    let url = if canonical.contains("/anime/") {
        canonical
    } else {
        page_url.to_string()
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
        season: get("season"),
        genres: parse_genres(html),
        episode_count: episodes.len(),
        episodes,
        recommendations: Vec::new(),
    })
}

fn clean_genre(raw: &str) -> String {
    let slug = raw.trim().trim_matches('/').to_ascii_lowercase();
    if slug.is_empty()
        || !slug
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        String::new()
    } else {
        slug
    }
}

fn clean_option(raw: &str) -> String {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 40
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == ' ')
    {
        String::new()
    } else {
        value
    }
}

/// Real theme patterns: `/anime/` archives, `/genres/<slug>/` archives,
/// `/anime/page/N/` pagination and `/?s=<term>` search.
pub fn catalog_url(query: &CatalogQuery) -> String {
    let page = query.page.max(1);
    let search = query.search.trim();
    if !search.is_empty() {
        return if page > 1 {
            format!("{BASE}/page/{page}/?s={}", enc(search))
        } else {
            format!("{BASE}/?s={}", enc(search))
        };
    }
    let genre = clean_genre(&query.genre);
    if !genre.is_empty() {
        return if page > 1 {
            format!("{BASE}/genres/{genre}/page/{page}/")
        } else {
            format!("{BASE}/genres/{genre}/")
        };
    }
    let mut params = Vec::new();
    let status = clean_option(&query.status);
    if !status.is_empty() {
        params.push(format!("status={}", enc(&status)));
    }
    let kind = clean_option(&query.kind);
    if !kind.is_empty() {
        params.push(format!("type={}", enc(&kind)));
    }
    let order = clean_option(&query.order);
    if !order.is_empty() {
        params.push(format!("order={}", enc(&order)));
    }
    let suffix = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };
    if page > 1 {
        format!("{BASE}/anime/page/{page}/{suffix}")
    } else {
        format!("{BASE}/anime/{suffix}")
    }
}

/// Read the site's filter form: `genre[]`, `status`, `type` and `order`.
pub fn parse_filters(html: &str) -> CatalogFilters {
    let Some(input) = rx(r#"(?is)<input\b([^>]*)>\s*<label\b[^>]*>(.*?)</label>"#) else {
        return CatalogFilters {
            genres: Vec::new(),
            orders: Vec::new(),
            statuses: Vec::new(),
            kinds: Vec::new(),
        };
    };
    let mut genres = Vec::new();
    let mut orders = Vec::new();
    let mut statuses = Vec::new();
    let mut kinds = Vec::new();
    for caps in input.captures_iter(html) {
        let attrs = &caps[1];
        let name = attr_of(attrs, "name");
        let value = attr_of(attrs, "value");
        let label = text_of(&caps[2]);
        if value.is_empty() || label.is_empty() {
            continue;
        }
        match name.as_str() {
            "genre[]" => {
                if !genres.iter().any(|item: &Genre| item.slug == value) {
                    genres.push(Genre { slug: value, label });
                }
            }
            "status" => statuses.push(FilterOption { value, label }),
            "type" => kinds.push(FilterOption { value, label }),
            "order" => orders.push(FilterOption { value, label }),
            _ => {}
        }
    }
    CatalogFilters {
        genres,
        orders,
        statuses,
        kinds,
    }
}

pub async fn filters(cache_minutes: u32, refresh: bool) -> Result<CatalogFilters, AppError> {
    if !refresh
        && let Some(hit) = cache::read::<CatalogFilters>(NAMESPACE, "filters", cache_minutes)
    {
        return Ok(hit.value);
    }
    let url = format!("{BASE}/anime/");
    let html = fetch_text(&url).await?;
    let parsed = parse_filters(&html);
    if parsed.genres.is_empty() {
        return Err(AppError::source_with(
            "wowtpix.com did not list any genres. The site layout may have changed.",
            url,
        ));
    }
    cache::write(NAMESPACE, "filters", &parsed, cache_minutes);
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
        let mut cached = hit.value;
        cached.cached = true;
        cached.age_seconds = hit.age_seconds;
        return Ok(cached);
    }
    let html = fetch_text(&url).await?;
    let mut cards = parse_cards(&html);
    if cards.is_empty() && mode == "catalog" && page == 1 {
        return Err(AppError::source_with(
            "wowtpix.com's series list did not contain any series. The site layout may have changed.",
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
        AppError::source_with("That is not a wowtpix.com series id.", id.to_string())
    })?;
    let key = format!("series:{url}");
    if !refresh && let Some(hit) = cache::read::<SourceSeries>(NAMESPACE, &key, cache_minutes) {
        return Ok(hit.value);
    }
    let html = fetch_text(&url).await?;
    let parsed = parse_series(&html, &url).ok_or_else(|| {
        AppError::source_with(
            "wowtpix.com did not return series details. The site layout may have changed.",
            url.clone(),
        )
    })?;
    cache::write(NAMESPACE, &key, &parsed, cache_minutes);
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = include_str!("../../../tests/fixtures/wowtpix_catalog.html");
    const DETAIL: &str = include_str!("../../../tests/fixtures/wowtpix_series.html");

    #[test]
    fn cards_and_filters_parse_from_the_theme_markup() {
        let cards = parse_cards(CATALOG);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].id, "101");
        assert_eq!(cards[0].title, "Renegade Immortal");
        assert_eq!(cards[0].url, "https://wowtpix.com/anime/renegade-immortal/");
        assert_eq!(cards[0].status, "Ongoing");
        assert_eq!(cards[0].kind, "ONA");
        assert_eq!(
            cards[1].thumbnail,
            "https://wowtpix.com/wp-content/uploads/perfect-world.jpg"
        );
        assert!(cards.iter().all(|card| card.source == SOURCE_KEY));
        let filters = parse_filters(CATALOG);
        assert_eq!(filters.genres[0].slug, "action");
        assert_eq!(filters.orders[0].value, "update");
        assert_eq!(filters.statuses[0].value, "ongoing");
        assert_eq!(filters.kinds[0].value, "ona");
    }

    #[test]
    fn catalog_urls_follow_the_wordpress_theme_patterns() {
        assert_eq!(
            catalog_url(&CatalogQuery::default()),
            "https://wowtpix.com/anime/"
        );
        let paged = CatalogQuery {
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&paged), "https://wowtpix.com/anime/page/2/");
        let genre = CatalogQuery {
            genre: "action".into(),
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&genre), "https://wowtpix.com/genres/action/");
        let search = CatalogQuery {
            search: "renegade immortal".into(),
            page: 2,
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&search),
            "https://wowtpix.com/page/2/?s=renegade+immortal"
        );
        let filtered = CatalogQuery {
            status: "ongoing".into(),
            kind: "ona".into(),
            order: "update".into(),
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&filtered),
            "https://wowtpix.com/anime/?status=ongoing&type=ona&order=update"
        );
        let injected = CatalogQuery {
            genre: "../etc".into(),
            ..CatalogQuery::default()
        };
        assert_eq!(catalog_url(&injected), "https://wowtpix.com/anime/");
    }

    #[test]
    fn series_and_episodes_parse_from_the_theme_markup() {
        let series = parse_series(DETAIL, "https://wowtpix.com/anime/renegade-immortal/")
            .expect("synthetic series fixture");
        assert_eq!(series.source, SOURCE_KEY);
        assert_eq!(series.id, "renegade-immortal");
        assert_eq!(series.title, "Renegade Immortal");
        assert_eq!(series.status, "Ongoing");
        assert_eq!(series.kind, "ONA");
        assert_eq!(series.studio, "Example Studio");
        assert_eq!(series.episode_count, 2);
        assert_eq!(series.episodes[0].episode_number, Some(2));
        assert_eq!(series.episodes[0].number_text, "2");
        assert_eq!(series.episodes[0].date, "November 21, 2026");
        assert!(series.genres.iter().any(|genre| genre == "Action"));
    }

    #[test]
    fn series_ids_accept_slugs_and_www_urls_only() {
        assert_eq!(
            series_url("renegade-immortal"),
            Some("https://wowtpix.com/anime/renegade-immortal/".to_string())
        );
        assert_eq!(
            series_url("https://www.wowtpix.com/anime/renegade-immortal/"),
            Some("https://www.wowtpix.com/anime/renegade-immortal/".to_string())
        );
        assert_eq!(series_url("../etc"), None);
        assert_eq!(series_url("https://example.org/anime/x/"), None);
    }
}
