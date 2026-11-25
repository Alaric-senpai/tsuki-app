import { ipcCall } from "./client"

export interface SeriesSummary {
  id: number
  key: string
  name: string
  saveDir: string
  description: string
  bannerUrl: string | null
  bannerSource: string
  createdAt: string
  updatedAt: string
  totalEpisodes: number
  downloadedEpisodes: number
  pendingEpisodes: number
  failedEpisodes: number
  queuedEpisodes: number
  /** Where the entry was imported from; empty for manual entries. */
  sourceKey: string
  /** Canonical remote page, used when re-checking for new episodes. */
  sourceUrl: string
  /** Whether the Home updates check re-resolves this entry. */
  checkUpdates: boolean
  /** Episodes missing remotely at the last check. */
  updateCount: number
  lastCheckedAt: string | null
}

export interface UpdateSeriesDetailsArgs {
  id: number
  name: string
  saveDir: string
  description: string
}

export interface SetSeriesBannerArgs {
  id: number
  sourcePath: string
}

export const EMPTY_SERIES: SeriesSummary = {
  id: 0,
  key: "",
  name: "Series",
  saveDir: "",
  description: "",
  bannerUrl: null,
  bannerSource: "",
  createdAt: "",
  updatedAt: "",
  totalEpisodes: 0,
  downloadedEpisodes: 0,
  pendingEpisodes: 0,
  failedEpisodes: 0,
  queuedEpisodes: 0,
  sourceKey: "",
  sourceUrl: "",
  checkUpdates: false,
  updateCount: 0,
  lastCheckedAt: null,
}

export interface Episode {
  id: number
  seriesId: number
  videoId: string
  url: string
  episodeNumber: number | null
  title: string
  status: string
  downloadedAt: string | null
  createdAt: string
}

export interface CreateEpisodeArgs {
  seriesId: number
  videoId: string
  url: string
  episodeNumber: number | null
  title: string
}

export interface UpdateEpisodeArgs {
  id: number
  seriesId: number
  videoId: string
  url: string
  episodeNumber: number | null
  title: string
  status: string
}

export interface DeleteEpisodeArgs {
  id: number
}

export interface PickBannerFileArgs {
  currentPath?: string | null
}

export interface EpisodeWithSeries extends Episode {
  seriesName: string
}

export interface SeriesPoster {
  seriesId: number
  videoId: string
}

export interface LibraryStats {
  seriesCount: number
  episodeCount: number
  downloadedCount: number
  pendingCount: number
  queuedCount: number
  failedCount: number
}

export const EMPTY_LIBRARY_STATS: LibraryStats = {
  seriesCount: 0,
  episodeCount: 0,
  downloadedCount: 0,
  pendingCount: 0,
  queuedCount: 0,
  failedCount: 0,
}

export function listSeries(): Promise<SeriesSummary[]> {
  return ipcCall<SeriesSummary[]>("list_series", [])
}

export function getSeries(query: string): Promise<SeriesSummary | null> {
  return ipcCall<SeriesSummary | null>("get_series", null, { query })
}

export function listEpisodes(
  seriesId: number,
  status?: string,
): Promise<Episode[]> {
  return ipcCall<Episode[]>("list_episodes", [], {
    seriesId,
    status: status ?? null,
  })
}

export function recentEpisodes(
  status?: string,
  limit = 50,
): Promise<EpisodeWithSeries[]> {
  return ipcCall<EpisodeWithSeries[]>("recent_episodes", [], {
    status: status ?? null,
    limit,
  })
}

export function seriesPosters(): Promise<SeriesPoster[]> {
  return ipcCall<SeriesPoster[]>("series_posters", [])
}

export function libraryStats(): Promise<LibraryStats> {
  return ipcCall<LibraryStats>("library_stats", EMPTY_LIBRARY_STATS)
}

export interface SeriesUpdate {
  seriesId: number
  seriesName: string
  /** Episodes on the source page the library does not know about yet. */
  newEpisodes: number
  /** Set when the source page could not be read. */
  error: string | null
}

/** Re-check every library entry that opted into update checks. */
export function checkSeriesUpdates(): Promise<SeriesUpdate[]> {
  return ipcCall<SeriesUpdate[]>("check_series_updates", [])
}

/** Opt a library entry in or out of the Home updates check. */
export function setSeriesCheckUpdates(
  id: number,
  enabled: boolean,
): Promise<boolean> {
  return ipcCall<boolean>("set_series_check_updates", false, {
    id,
    enabled,
  })
}

export function syncArchives(): Promise<number> {
  return ipcCall<number>("sync_archives", 0)
}

export function updateSeriesDetails(
  args: UpdateSeriesDetailsArgs,
): Promise<SeriesSummary> {
  return ipcCall<SeriesSummary>("update_series_details", EMPTY_SERIES, {
    id: args.id,
    name: args.name,
    description: args.description,
    // Both spellings are sent so the wrapper works with the camelCase IPC
    // contract and the current snake_case Rust command signature.
    saveDir: args.saveDir,
    save_dir: args.saveDir,
  })
}

export function setSeriesBanner(args: SetSeriesBannerArgs): Promise<SeriesSummary> {
  return ipcCall<SeriesSummary>("set_series_banner", EMPTY_SERIES, {
    id: args.id,
    sourcePath: args.sourcePath,
    source_path: args.sourcePath,
  })
}

export function resyncSeriesBanners(): Promise<number> {
  return ipcCall<number>("resync_series_banners", 0)
}

export function pickBannerFile(
  args: PickBannerFileArgs = {},
): Promise<string> {
  return ipcCall<string>("pick_banner_file", "", {
    currentPath: args.currentPath ?? null,
  })
}

export function createEpisode(
  args: CreateEpisodeArgs,
): Promise<Episode> {
  return ipcCall<Episode>("create_episode", EMPTY_EPISODE, {
    seriesId: args.seriesId,
    videoId: args.videoId,
    url: args.url,
    episodeNumber: args.episodeNumber,
    title: args.title,
  })
}

export function updateEpisode(
  args: UpdateEpisodeArgs,
): Promise<Episode> {
  return ipcCall<Episode>("update_episode", EMPTY_EPISODE, {
    id: args.id,
    seriesId: args.seriesId,
    videoId: args.videoId,
    url: args.url,
    episodeNumber: args.episodeNumber,
    title: args.title,
    status: args.status,
  })
}

export function deleteEpisode(
  args: DeleteEpisodeArgs,
): Promise<void> {
  return ipcCall<void>("delete_episode", undefined, { id: args.id })
}

export const EMPTY_EPISODE: Episode = {
  id: 0,
  seriesId: 0,
  videoId: "",
  url: "",
  episodeNumber: null,
  title: "",
  status: "ready",
  downloadedAt: null,
  createdAt: "",
}
