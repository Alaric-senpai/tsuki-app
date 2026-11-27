"use client"

import { useCallback } from "react"

import {
  ALL_SOURCES,
  DEFAULT_LIBRARY_VIEW,
  parseLibraryFilter,
  parseLibrarySort,
  parsePage,
  parsePageSize,
  parseText,
  type LibraryView,
} from "@/components/features/library/library-view"

import { usePersistedState } from "./use-persisted-state"

export const LIBRARY_STORAGE_KEYS = {
  filter: "tsuki.library.filter",
  sort: "tsuki.library.sort",
  query: "tsuki.library.query",
  page: "tsuki.library.page",
  pageSize: "tsuki.library.pageSize",
  source: "tsuki.library.source",
} as const

/** Library view state that survives reloads, kept out of the route file. */
export function useLibraryView() {
  const [filter, setFilter] = usePersistedState(
    LIBRARY_STORAGE_KEYS.filter,
    DEFAULT_LIBRARY_VIEW.filter,
    parseLibraryFilter,
  )
  const [sort, setSort] = usePersistedState(
    LIBRARY_STORAGE_KEYS.sort,
    DEFAULT_LIBRARY_VIEW.sort,
    parseLibrarySort,
  )
  const [query, setQuery] = usePersistedState(
    LIBRARY_STORAGE_KEYS.query,
    DEFAULT_LIBRARY_VIEW.query,
    parseText,
  )
  const [page, setPage] = usePersistedState(
    LIBRARY_STORAGE_KEYS.page,
    DEFAULT_LIBRARY_VIEW.page,
    parsePage,
  )
  const [pageSize, setPageSize] = usePersistedState(
    LIBRARY_STORAGE_KEYS.pageSize,
    DEFAULT_LIBRARY_VIEW.pageSize,
    parsePageSize,
  )
  const [source, setSource] = usePersistedState(
    LIBRARY_STORAGE_KEYS.source,
    DEFAULT_LIBRARY_VIEW.source,
    parseText,
  )

  const changeFilter = useCallback(
    (next: string) => {
      setFilter(parseLibraryFilter(next) ?? DEFAULT_LIBRARY_VIEW.filter)
      setPage(1)
    },
    [setFilter, setPage],
  )

  const changeQuery = useCallback(
    (next: string) => {
      setQuery(next)
      setPage(1)
    },
    [setQuery, setPage],
  )

  const changeSort = useCallback(
    (next: string) => {
      setSort(parseLibrarySort(next) ?? DEFAULT_LIBRARY_VIEW.sort)
      setPage(1)
    },
    [setSort, setPage],
  )

  const changeSource = useCallback(
    (next: string) => {
      setSource(next || ALL_SOURCES)
      setPage(1)
    },
    [setSource, setPage],
  )

  const changePageSize = useCallback(
    (next: string) => {
      setPageSize(parsePageSize(next) ?? DEFAULT_LIBRARY_VIEW.pageSize)
      setPage(1)
    },
    [setPageSize, setPage],
  )

  const reset = useCallback(() => {
    setFilter(DEFAULT_LIBRARY_VIEW.filter)
    setSort(DEFAULT_LIBRARY_VIEW.sort)
    setQuery(DEFAULT_LIBRARY_VIEW.query)
    setPage(DEFAULT_LIBRARY_VIEW.page)
    setPageSize(DEFAULT_LIBRARY_VIEW.pageSize)
    setSource(DEFAULT_LIBRARY_VIEW.source)
  }, [setFilter, setSort, setQuery, setPage, setPageSize, setSource])

  const view: LibraryView = { filter, sort, query, page, pageSize, source }

  return {
    view,
    setFilter: changeFilter,
    setSort: changeSort,
    setQuery: changeQuery,
    setPage,
    setPageSize: changePageSize,
    setSource: changeSource,
    reset,
  }
}
