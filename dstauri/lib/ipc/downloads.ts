import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { isTauri } from "@/lib/tauri"

import { ipcCall } from "./client"

/** Lifecycle states emitted by the Rust download queue. */
export type DownloadPhase =
  | "queued"
  | "resolving"
  | "downloading"
  | "retrying"
  | "done"
  | "failed"
  | "cancelled"

export interface DownloadSnapshot {
  episodeId: number
  seriesId: number
  title: string
  state: DownloadPhase | string
  attempt: number
  maxRetries: number
  downloadedBytes: number
  totalBytes: number
  speedBps: number
  etaSec: number
  currentFormat: string
  error: string | null
  filePath: string | null
}

export interface DownloadConsoleLine {
  episodeId: number
  line: string
}

export const EMPTY_DOWNLOAD_SNAPSHOT: DownloadSnapshot = {
  episodeId: 0,
  seriesId: 0,
  title: "",
  state: "queued",
  attempt: 0,
  maxRetries: 0,
  downloadedBytes: 0,
  totalBytes: 0,
  speedBps: 0,
  etaSec: 0,
  currentFormat: "",
  error: null,
  filePath: null,
}

export function enqueueEpisodeDownload(
  episodeId: number,
): Promise<DownloadSnapshot> {
  return ipcCall<DownloadSnapshot>(
    "enqueue_episode_download",
    EMPTY_DOWNLOAD_SNAPSHOT,
    { episodeId },
  )
}

export function cancelEpisodeDownload(episodeId: number): Promise<void> {
  return ipcCall<void>("cancel_episode_download", undefined, { episodeId })
}

/**
 * Stop a job (if running) and drop its queue row entirely. Unlike cancel, the
 * row disappears from the queue list; enqueueing again re-adds it.
 */
export function removeFromQueue(episodeId: number): Promise<boolean> {
  return ipcCall<boolean>("remove_from_queue", false, { episodeId })
}

/** Requeue every episode the catalog still marks as `queued` (e.g. after a restart). */
export function resumeDownloads(): Promise<number> {
  return ipcCall<number>("resume_downloads", 0)
}

export function downloadQueue(): Promise<DownloadSnapshot[]> {
  return ipcCall<DownloadSnapshot[]>("download_queue", [])
}

/**
 * Drop finished jobs from the queue. Running and queued jobs are untouched.
 * Returns the episode ids that were cleared.
 */
export function clearDownloadQueue(): Promise<number[]> {
  return ipcCall<number[]>("clear_download_queue", [])
}

export function downloadState(
  episodeId: number,
): Promise<DownloadSnapshot | null> {
  return ipcCall<DownloadSnapshot | null>("download_state", null, { episodeId })
}

/** Finished file path from the catalog (survives restarts and queue clears). */
export function downloadedEpisodePath(
  episodeId: number,
): Promise<string | null> {
  return ipcCall<string | null>("downloaded_episode_path", null, { episodeId })
}

/** Live queue state, emitted on every progress tick. */
export async function listenDownloadState(
  handler: (snapshot: DownloadSnapshot) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => {}
  }
  return listen<DownloadSnapshot>("download://state", (event) =>
    handler(event.payload),
  )
}

/** yt-dlp console lines, including the error tail of a failed attempt. */
export async function listenDownloadConsole(
  handler: (line: DownloadConsoleLine) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => {}
  }
  return listen<DownloadConsoleLine>("download://console", (event) =>
    handler(event.payload),
  )
}

/** Change how many downloads run at once (persisted, 1–16). */
export function setDownloadConcurrency(limit: number): Promise<number> {
  return ipcCall<number>("set_download_concurrency", 3, { limit })
}

/** Catalog rows changed somewhere (download finished, import, cancel…). */
export async function listenLibraryChanged(
  handler: () => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => {}
  }
  return listen<null>("library://changed", () => handler())
}
