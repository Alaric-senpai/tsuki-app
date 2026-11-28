//! Shared Explore payload types.
//!
//! Mirrors the dictionaries the legacy `sources.py` returned to the GUI
//! (`_series_card`, `_parse_series`) so the frontend has one shape for every
//! remote source.

use serde::{Deserialize, Serialize};

/// One poster in a search/trending grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCard {
    /// Stable source id (DonghuaStream: the `/anime/<slug>/` slug).
    pub id: String,
    pub title: String,
    /// Stable series page URL — this is what the library stores.
    pub url: String,
    pub thumbnail: String,
    /// `Ongoing`, `Completed`, … as the site labels it.
    pub status: String,
    /// Format badge: `ONA`, `TV`, `Movie`, …
    pub kind: String,
    /// Episode badge text, e.g. `Ep 12` or `Completed`.
    pub episode_text: String,
    pub source: String,
}

/// One episode of a remote series, before it exists in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEpisode {
    /// Stable episode page URL.
    pub url: String,
    pub title: String,
    pub episode_number: Option<i64>,
    /// Raw number label, which can be a range like `539to540`.
    pub number_text: String,
    pub date: String,
}

/// A remote series page: metadata, genres, episodes and recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSeries {
    pub source: String,
    pub id: String,
    pub title: String,
    pub url: String,
    pub thumbnail: String,
    pub description: String,
    pub status: String,
    pub kind: String,
    pub studio: String,
    pub network: String,
    pub released: String,
    pub duration: String,
    pub season: String,
    pub genres: Vec<String>,
    pub episode_count: usize,
    pub episodes: Vec<SourceEpisode>,
    pub recommendations: Vec<SourceCard>,
}

/// A genre chip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub slug: String,
    pub label: String,
}

/// A value/label pair for the order/status/type dropdowns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOption {
    pub value: String,
    pub label: String,
}

/// Everything the filter bar needs, scraped from the site's advanced filter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFilters {
    pub genres: Vec<Genre>,
    pub orders: Vec<FilterOption>,
    pub statuses: Vec<FilterOption>,
    pub kinds: Vec<FilterOption>,
}

/// One page of results plus the telemetry the Explore header shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub source: String,
    /// `search` or `catalog`.
    pub mode: String,
    pub query: String,
    /// 1-based source page number that produced these cards.
    pub page: usize,
    /// True when the source advertises (or clearly implies) a further page.
    pub has_more: bool,
    pub cards: Vec<SourceCard>,
    pub total: usize,
    /// True when this response came from the disk cache.
    pub cached: bool,
    pub age_seconds: u64,
    pub fetched_at: String,
}
