import { ipcCall } from "./client"

/** Saved playback position for one episode. */
export interface WatchProgress {
  episodeId: number
  positionSec: number
  durationSec: number
  updatedAt: string
  completed: boolean
}

/** Fraction of the runtime after which an episode counts as watched. */
export const WATCHED_THRESHOLD = 0.9

/** True when the saved row is worth offering as "resume". */
export function isResumable(progress: WatchProgress | null | undefined): boolean {
  if (!progress || progress.completed) return false
  if (progress.positionSec < 5) return false
  if (progress.durationSec <= 0) return progress.positionSec >= 5
  return progress.positionSec / progress.durationSec < WATCHED_THRESHOLD
}

/** True when playback has reached the watched threshold. */
export function isWatched(positionSec: number, durationSec: number): boolean {
  if (!Number.isFinite(durationSec) || durationSec <= 0) return false
  return positionSec / durationSec >= WATCHED_THRESHOLD
}

/** Persist the playback position of an episode. */
export function saveWatchProgress(
  episodeId: number,
  positionSec: number,
  durationSec: number,
  completed: boolean,
): Promise<boolean> {
  return ipcCall<boolean>("save_watch_progress", false, {
    episodeId,
    positionSec,
    durationSec,
    completed,
  })
}

/** Read the saved playback position of an episode. */
export function getWatchProgress(episodeId: number): Promise<WatchProgress | null> {
  return ipcCall<WatchProgress | null>("get_watch_progress", null, { episodeId })
}

/** Forget the saved position of an episode. */
export function clearWatchProgress(episodeId: number): Promise<boolean> {
  return ipcCall<boolean>("clear_watch_progress", false, { episodeId })
}

/** Every saved position, newest first. */
export function allWatchProgress(): Promise<WatchProgress[]> {
  return ipcCall<WatchProgress[]>("all_watch_progress", [])
}
