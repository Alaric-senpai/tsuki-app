import { ipcCall } from "./client"

// ── Shared types ──────────────────────────────────────────────────────────────

export interface SourceInfo {
  key: string
  label: string
  available: boolean
  note?: string
}

export interface Genre {
  slug: string
  label: string
}

export interface FilterOption {
  value: string
  label: string
}

export interface CatalogFilters {
  genres: Genre[]
  orders: FilterOption[]
  statuses: FilterOption[]
  kinds: FilterOption[]
}

export interface SourceCard {
  id: string
  title: string
  url: string
  thumbnail: string
  status: string
  kind: string
  episodeText: string
  source: string
}

export interface SourceEpisode {
  url: string
  title: string
  episodeNumber: number | null
  numberText: string
  date: string
}

export interface SourceSeries {
  source: string
  id: string
  title: string
  url: string
  thumbnail: string
  description: string
  status: string
  kind: string
  studio: string
  network: string
  released: string
  duration: string
  season: string
  genres: string[]
  episodeCount: number
  episodes: SourceEpisode[]
  recommendations: SourceCard[]
}

export interface CatalogPage {
  source: string
  mode: string
  /** The search term, empty in catalog mode. */
  query: string
  /** 1-based source page number that produced these cards. */
  page: number
  /** True when the source advertises (or implies) a further page. */
  hasMore: boolean
  cards: SourceCard[]
  total: number
  cached: boolean
  ageSeconds: number
  fetchedAt: string
}

export interface CatalogQuery {
  search?: string
  genre?: string
  order?: string
  status?: string
  kind?: string
  page?: number
  limit?: number
}

export interface SourceImportResult {
  seriesId: number
  seriesName: string
  added: number
  updated: number
  total: number
  queued: import("./downloads").DownloadSnapshot[]
  warnings: string[]
}

// ── Empty fallbacks ───────────────────────────────────────────────────────────

const EMPTY_SOURCES: SourceInfo[] = [
  {
    key: "donghuastream",
    label: "DonghuaStream",
    available: true,
    note: "Search, genres and trending.",
  },
  {
    key: "animecube",
    label: "AnimeCube",
    available: false,
    note: "Temporarily disconnected — AnimeCube downloads are unavailable.",
  },
  {
    key: "animekhor",
    label: "AnimeKhor",
    available: true,
    note: "Search, genres and trending.",
  },
  {
    key: "youtube",
    label: "YouTube",
    available: false,
    note: "Needs a YouTube Data API key in Settings.",
  },
]

const EMPTY_FILTERS: CatalogFilters = {
  genres: [],
  orders: [],
  statuses: [],
  kinds: [],
}

const EMPTY_CATALOG_PAGE: CatalogPage = {
  source: "donghuastream",
  mode: "catalog",
  query: "",
  page: 1,
  hasMore: false,
  cards: [],
  total: 0,
  cached: false,
  ageSeconds: 0,
  fetchedAt: "",
}

// ── IPC commands ──────────────────────────────────────────────────────────────

export function exploreSources(): Promise<SourceInfo[]> {
  return ipcCall<SourceInfo[]>("explore_sources", EMPTY_SOURCES)
}

export function exploreFilters(
  source: string,
  refresh?: boolean,
): Promise<CatalogFilters> {
  return ipcCall<CatalogFilters>("explore_filters", EMPTY_FILTERS, {
    source,
    refresh: refresh ?? null,
  })
}

export function exploreCatalog(
  source: string,
  query: CatalogQuery,
  refresh?: boolean,
): Promise<CatalogPage> {
  return ipcCall<CatalogPage>("explore_catalog", EMPTY_CATALOG_PAGE, {
    source,
    query,
    refresh: refresh ?? null,
  })
}

export function exploreSeries(
  source: string,
  id: string,
  refresh?: boolean,
): Promise<SourceSeries> {
  return ipcCall<SourceSeries>(
    "explore_series",
    {
      source,
      id: "",
      title: "",
      url: "",
      thumbnail: "",
      description: "",
      status: "",
      kind: "",
      studio: "",
      network: "",
      released: "",
      duration: "",
      season: "",
      genres: [],
      episodeCount: 0,
      episodes: [],
      recommendations: [],
    },
    { source, id, refresh: refresh ?? null },
  )
}

export function importSourceSeries(
  source: string,
  id: string,
  episodeUrls: string[],
  download: boolean,
  targetSeriesId?: number | null,
  seriesName?: string | null,
): Promise<SourceImportResult> {
  return ipcCall<SourceImportResult>(
    "import_source_series",
    {
      seriesId: 0,
      seriesName: "",
      added: 0,
      updated: 0,
      total: 0,
      queued: [],
      warnings: [],
    },
    {
      source,
      id,
      episodeUrls,
      download,
      targetSeriesId: targetSeriesId ?? null,
      seriesName: seriesName ?? null,
    },
  )
}
