import type { SeriesSummary } from "@/lib/ipc/library"

export const LIBRARY_FILTERS = ["All", "Watching", "Completed", "Failed"] as const
export type LibraryFilter = (typeof LIBRARY_FILTERS)[number]

export const LIBRARY_SORTS = [
  "latest-added",
  "oldest-added",
  "name-asc",
  "name-desc",
  "status",
] as const
export type LibrarySort = (typeof LIBRARY_SORTS)[number]

export const LIBRARY_SORT_LABELS: Record<LibrarySort, string> = {
  "latest-added": "Latest added",
  "oldest-added": "Oldest added",
  "name-asc": "Name A–Z",
  "name-desc": "Name Z–A",
  status: "Status",
}

export const LIBRARY_PAGE_SIZES = [
  { value: "5", label: "5 per page" },
  { value: "10", label: "10 per page" },
  { value: "15", label: "15 per page" },
  { value: "30", label: "30 per page" },
  { value: "50", label: "50 per page" },
  { value: "all", label: "All" },
] as const

export interface LibraryView {
  filter: LibraryFilter
  sort: LibrarySort
  query: string
  page: number
  pageSize: string
  source: string
}

export const DEFAULT_LIBRARY_VIEW: LibraryView = {
  filter: "All",
  sort: "latest-added",
  query: "",
  page: 1,
  pageSize: "10",
  source: "All sources",
}

export const ALL_SOURCES = "All sources"

/** Status chips keep the semantics of the original library page. */
export function matchesFilter(series: SeriesSummary, filter: LibraryFilter): boolean {
  if (filter === "All") return true
  if (filter === "Failed") return series.failedEpisodes > 0
  if (filter === "Completed") {
    return series.totalEpisodes > 0 && series.downloadedEpisodes >= series.totalEpisodes
  }
  return series.downloadedEpisodes < series.totalEpisodes && series.failedEpisodes === 0
}

/** `sourceKey` is the only source-like field on `SeriesSummary` (no tags yet). */
export function matchesSource(series: SeriesSummary, source: string): boolean {
  if (!source || source === ALL_SOURCES) return true
  return series.sourceKey.trim().toLowerCase() === source.trim().toLowerCase()
}

function statusRank(series: SeriesSummary): number {
  if (series.failedEpisodes > 0) return 0
  if (series.totalEpisodes > 0 && series.downloadedEpisodes >= series.totalEpisodes) return 2
  return 1
}

/** Pure filter + sort helper, isolated from hooks and IPC for unit testing. */
export function filterAndSortLibrary(
  rows: readonly SeriesSummary[],
  view: Pick<LibraryView, "filter" | "sort" | "query" | "source">,
): SeriesSummary[] {
  const needle = view.query.trim().toLowerCase()
  const filtered = rows.filter(
    (series) =>
      matchesFilter(series, view.filter) &&
      matchesSource(series, view.source) &&
      (!needle ||
        series.name.toLowerCase().includes(needle) ||
        series.key.includes(needle)),
  )

  const sorted = [...filtered]
  switch (view.sort) {
    case "oldest-added":
      sorted.sort(
        (a, b) => a.createdAt.localeCompare(b.createdAt) || a.id - b.id,
      )
      break
    case "name-asc":
      sorted.sort((a, b) => a.name.localeCompare(b.name) || a.id - b.id)
      break
    case "name-desc":
      sorted.sort((a, b) => b.name.localeCompare(a.name) || a.id - b.id)
      break
    case "status":
      sorted.sort(
        (a, b) =>
          statusRank(a) - statusRank(b) || a.name.localeCompare(b.name),
      )
      break
    case "latest-added":
    default:
      sorted.sort(
        (a, b) => b.createdAt.localeCompare(a.createdAt) || b.id - a.id,
      )
      break
  }
  return sorted
}

/** Clamp stale persisted pages against a changed result set or page size. */
export function paginateLibrary(
  rows: readonly SeriesSummary[],
  page: number,
  pageSize: string,
): { rows: SeriesSummary[]; page: number; pageCount: number } {
  const limit = pageSize === "all" ? Math.max(rows.length, 1) : Number(pageSize)
  const pageCount = Math.max(1, Math.ceil(rows.length / limit))
  const currentPage = Math.min(Math.max(1, page), pageCount)
  return {
    rows: rows.slice((currentPage - 1) * limit, currentPage * limit),
    page: currentPage,
    pageCount,
  }
}

function oneOf<T extends string>(
  raw: unknown,
  values: readonly T[],
): T | null {
  return typeof raw === "string" && values.some((value) => value === raw)
    ? (raw as T)
    : null
}

export function parseLibraryFilter(raw: unknown): LibraryFilter | null {
  return oneOf(raw, LIBRARY_FILTERS)
}

export function parseLibrarySort(raw: unknown): LibrarySort | null {
  return oneOf(raw, LIBRARY_SORTS)
}

export function parsePageSize(raw: unknown): string | null {
  return oneOf(
    raw,
    LIBRARY_PAGE_SIZES.map((option) => option.value),
  )
}

export function parsePage(raw: unknown): number | null {
  return typeof raw === "number" && Number.isInteger(raw) && raw >= 1 ? raw : null
}

export function parseText(raw: unknown): string | null {
  return typeof raw === "string" ? raw : null
}
