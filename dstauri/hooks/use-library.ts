"use client"

import { useCallback, useState } from "react"
import { toast } from "sonner"

import {
  DEFAULT_CONFIG,
  getConfig,
  setExploreConfig,
  setPlayerConfig,
  setSetting,
  setYouTubeConfig,
  type AppConfig,
  type SetExploreConfigArgs,
  type SetPlayerConfigArgs,
  type SetYouTubeConfigArgs,
} from "@/lib/ipc/config"
import {
  EMPTY_LIBRARY_STATS,
  checkSeriesUpdates,
  createEpisode,
  deleteEpisode,
  getSeries,
  libraryStats,
  listEpisodes,
  listSeries,
  pickBannerFile,
  recentEpisodes,
  resyncSeriesBanners,
  seriesPosters,
  setSeriesBanner,
  setSeriesCheckUpdates,
  syncArchives,
  updateEpisode,
  updateSeriesDetails,
  type Episode,
  type EpisodeWithSeries,
  type LibraryStats,
  type SeriesPoster,
  type SeriesSummary,
  type SeriesUpdate,
  type SetSeriesBannerArgs,
  type UpdateSeriesDetailsArgs,
} from "@/lib/ipc/library"
import {
  legacyStatus,
  pullLegacyData,
  type LegacyStatus,
  type PullSummary,
} from "@/lib/ipc/legacy"
import {
  importLegacyJson,
  legacyExportPath,
  type ImportSummary,
} from "@/lib/ipc/migrate"
import { toAppIpcError, type AppIpcError } from "@/lib/ipc/client"

import { useAsyncResource, type AsyncResource } from "./use-async-resource"

const NO_SERIES: SeriesSummary[] = []
const NO_EPISODES: Episode[] = []
const NO_CROSS_EPISODES: EpisodeWithSeries[] = []
const NO_POSTERS: SeriesPoster[] = []

export function useSeries(): AsyncResource<SeriesSummary[]> {
  const load = useCallback(() => listSeries(), [])
  return useAsyncResource(load, NO_SERIES)
}

export function useSeriesDetail(id: string | null): AsyncResource<SeriesSummary | null> {
  const load = useCallback(() => (id ? getSeries(id) : Promise.resolve(null)), [id])
  return useAsyncResource(load, null)
}

export function useEpisodes(
  seriesId: number | null,
  status?: string,
): AsyncResource<Episode[]> {
  const load = useCallback(
    () => (seriesId === null ? Promise.resolve([]) : listEpisodes(seriesId, status)),
    [seriesId, status],
  )
  return useAsyncResource(load, NO_EPISODES)
}

export function useRecentEpisodes(
  status?: string,
  limit = 50,
): AsyncResource<EpisodeWithSeries[]> {
  const load = useCallback(() => recentEpisodes(status, limit), [status, limit])
  return useAsyncResource(load, NO_CROSS_EPISODES)
}

export function useSeriesPosters(): AsyncResource<SeriesPoster[]> {
  const load = useCallback(() => seriesPosters(), [])
  return useAsyncResource(load, NO_POSTERS)
}

export function useLibraryStats(): AsyncResource<LibraryStats> {
  const load = useCallback(() => libraryStats(), [])
  return useAsyncResource(load, EMPTY_LIBRARY_STATS)
}

export function useAppConfig(): AsyncResource<AppConfig> & {
  savingKey: string | null
  saveError: AppIpcError | null
  saveSetting: (key: string, value: string) => Promise<void>
  savePlayerConfig: (args: SetPlayerConfigArgs) => Promise<void>
  saveExploreConfig: (args: SetExploreConfigArgs) => Promise<void>
  saveYouTubeConfig: (args: SetYouTubeConfigArgs) => Promise<void>
} {
  const load = useCallback(() => getConfig(), [])
  const resource = useAsyncResource(load, DEFAULT_CONFIG)
  const { setData } = resource
  const [savingKey, setSavingKey] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<AppIpcError | null>(null)

  const runSave = useCallback(
    async (key: string, save: () => Promise<AppConfig>) => {
      setSavingKey(key)
      setSaveError(null)
      try {
        setData(await save())
        toast.success("Setting saved", { description: key })
      } catch (error) {
        const appError = toAppIpcError(error)
        setSaveError(appError)
        toast.error("Could not save setting", {
          description: appError.detail ?? appError.message,
        })
      } finally {
        setSavingKey(null)
      }
    },
    [setData],
  )

  const saveSetting = useCallback(
    (key: string, value: string) => runSave(key, () => setSetting(key, value)),
    [runSave],
  )

  const savePlayerConfig = useCallback(
    (args: SetPlayerConfigArgs) => runSave("player", () => setPlayerConfig(args)),
    [runSave],
  )

  const saveExploreConfig = useCallback(
    (args: SetExploreConfigArgs) => runSave("explore", () => setExploreConfig(args)),
    [runSave],
  )

  const saveYouTubeConfig = useCallback(
    (args: SetYouTubeConfigArgs) => runSave("youtube", () => setYouTubeConfig(args)),
    [runSave],
  )

  return {
    ...resource,
    savingKey,
    saveError,
    saveSetting,
    savePlayerConfig,
    saveExploreConfig,
    saveYouTubeConfig,
  }
}

export function useSeriesMutations(): {
  busy: boolean
  error: AppIpcError | null
  updateSeries: (args: UpdateSeriesDetailsArgs) => Promise<SeriesSummary | null>
  setBanner: (args: SetSeriesBannerArgs) => Promise<SeriesSummary | null>
  resyncBanners: () => Promise<number | null>
} {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<AppIpcError | null>(null)

  const run = useCallback(async <T,>(task: () => Promise<T>): Promise<T | null> => {
    setBusy(true)
    setError(null)
    try {
      return await task()
    } catch (caught) {
      setError(toAppIpcError(caught))
      return null
    } finally {
      setBusy(false)
    }
  }, [])

  const updateSeries = useCallback(
    (args: UpdateSeriesDetailsArgs) => run(() => updateSeriesDetails(args)),
    [run],
  )

  const setBanner = useCallback(
    (args: SetSeriesBannerArgs) => run(() => setSeriesBanner(args)),
    [run],
  )

  const resyncBanners = useCallback(() => run(() => resyncSeriesBanners()), [run])

  return { busy, error, updateSeries, setBanner, resyncBanners }
}

export function useArchiveSync(): {
  syncing: boolean
  changed: number | null
  error: AppIpcError | null
  sync: () => Promise<void>
} {
  const [syncing, setSyncing] = useState(false)
  const [changed, setChanged] = useState<number | null>(null)
  const [error, setError] = useState<AppIpcError | null>(null)

  const sync = useCallback(async () => {
    setSyncing(true)
    setError(null)
    try {
      setChanged(await syncArchives())
    } catch (caught) {
      setError(toAppIpcError(caught))
    } finally {
      setSyncing(false)
    }
  }, [])

  return { syncing, changed, error, sync }
}

export function useEpisodeMutations(): {
  busy: boolean
  error: AppIpcError | null
  createEpisode: (args: {
    seriesId: number
    videoId: string
    url: string
    episodeNumber: number | null
    title: string
  }) => Promise<Episode | null>
  updateEpisode: (args: {
    id: number
    seriesId: number
    videoId: string
    url: string
    episodeNumber: number | null
    title: string
    status: string
  }) => Promise<Episode | null>
  deleteEpisode: (id: number) => Promise<boolean>
} {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<AppIpcError | null>(null)

  const run = useCallback(async <T,>(task: () => Promise<T>): Promise<T | null> => {
    setBusy(true)
    setError(null)
    try {
      return await task()
    } catch (caught) {
      setError(toAppIpcError(caught))
      return null
    } finally {
      setBusy(false)
    }
  }, [])

  const create = useCallback(
    (args: {
      seriesId: number
      videoId: string
      url: string
      episodeNumber: number | null
      title: string
    }) =>
      run(() => createEpisode({
        seriesId: args.seriesId,
        videoId: args.videoId,
        url: args.url,
        episodeNumber: args.episodeNumber,
        title: args.title,
      })),
    [run],
  )

  const update = useCallback(
    (args: {
      id: number
      seriesId: number
      videoId: string
      url: string
      episodeNumber: number | null
      title: string
      status: string
    }) =>
      run(() => updateEpisode({
        id: args.id,
        seriesId: args.seriesId,
        videoId: args.videoId,
        url: args.url,
        episodeNumber: args.episodeNumber,
        title: args.title,
        status: args.status,
      })),
    [run],
  )

  const remove = useCallback(
    async (id: number) => (await run(() => deleteEpisode({ id }))) ?? false,
    [run],
  )

  return { busy, error, createEpisode: create, updateEpisode: update, deleteEpisode: remove }
}

export function useBannerPick(): {
  picking: boolean
  error: AppIpcError | null
  pick: (currentPath?: string | null) => Promise<string | null>
} {
  const [picking, setPicking] = useState(false)
  const [error, setError] = useState<AppIpcError | null>(null)

  const pick = useCallback(
    async (currentPath?: string | null): Promise<string | null> => {
      setPicking(true)
      setError(null)
      try {
        return await pickBannerFile({ currentPath })
      } catch (caught) {
        setError(toAppIpcError(caught))
        return null
      } finally {
        setPicking(false)
      }
    },
    [],
  )

  return { picking, error, pick }
}

export function useLegacyImport(): {
  importing: boolean
  summary: ImportSummary | null
  error: AppIpcError | null
  path: string
  setPath: (path: string) => void
  loadPath: () => Promise<void>
  run: () => Promise<void>
} {
  const [importing, setImporting] = useState(false)
  const [summary, setSummary] = useState<ImportSummary | null>(null)
  const [error, setError] = useState<AppIpcError | null>(null)
  const [path, setPath] = useState("")

  const loadPath = useCallback(async () => {
    setPath(await legacyExportPath())
  }, [])

  const run = useCallback(async () => {
    if (!path.trim()) {
      return
    }
    setImporting(true)
    setError(null)
    try {
      setSummary(await importLegacyJson(path.trim()))
    } catch (caught) {
      setError(toAppIpcError(caught))
    } finally {
      setImporting(false)
    }
  }, [path])

  return { importing, summary, error, path, setPath, loadPath, run }
}

/** Cached results of the Home "check for updates" action. Nothing runs until
 * the user asks, so loading the home screen never hits a remote source. */
export function useSeriesUpdates(): {
  updates: SeriesUpdate[]
  checking: boolean
  error: AppIpcError | null
  check: () => Promise<SeriesUpdate[]>
} {
  const [updates, setUpdates] = useState<SeriesUpdate[]>([])
  const [checking, setChecking] = useState(false)
  const [error, setError] = useState<AppIpcError | null>(null)

  const check = useCallback(async () => {
    setChecking(true)
    setError(null)
    try {
      const result = await checkSeriesUpdates()
      setUpdates(result)
      return result
    } catch (caught) {
      setError(toAppIpcError(caught))
      return []
    } finally {
      setChecking(false)
    }
  }, [])

  return { updates, checking, error, check }
}

/** Opt a library entry in or out of update checks. */
export function useUpdatePreference(): {
  busy: boolean
  set: (id: number, enabled: boolean) => Promise<boolean>
} {
  const [busy, setBusy] = useState(false)

  const update = useCallback(async (id: number, enabled: boolean) => {
    setBusy(true)
    try {
      return await setSeriesCheckUpdates(id, enabled)
    } catch {
      return false
    } finally {
      setBusy(false)
    }
  }, [])

  return { busy, set: update }
}

/** Detect a legacy donghua-stream install for the Settings "Legacy" section. */
export function useLegacyDetection(): AsyncResource<LegacyStatus> {
  const load = useCallback(() => legacyStatus(), [])
  return useAsyncResource(load, {
    detected: false,
    configPath: null,
    exportPath: "",
    exportReady: false,
    seriesCount: 0,
    episodeCount: 0,
    binaries: [],
    exportScript: null,
    imported: false,
    importedAt: null,
    note: "Browser preview: legacy import is unavailable.",
  })
}

/** User-triggered pull of un-migrated legacy data into the local catalog. */
export function useLegacyPull(): {
  pulling: boolean
  summary: PullSummary | null
  error: AppIpcError | null
  pull: () => Promise<PullSummary | null>
} {
  const [pulling, setPulling] = useState(false)
  const [summary, setSummary] = useState<PullSummary | null>(null)
  const [error, setError] = useState<AppIpcError | null>(null)

  const pull = useCallback(async () => {
    setPulling(true)
    setError(null)
    try {
      const result = await pullLegacyData()
      setSummary(result)
      return result
    } catch (caught) {
      setError(toAppIpcError(caught))
      return null
    } finally {
      setPulling(false)
    }
  }, [])

  return { pulling, summary, error, pull }
}
