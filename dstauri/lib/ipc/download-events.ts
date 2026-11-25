import type { DownloadSnapshot } from "./downloads"

export type SnapshotMap = Record<number, DownloadSnapshot>

function withoutIds<T>(
  current: Record<number, T>,
  ids: Iterable<number>,
): Record<number, T> {
  const dropped = new Set(ids)
  if (dropped.size === 0) {
    return current
  }
  return Object.fromEntries(
    Object.entries(current).filter(([id]) => !dropped.has(Number(id))),
  )
}

/** Remove one row after the backend emits `download://removed`. */
export function applyRemovedId<T>(
  current: Record<number, T>,
  episodeId: number,
): Record<number, T> {
  return withoutIds(current, [episodeId])
}

/** Remove every row returned by `clear_download_queue`. */
export function applyClearedIds<T>(
  current: Record<number, T>,
  ids: number[],
): Record<number, T> {
  return withoutIds(current, ids)
}

/**
 * Apply a `download://state` event while respecting ids the user cleared.
 * A worker can emit a final state just before the removal reaches the webview;
 * without this guard that event would resurrect a row the user just cleared.
 * Restarting an id removes it from `cleared`, so new downloads show normally.
 */
export function applyDownloadState(
  current: SnapshotMap,
  snapshot: DownloadSnapshot,
  cleared: ReadonlySet<number>,
): SnapshotMap {
  if (cleared.has(snapshot.episodeId)) {
    return current
  }
  return { ...current, [snapshot.episodeId]: snapshot }
}
