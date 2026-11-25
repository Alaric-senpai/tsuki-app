"use client"

import { useCallback, useState } from "react"

import {
  exploreCatalog,
  exploreFilters,
  exploreSeries,
  exploreSources,
  importSourceSeries,
  type CatalogFilters,
  type CatalogPage,
  type CatalogQuery,
  type SourceImportResult,
  type SourceInfo,
  type SourceSeries,
} from "@/lib/ipc/explore"
import { toAppIpcError, type AppIpcError } from "@/lib/ipc/client"

import { useAsyncResource, type AsyncResource } from "./use-async-resource"

const NO_SOURCES: SourceInfo[] = []
const NO_FILTERS: CatalogFilters = { genres: [], orders: [], statuses: [], kinds: [] }
const NO_PAGE: CatalogPage = {
  source: "",
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

export function useExploreSources(): AsyncResource<SourceInfo[]> {
  // The IPC layer returns a static fallback list in browser preview; only the
  // real Tauri call resolves to the command.
  const load = useCallback(() => exploreSources(), [])
  return useAsyncResource(load, NO_SOURCES)
}

export function useExploreFilters(
  source: string | null,
  refreshToken = 0,
): AsyncResource<CatalogFilters> {
  const load = useCallback(
    () =>
      source === null ? Promise.resolve(NO_FILTERS) : exploreFilters(source, refreshToken > 0),
    [source, refreshToken],
  )
  return useAsyncResource(load, NO_FILTERS)
}

export function useExploreCatalog(
  source: string | null,
  query: CatalogQuery,
  refreshToken = 0,
): AsyncResource<CatalogPage> {
  const load = useCallback(
    () =>
      source === null || source === ""
        ? Promise.resolve(NO_PAGE)
        : exploreCatalog(source, query, refreshToken > 0),
    [source, query, refreshToken],
  )
  return useAsyncResource(load, NO_PAGE)
}

export function useExploreSeries(
  source: string | null,
  id: string | null,
  refreshToken = 0,
): AsyncResource<SourceSeries | null> {
  const load = useCallback(
    () =>
      source === null || id === null
        ? Promise.resolve(null)
        : exploreSeries(source, id, refreshToken > 0),
    [source, id, refreshToken],
  )
  return useAsyncResource(load, null)
}

export function useSourceImport(): {
  busy: boolean
  error: AppIpcError | null
  last: SourceImportResult | null
  importSeries: (
    source: string,
    id: string,
    episodeUrls: string[],
    download: boolean,
    targetSeriesId?: number | null,
    seriesName?: string | null,
  ) => Promise<SourceImportResult | null>
} {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<AppIpcError | null>(null)
  const [last, setLast] = useState<SourceImportResult | null>(null)

  const run = useCallback(
    async (
      source: string,
      id: string,
      episodeUrls: string[],
      download: boolean,
      targetSeriesId?: number | null,
      seriesName?: string | null,
    ): Promise<SourceImportResult | null> => {
      setBusy(true)
      setError(null)
      try {
        const result = await importSourceSeries(
          source,
          id,
          episodeUrls,
          download,
          targetSeriesId,
          seriesName,
        )
        setLast(result)
        return result
      } catch (caught) {
        setError(toAppIpcError(caught))
        return null
      } finally {
        setBusy(false)
      }
    },
    [],
  )

  return { busy, error, last, importSeries: run }
}

/**
 * Import variant for the add-to-library dialog. It returns the outcome instead
 * of swallowing it so the caller can show a precise success/error toast.
 */
export function useAddToLibrary(): {
  busy: boolean
  run: (args: {
    source: string
    id: string
    episodeUrls: string[]
    download: boolean
    targetSeriesId?: number | null
    seriesName?: string | null
  }) => Promise<{ result: SourceImportResult | null; error: AppIpcError | null }>
} {
  const [busy, setBusy] = useState(false)

  const run = useCallback(
    async (args: {
      source: string
      id: string
      episodeUrls: string[]
      download: boolean
      targetSeriesId?: number | null
      seriesName?: string | null
    }): Promise<{ result: SourceImportResult | null; error: AppIpcError | null }> => {
      setBusy(true)
      try {
        const result = await importSourceSeries(
          args.source,
          args.id,
          args.episodeUrls,
          args.download,
          args.targetSeriesId,
          args.seriesName,
        )
        return { result, error: null }
      } catch (caught) {
        return { result: null, error: toAppIpcError(caught) }
      } finally {
        setBusy(false)
      }
    },
    [],
  )

  return { busy, run }
}
