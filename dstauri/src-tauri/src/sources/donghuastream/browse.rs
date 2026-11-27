//! DonghuaStream browsing: search, trending/catalog, genres and series pages.
//!
//! Ported from `DonghuaStreamSource` in the legacy `sources.py`
//! (`_parse_bs_cards`, `_dhs_image`, `_parse_series`, `_parse_recommendations`),
//! plus the site's advanced-filter form, which the legacy app never exposed.
//!
//! Every parser here is a pure function over HTML so it can be unit-tested
//! against fixtures captured from the live site.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

use crate::config::now_iso8601;
use crate::error::AppError;

use super::{SOURCE_KEY, decode_entities, strip_comments, text_of};
use crate::sources::browse::{
    CatalogFilters, CatalogPage, FilterOption, Genre, SourceCard, SourceEpisode, SourceSeries,
};
use crate::sources::{DONGHUASTREAM_BASE, cache, fetch_text};

const NAMESPACE: &str = "donghuastream";

/// Sort orders the site's filter form offers.
const ORDERS: [(&str, &str); 6] = [
    ("update", "Latest update"),
    ("popular", "Popular"),
    ("latest", "Latest added"),
    ("title", "A-Z"),
    ("titlereverse", "Z-A"),
    ("", "Default"),
];

/// Airing statuses the site's filter form offers.
const STATUSES: [(&str, &str); 5] = [
    ("", "All"),
    ("ongoing", "Ongoing"),
    ("completed", "Completed"),
    ("upcoming", "Upcoming"),
    ("hiatus", "Hiatus"),
];

/// Formats the site's filter form offers.
const KINDS: [(&str, &str); 9] = [
    ("", "All"),
    ("tv", "TV series"),
    ("ona", "ONA"),
    ("ova", "OVA"),
    ("movie", "Movie"),
    ("special", "Special"),
    ("bd", "BD"),
    ("live", "Live action"),
    ("music", "Music"),
];

/// What the Explore filter bar asks for.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogQuery {
    /// Non-empty switches the page into search mode.
    pub search: String,
    pub genre: String,
    pub order: String,
    pub status: String,
    pub kind: String,
    /// 1-based source page. `0`/`1` both mean the first page.
    pub page: usize,
    /// Maximum cards to return; `0` means "everything the page had".
    pub limit: usize,
}

fn compiled(pattern: &str) -> Regex {
    Regex::new(pattern).expect("the DonghuaStream browse patterns are valid")
}

fn article_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<article class="bs".*?</article>"#))
}

fn item_link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        compiled(r#"(?is)<a\s+href="([^"]+)"[^>]*itemprop="url"[^>]*title="([^"]*)""#)
    })
}

fn tip_link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<a\s+href="([^"]+)"[^>]*class="tip"[^>]*title="([^"]*)""#))
}

fn tt_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="tt">\s*(?:<a[^>]*>)?([^<]+)"#))
}

fn epx_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<span class="epx">(.*?)</span>"#))
}

fn status_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="status[^"]*">(.*?)</div>"#))
}

fn typez_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="typez[^"]*">(.*?)</div>"#))
}

fn data_src_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?i)data-src="([^"]+)""#))
}

fn img_src_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<img[^>]+src="([^"]+)""#))
}

fn anime_slug_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r"/anime/([^/?#]+)"))
}

fn filter_input_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        compiled(r#"(?is)<input[^>]*name="([^"]+)"[^>]*value="([^"]*)"[^>]*>\s*<label[^>]*>(.*?)</label>"#)
    })
}

fn spe_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="spe">(.*?)</div>"#))
}

fn spe_field_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<b>\s*([^<]+?)\s*:?\s*</b>(.*?)</span>"#))
}

fn genxed_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="genxed">(.*?)</div>"#))
}

fn anchor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<a\s+href="([^"]+)"[^>]*>(.*?)</a>"#))
}

fn entry_content_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="entry-content"[^>]*>(.*?)</div>"#))
}

fn desc_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="desc"[^>]*>(.*?)</div>"#))
}

fn og_image_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<meta[^>]*property="og:image"[^>]*content="([^"]+)""#))
}

fn meta_description_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<meta[^>]*name="description"[^>]*content="([^"]+)""#))
}

fn eplister_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)class="eplister"(.*?)(?:<div class="bixbox"|$)"#))
}

fn epl_num_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="epl-num">(.*?)</div>"#))
}

fn epl_title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="epl-title">(.*?)</div>"#))
}

fn epl_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r#"(?is)<div class="epl-date">(.*?)</div>"#))
}

fn digits_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r"\d+"))
}

fn url_episode_number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r"-episode-(\d+)"))
}

/// Some series publish episodes as `-part1/`, `-part2/` instead of
/// `-episode-N/`, so recognise that numbering too.
fn url_part_number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r"-part(\d+)"))
}

fn h1_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| compiled(r"(?is)<h1[^>]*>(.*?)</h1>"))
}

/// Canonical series page URL for a slug or any `/anime/<slug>/` link.
pub fn series_url(raw: &str) -> Option<String> {
    let text = raw.trim();
    let slug = match anime_slug_re().captures(text) {
        Some(captures) => captures[1].to_string(),
        None => text.trim_matches('/').to_string(),
    };
    if slug.is_empty() || slug.contains('/') || slug.contains(' ') {
        return None;
    }
    Some(format!("{DONGHUASTREAM_BASE}/anime/{slug}/"))
}

/// The real poster URL, skipping the site's base64 lazy-load placeholder.
pub fn card_image(article: &str) -> String {
    if let Some(captures) = data_src_re().captures(article) {
        let value = decode_entities(&captures[1]).trim().to_string();
        if !value.is_empty() && !value.starts_with("data:") {
            return value;
        }
    }
    for captures in img_src_re().captures_iter(article) {
        let value = decode_entities(&captures[1]).trim().to_string();
        if !value.is_empty() && !value.starts_with("data:") {
            return value;
        }
    }
    String::new()
}

fn first_text(regex: &Regex, html: &str) -> String {
    regex
        .captures(html)
        .map(|captures| text_of(&captures[1]))
        .unwrap_or_default()
}

/// Parse every `<article class="bs">` poster card on a page.
pub fn parse_cards(html: &str) -> Vec<SourceCard> {
    let html = strip_comments(html);
    let mut cards = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for article in article_re().find_iter(&html).map(|found| found.as_str()) {
        let link = item_link_re()
            .captures(article)
            .or_else(|| tip_link_re().captures(article));
        let Some(link) = link else {
            continue;
        };
        let Some(url) = series_url(&link[1]) else {
            continue;
        };
        if !seen.insert(url.clone()) {
            continue;
        }

        let link_title = decode_entities(&link[2]).trim().to_string();
        let title = first_text(tt_re(), article);
        let title = if title.is_empty() { link_title } else { title };
        let episode_text = first_text(epx_re(), article);
        let status = first_text(status_re(), article);
        let id = anime_slug_re()
            .captures(&url)
            .map(|captures| captures[1].to_string())
            .unwrap_or_default();

        cards.push(SourceCard {
            id,
            title,
            url,
            thumbnail: card_image(article),
            status: if status.is_empty() {
                episode_text.clone()
            } else {
                status
            },
            kind: first_text(typez_re(), article),
            episode_text,
            source: SOURCE_KEY.to_string(),
        });
    }

    cards
}

/// Genres, orders, statuses and formats from the advanced-filter form.
pub fn parse_filters(html: &str) -> CatalogFilters {
    let html = strip_comments(html);
    let mut genres = Vec::new();
    let mut seen = HashSet::new();

    for captures in filter_input_re().captures_iter(&html) {
        if &captures[1] != "genre[]" {
            continue;
        }
        let slug = captures[2].trim().to_string();
        if slug.is_empty() || !seen.insert(slug.clone()) {
            continue;
        }
        genres.push(Genre {
            slug,
            label: text_of(&captures[3]),
        });
    }

    let options = |table: &[(&str, &str)]| {
        table
            .iter()
            .map(|(value, label)| FilterOption {
                value: (*value).to_string(),
                label: (*label).to_string(),
            })
            .collect()
    };

    CatalogFilters {
        genres,
        orders: options(&ORDERS),
        statuses: options(&STATUSES),
        kinds: options(&KINDS),
    }
}

/// The `<b>Label:</b> value` metadata block on a series page.
fn spe_fields(html: &str) -> Vec<(String, String)> {
    let Some(block) = spe_re().captures(html) else {
        return Vec::new();
    };
    spe_field_re()
        .captures_iter(&block[1])
        .map(|captures| (text_of(&captures[1]).to_lowercase(), text_of(&captures[2])))
        .collect()
}

fn field(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

/// Episodes from the `eplister` block, oldest first.
pub fn parse_episodes(html: &str) -> Vec<SourceEpisode> {
    let Some(block) = eplister_re().captures(html) else {
        return Vec::new();
    };
    let mut episodes = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for captures in anchor_re().captures_iter(&block[1]) {
        let raw = decode_entities(&captures[1]).trim().to_string();
        if !raw.contains("-episode-") && !raw.contains("-part") {
            continue;
        }
        let url = super::episode_url(&raw);
        if !seen.insert(url.clone()) {
            continue;
        }
        let inner = &captures[2];
        let number_text = first_text(epl_num_re(), inner);
        let number = digits_re()
            .find(&number_text)
            .and_then(|found| found.as_str().parse::<i64>().ok())
            .or_else(|| {
                url_episode_number_re()
                    .captures(&url)
                    .and_then(|found| found[1].parse::<i64>().ok())
            })
            .or_else(|| {
                url_part_number_re()
                    .captures(&url)
                    .and_then(|found| found[1].parse::<i64>().ok())
            });
        let Some(number) = number else {
            continue;
        };
        let title = first_text(epl_title_re(), inner);

        episodes.push(SourceEpisode {
            url,
            title: if title.is_empty() {
                format!("Episode {number}")
            } else {
                title
            },
            episode_number: Some(number),
            number_text,
            date: first_text(epl_date_re(), inner),
        });
    }

    episodes.sort_by(|left, right| {
        left.episode_number
            .cmp(&right.episode_number)
            .then_with(|| left.url.cmp(&right.url))
    });
    episodes
}

/// Recommendation posters below the episode list.
pub fn parse_recommendations(html: &str) -> Vec<SourceCard> {
    let Some(position) = html.find("Recommended Series") else {
        return Vec::new();
    };
    let tail = &html[position..];
    let end = tail
        .find(r#"<div class="section""#)
        .unwrap_or(tail.len().min(40_000));
    parse_cards(&tail[..end]).into_iter().take(8).collect()
}

/// Pure parser: series page HTML → full remote series.
pub fn parse_series(html: &str, url: &str) -> Result<SourceSeries, AppError> {
    let html = strip_comments(html);
    let html = html.as_str();
    let fields = spe_fields(html);

    let mut title = first_text(h1_re(), html);
    if title.is_empty() {
        title = series_url(url)
            .and_then(|canonical| {
                anime_slug_re()
                    .captures(&canonical)
                    .map(|captures| captures[1].replace('-', " "))
            })
            .unwrap_or_default();
    }
    if title.is_empty() {
        return Err(AppError::source_with(
            "DonghuaStream's series page did not contain a title. The site layout may have changed.",
            url.to_string(),
        ));
    }

    let mut description = first_text(entry_content_re(), html);
    if description.is_empty() {
        description = first_text(desc_re(), html);
    }
    if description.is_empty() {
        description = meta_description_re()
            .captures(html)
            .map(|captures| text_of(&captures[1]))
            .unwrap_or_default();
    }

    let genres = genxed_re()
        .captures(html)
        .map(|block| {
            anchor_re()
                .captures_iter(&block[1])
                .map(|captures| text_of(&captures[2]))
                .filter(|label| !label.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let episodes = parse_episodes(html);
    let id = anime_slug_re()
        .captures(url)
        .map(|captures| captures[1].to_string())
        .unwrap_or_default();

    Ok(SourceSeries {
        source: SOURCE_KEY.to_string(),
        id,
        title,
        url: series_url(url).unwrap_or_else(|| url.to_string()),
        thumbnail: og_image_re()
            .captures(html)
            .map(|captures| decode_entities(&captures[1]).trim().to_string())
            .unwrap_or_default(),
        description,
        status: field(&fields, "status"),
        kind: field(&fields, "type"),
        studio: field(&fields, "studio"),
        network: field(&fields, "network"),
        released: field(&fields, "released"),
        duration: field(&fields, "duration"),
        season: field(&fields, "season"),
        genres,
        episode_count: episodes.len(),
        episodes,
        recommendations: parse_recommendations(html),
    })
}

/// Only accept values the site's own form offers.
fn allowed(table: &[(&str, &str)], value: &str) -> String {
    let trimmed = value.trim();
    if table.iter().any(|(key, _)| *key == trimmed) {
        trimmed.to_string()
    } else {
        String::new()
    }
}

fn genre_slug(value: &str) -> String {
    let trimmed = value.trim().to_lowercase();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        trimmed
    } else {
        String::new()
    }
}

/// Build the catalog/search URL for a filter combination.
///
/// The site's real pagination is a `page` query parameter (its own Next link
/// uses `?page=2`). `/anime/page/2/` answers 200 but silently serves page 1,
/// and `/page/2/?s=…` answers 404; search results paginate with `/pagg/N/`.
pub fn catalog_url(query: &CatalogQuery) -> String {
    let page = query.page.max(1);
    let search = query.search.trim();
    if !search.is_empty() {
        let encoded: String = url::form_urlencoded::byte_serialize(search.as_bytes()).collect();
        if page > 1 {
            return format!("{DONGHUASTREAM_BASE}/pagg/{page}/?s={encoded}");
        }
        return format!("{DONGHUASTREAM_BASE}/?s={encoded}");
    }

    let page_param = if page > 1 {
        format!("page={page}&")
    } else {
        String::new()
    };
    let mut url = format!(
        "{DONGHUASTREAM_BASE}/anime/?{page_param}status={}&type={}&order={}",
        allowed(&STATUSES, &query.status),
        allowed(&KINDS, &query.kind),
        allowed(&ORDERS, &query.order),
    );
    let genre = genre_slug(&query.genre);
    if !genre.is_empty() {
        url.push_str(&format!("&genre%5B%5D={genre}"));
    }
    url
}

/// Does this results page actually link to another one?
///
/// Both scraped WordPress themes render either a plain `<a …>Next</a>` link
/// (the archive pages) or a `next page-numbers` anchor. A `<link rel="next">`
/// in `<head>` is deliberately ignored: DonghuaStream emits it even on an
/// empty last page. A full card count is *not* pagination — the broken
/// `/anime/page/N/` URL used to return a full, repeated page with no next link.
pub(crate) fn has_next_page(html: &str) -> bool {
    static NEXT_TEXT_RE: OnceLock<Regex> = OnceLock::new();
    static NEXT_REL_RE: OnceLock<Regex> = OnceLock::new();
    let text = NEXT_TEXT_RE.get_or_init(|| compiled(r#"(?is)<a\b[^>]*>\s*(?:Next\b|&raquo;|»)"#));
    let rel = NEXT_REL_RE
        .get_or_init(|| compiled(r#"(?is)<a\b[^>]*(?:rel=["']next["']|class=["'][^"']*\bnext\b)"#));
    let html = strip_comments(html);
    text.is_match(&html) || rel.is_match(&html)
}

fn empty_page_error(url: &str) -> AppError {
    AppError::source_with(
        "DonghuaStream's series list page did not contain any series. The site layout may have changed.",
        url.to_string(),
    )
}

/// Fetch the filter form (genres, orders, statuses, formats).
pub async fn filters(cache_minutes: u32, refresh: bool) -> Result<CatalogFilters, AppError> {
    let key = "filters";
    if !refresh && let Some(hit) = cache::read::<CatalogFilters>(NAMESPACE, key, cache_minutes) {
        return Ok(hit.value);
    }
    let url = format!("{DONGHUASTREAM_BASE}/anime/");
    let html = fetch_text(&url).await?;
    let parsed = parse_filters(&html);
    if parsed.genres.is_empty() {
        return Err(AppError::source_with(
            "DonghuaStream's filter form did not list any genres. The site layout may have changed.",
            url,
        ));
    }
    cache::write(NAMESPACE, key, &parsed, cache_minutes);
    Ok(parsed)
}

/// Fetch one page of search results or filtered trending series.
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
    // Include the page in the key as well as the URL: previously cached
    // pages from the broken `/anime/page/N/` pattern must never be reused.
    let key = format!("{mode}:{page_number}:{url}");

    if !refresh && let Some(hit) = cache::read::<CatalogPage>(NAMESPACE, &key, cache_minutes) {
        let mut page = hit.value;
        page.cached = true;
        page.age_seconds = hit.age_seconds;
        return Ok(page);
    }

    let html = fetch_text(&url).await?;
    let mut cards = parse_cards(&html);
    // An empty first page means the site layout changed. An empty later page
    // just means the user walked past the end of the list.
    if cards.is_empty() && mode == "catalog" && page_number == 1 {
        return Err(empty_page_error(&url));
    }
    // Only a real Next link means more results. A full page is not evidence:
    // the old `/anime/page/N/` URL returned a full duplicate of page 1.
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

/// Fetch one remote series page (metadata + episodes + recommendations).
pub async fn series(id: &str, cache_minutes: u32, refresh: bool) -> Result<SourceSeries, AppError> {
    let url = series_url(id).ok_or_else(|| {
        AppError::source_with("That is not a DonghuaStream series id.", id.to_string())
    })?;
    let key = format!("series:{url}");
    if !refresh && let Some(hit) = cache::read::<SourceSeries>(NAMESPACE, &key, cache_minutes) {
        return Ok(hit.value);
    }
    let html = fetch_text(&url).await?;
    let parsed = parse_series(&html, &url)?;
    cache::write(NAMESPACE, &key, &parsed, cache_minutes);
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = r#"
        <article class="bs" itemscope="itemscope">
          <div class="bsx">
            <a href="https://donghuastream.org/anime/against-the-sky-supreme/" itemprop="url"
               title="Against the Sky Supreme" class="tip" rel="26331">
              <div class="limit">
                <div class="status Ongoing">Ongoing</div>
                <div class="typez ONA">ONA</div>
                <div class="bt"><span class="epx">Ep 539</span></div>
                <img src="data:image/svg+xml;base64,AAAA"
                     data-src="https://donghuastream.org/wp-content/uploads/poster.webp">
              </div>
              <div class="tt">Against the Sky Supreme<h2 itemprop="headline">x</h2></div>
            </a>
          </div>
        </article>
    "#;

    #[test]
    fn cards_use_the_real_poster_and_the_stable_series_url() {
        let cards = parse_cards(CARD);
        assert_eq!(cards.len(), 1);
        let card = &cards[0];
        assert_eq!(card.id, "against-the-sky-supreme");
        assert_eq!(
            card.url,
            "https://donghuastream.org/anime/against-the-sky-supreme/"
        );
        assert_eq!(card.title, "Against the Sky Supreme");
        assert_eq!(card.status, "Ongoing");
        assert_eq!(card.kind, "ONA");
        assert_eq!(card.episode_text, "Ep 539");
        assert_eq!(
            card.thumbnail,
            "https://donghuastream.org/wp-content/uploads/poster.webp"
        );
    }

    #[test]
    fn duplicate_and_non_series_cards_are_dropped() {
        let html = format!(
            "{CARD}{CARD}<article class=\"bs\"><a href=\"https://donghuastream.org/genres/action/\" itemprop=\"url\" title=\"Action\"></a></article>"
        );
        assert_eq!(parse_cards(&html).len(), 1);
    }

    #[test]
    fn the_filter_form_yields_genre_chips() {
        let html = r#"
            <li><input type="checkbox" id="genre-action" name="genre[]" value="action"> <label for="genre-action">Action</label></li>
            <li><input type="checkbox" id="genre-sci-fi" name="genre[]" value="sci-fi"> <label for="genre-sci-fi">Sci-Fi</label></li>
            <li><input type="radio" id="sort_by-update" name="order" value="update"> <label for="sort_by-update">Latest Update</label></li>
        "#;
        let filters = parse_filters(html);
        assert_eq!(filters.genres.len(), 2);
        assert_eq!(filters.genres[1].slug, "sci-fi");
        assert_eq!(filters.genres[1].label, "Sci-Fi");
        assert!(
            filters
                .orders
                .iter()
                .any(|option| option.value == "popular")
        );
        assert!(
            filters
                .statuses
                .iter()
                .any(|option| option.value == "ongoing")
        );
    }

    #[test]
    fn catalog_urls_carry_only_known_filter_values() {
        let query = CatalogQuery {
            genre: "martial-arts".to_string(),
            order: "popular".to_string(),
            status: "ongoing".to_string(),
            kind: "ona".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&query),
            "https://donghuastream.org/anime/?status=ongoing&type=ona&order=popular&genre%5B%5D=martial-arts"
        );

        let injected = CatalogQuery {
            order: "drop table".to_string(),
            genre: "../etc".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&injected),
            "https://donghuastream.org/anime/?status=&type=&order="
        );

        let mut paged = CatalogQuery {
            genre: "martial-arts".into(),
            order: "popular".into(),
            status: "ongoing".into(),
            kind: "ona".into(),
            ..CatalogQuery::default()
        };
        paged.page = 2;
        assert_eq!(
            catalog_url(&paged),
            "https://donghuastream.org/anime/?page=2&status=ongoing&type=ona&order=popular&genre%5B%5D=martial-arts"
        );

        let mut paged_injected = CatalogQuery {
            order: "drop table".into(),
            genre: "../etc".into(),
            ..CatalogQuery::default()
        };
        paged_injected.page = 4;
        assert_eq!(
            catalog_url(&paged_injected),
            "https://donghuastream.org/anime/?page=4&status=&type=&order="
        );
    }

    #[test]
    fn searches_use_the_site_query_parameter() {
        let query = CatalogQuery {
            search: "renegade immortal".to_string(),
            ..CatalogQuery::default()
        };
        assert_eq!(
            catalog_url(&query),
            "https://donghuastream.org/?s=renegade+immortal"
        );
        let mut paged = query;
        paged.page = 3;
        assert_eq!(
            catalog_url(&paged),
            "https://donghuastream.org/pagg/3/?s=renegade+immortal"
        );
    }

    #[test]
    fn live_catalog_pages_have_distinct_results_and_real_next_links() {
        let page1 = include_str!("../../../tests/fixtures/donghuastream_catalog.html");
        let page2 = include_str!("../../../tests/fixtures/donghuastream_catalog_page2.html");
        let first = parse_cards(page1);
        let second = parse_cards(page2);
        assert_eq!(first[0].id, "100-000-years-of-refining-qi-yang-ascendant");
        assert_eq!(second[0].id, "ancient-vengeance");
        assert!(
            first
                .iter()
                .all(|card| !second.iter().any(|other| other.id == card.id)),
            "page 2 repeated page 1 cards"
        );
        // The captured page-2 archive still links to page 3; the old path-style
        // URL returned page 1 with no such link.
        assert!(has_next_page(page2));
        assert!(!has_next_page(
            "<html><body><div class=\"pagination\"></div></body></html>"
        ));
    }

    #[test]
    fn episodes_are_sorted_oldest_first_and_keep_range_labels() {
        let html = r#"
            <div class="eplister" id="eplister">
              <ul>
                <li><a href="https://donghuastream.org/show-episode-539to540-subtitles/">
                  <div class="epl-num">539to540 (4K)</div>
                  <div class="epl-title">Show Episode 539to540</div>
                  <div class="epl-date">August 14, 2026</div></a></li>
                <li><a href="https://donghuastream.org/show-episode-2-subtitles/">
                  <div class="epl-num">2</div>
                  <div class="epl-title">Show Episode 2</div>
                  <div class="epl-date">July 1, 2021</div></a></li>
                <li><a href="https://donghuastream.org/anime/show/">
                  <div class="epl-num">Series page</div></a></li>
              </ul>
            </div>
            <div class="bixbox">other</div>
        "#;
        let episodes = parse_episodes(html);
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].episode_number, Some(2));
        assert_eq!(episodes[1].episode_number, Some(539));
        assert_eq!(episodes[1].number_text, "539to540 (4K)");
        assert_eq!(episodes[1].date, "August 14, 2026");
        assert_eq!(
            episodes[1].url,
            "https://donghuastream.org/show-episode-539to540-subtitles/"
        );
    }

    #[test]
    fn series_pages_expose_metadata_genres_and_recommendations() {
        let html = format!(
            r#"
            <h1 class="entry-title">Against the Sky Supreme</h1>
            <meta property="og:image" content="https://donghuastream.org/poster.webp" />
            <div class="spe">
              <span><b>Status:</b> Ongoing</span>
              <span><b>Studio:</b> <a href="/studio/soyep/" rel="tag">Soyep</a></span>
              <span><b>Type:</b> ONA</span>
              <span><b>Duration:</b> 7min</span>
            </div>
            <div class="genxed"><a href="/genres/action/" rel="tag">Action</a> <a href="/genres/beast/" rel="tag">Beast</a></div>
            <div class="entry-content" itemprop="description"><p>Two realms at war.</p></div>
            <div class="eplister"><ul><li><a href="https://donghuastream.org/s-episode-1-subtitles/">
              <div class="epl-num">1</div><div class="epl-title">Episode 1</div></a></li></ul></div>
            <div class="bixbox"><h3><span>Recommended Series</span></h3>{CARD}</div>
            "#
        );
        let series = parse_series(
            &html,
            "https://donghuastream.org/anime/against-the-sky-supreme/",
        )
        .expect("series parses");
        assert_eq!(series.title, "Against the Sky Supreme");
        assert_eq!(series.status, "Ongoing");
        assert_eq!(series.studio, "Soyep");
        assert_eq!(series.kind, "ONA");
        assert_eq!(series.duration, "7min");
        assert_eq!(series.genres, vec!["Action", "Beast"]);
        assert_eq!(series.description, "Two realms at war.");
        assert_eq!(series.episode_count, 1);
        assert_eq!(series.recommendations.len(), 1);
        assert_eq!(series.thumbnail, "https://donghuastream.org/poster.webp");
    }

    #[test]
    fn series_ids_normalise_and_reject_junk() {
        assert_eq!(
            series_url("https://donghuastream.org/anime/x/?page=2"),
            Some("https://donghuastream.org/anime/x/".to_string())
        );
        assert_eq!(
            series_url("x"),
            Some("https://donghuastream.org/anime/x/".to_string())
        );
        assert_eq!(series_url(""), None);
        assert_eq!(series_url("a/b"), None);
    }

    #[test]
    fn a_live_catalog_fixture_parses() {
        let html = include_str!("../../../tests/fixtures/donghuastream_catalog.html");
        let cards = parse_cards(html);
        assert!(cards.len() >= 3, "expected posters, got {}", cards.len());
        assert!(cards.iter().all(|card| card.url.contains("/anime/")));
        assert!(
            cards
                .iter()
                .all(|card| !card.thumbnail.starts_with("data:"))
        );
        let filters = parse_filters(html);
        assert!(
            filters
                .genres
                .iter()
                .any(|genre| genre.slug == "martial-arts")
        );
    }

    #[test]
    fn a_live_series_fixture_parses() {
        let html = include_str!("../../../tests/fixtures/donghuastream_series.html");
        let series = parse_series(
            html,
            "https://donghuastream.org/anime/against-the-sky-supreme/",
        )
        .expect("the live series page parses");
        assert_eq!(series.title, "Against the Sky Supreme");
        assert_eq!(series.status, "Ongoing");
        assert!(series.episode_count > 100);
        assert!(series.genres.contains(&"Action".to_string()));
        assert!(!series.description.is_empty());
        assert!(
            series
                .episodes
                .windows(2)
                .all(|pair| pair[0].episode_number <= pair[1].episode_number)
        );
    }
}
