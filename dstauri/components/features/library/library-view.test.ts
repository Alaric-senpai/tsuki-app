import { describe, expect, it } from "vitest"

import {
  EMPTY_SERIES,
  type SeriesSummary,
} from "@/lib/ipc/library"

import {
  DEFAULT_LIBRARY_VIEW,
  filterAndSortLibrary,
  matchesFilter,
  matchesSource,
  paginateLibrary,
  parseLibraryFilter,
  parseLibrarySort,
  parsePage,
  parsePageSize,
  parseText,
} from "./library-view"

function series(overrides: Partial<SeriesSummary> = {}): SeriesSummary {
  return {
    ...EMPTY_SERIES,
    id: 1,
    key: "key-1",
    name: "Alpha",
    sourceKey: "animekhor",
    createdAt: "2026-01-01T00:00:00Z",
    ...overrides,
  }
}

const rows = [
  series({
    id: 1,
    name: "Zeta",
    createdAt: "2026-01-03T00:00:00Z",
    totalEpisodes: 5,
  }),
  series({
    id: 2,
    key: "key-2",
    name: "Alpha",
    sourceKey: "donghuastream",
    createdAt: "2026-01-01T00:00:00Z",
    downloadedEpisodes: 12,
    totalEpisodes: 12,
  }),
  series({
    id: 3,
    name: "Beta",
    createdAt: "2026-01-02T00:00:00Z",
    failedEpisodes: 1,
    totalEpisodes: 4,
  }),
]

const options = {
  filter: "All" as const,
  sort: "latest-added" as const,
  query: "",
  source: DEFAULT_LIBRARY_VIEW.source,
}

describe("matchesFilter", () => {
  it("keeps the original status semantics", () => {
    expect(matchesFilter(rows[0], "All")).toBe(true)
    expect(matchesFilter(rows[1], "Completed")).toBe(true)
    expect(matchesFilter(rows[2], "Failed")).toBe(true)
    expect(matchesFilter(rows[0], "Watching")).toBe(true)
    expect(matchesFilter(rows[1], "Watching")).toBe(false)
    expect(matchesFilter(rows[2], "Watching")).toBe(false)
  })
})

describe("matchesSource", () => {
  it("treats the all-sources option as no filter", () => {
    expect(matchesSource(rows[0], "All sources")).toBe(true)
    expect(matchesSource(rows[1], "animekhor")).toBe(false)
    expect(matchesSource(rows[1], "DONGHUASTREAM")).toBe(true)
  })
})

describe("filterAndSortLibrary", () => {
  it("sorts newest and oldest first with stable id tie-breaks", () => {
    expect(
      filterAndSortLibrary(rows, options).map((item) => item.id),
    ).toEqual([1, 3, 2])
    expect(
      filterAndSortLibrary(rows, { ...options, sort: "oldest-added" }).map(
        (item) => item.id,
      ),
    ).toEqual([2, 3, 1])
  })

  it("sorts names both ways", () => {
    expect(
      filterAndSortLibrary(rows, { ...options, sort: "name-asc" }).map(
        (item) => item.name,
      ),
    ).toEqual(["Alpha", "Beta", "Zeta"])
    expect(
      filterAndSortLibrary(rows, { ...options, sort: "name-desc" }).map(
        (item) => item.name,
      ),
    ).toEqual(["Zeta", "Beta", "Alpha"])
  })

  it("groups failed entries first for the status sort", () => {
    expect(
      filterAndSortLibrary(rows, { ...options, sort: "status" }).map(
        (item) => item.id,
      ),
    ).toEqual([3, 1, 2])
  })

  it("applies query, status and source filters together", () => {
    expect(
      filterAndSortLibrary(rows, {
        ...options,
        filter: "Failed",
        query: "BET",
        source: "animekhor",
      }).map((item) => item.id),
    ).toEqual([3])
    expect(
      filterAndSortLibrary(rows, { ...options, query: "key-2" }).map(
        (item) => item.id,
      ),
    ).toEqual([2])
  })
})

describe("paginateLibrary", () => {
  it("clamps stale pages and supports an all page size", () => {
    expect(paginateLibrary(rows, 99, "1").page).toBe(3)
    expect(paginateLibrary(rows, 99, "1").rows.map((item) => item.id)).toEqual([3])
    expect(paginateLibrary(rows, 1, "all")).toEqual({
      rows,
      page: 1,
      pageCount: 1,
    })
    expect(paginateLibrary([], 5, "10")).toEqual({ rows: [], page: 1, pageCount: 1 })
  })
})

describe("storage validators", () => {
  it("accepts only known filter, sort and page size values", () => {
    expect(parseLibraryFilter("Watching")).toBe("Watching")
    expect(parseLibraryFilter("unknown")).toBeNull()
    expect(parseLibrarySort("name-desc")).toBe("name-desc")
    expect(parseLibrarySort("random")).toBeNull()
    expect(parsePageSize("15")).toBe("15")
    expect(parsePageSize("7")).toBeNull()
  })

  it("validates pages and text", () => {
    expect(parsePage(3)).toBe(3)
    expect(parsePage(0)).toBeNull()
    expect(parsePage("2")).toBeNull()
    expect(parseText("hello")).toBe("hello")
    expect(parseText(42)).toBeNull()
  })
})
