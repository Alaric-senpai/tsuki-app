"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"

import { toAppIpcError, type AppIpcError } from "@/lib/ipc/client"
import {
  cancelEpisodeDownload,
  clearDownloadQueue,
  downloadQueue,
  enqueueEpisodeDownload,
  listenDownloadConsole,
  listenDownloadState,
  removeFromQueue,
  type DownloadSnapshot,
} from "@/lib/ipc/downloads"

const MAX_CONSOLE_LINES = 120

export interface DownloadQueue {
  rows: DownloadSnapshot[]
  consoleLines: Record<number, string[]>
  loading: boolean
  error: AppIpcError | null
  start: (episodeId: number) => Promise<DownloadSnapshot>
  cancel: (episodeId: number) => Promise<void>
  /** Stop a job (if running) and drop its row entirely. */
  remove: (episodeId: number) => Promise<boolean>
  clear: () => Promise<void>
  clearError: () => void
}

/**
 * Live download queue: the initial snapshot comes from `download_queue`, then
 * `download://state` / `download://console` events keep it current.
 */
export function useDownloadQueue(): DownloadQueue {
  const [snapshots, setSnapshots] = useState<Record<number, DownloadSnapshot>>({})
  const [consoleLines, setConsoleLines] = useState<Record<number, string[]>>({})
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<AppIpcError | null>(null)
  const mounted = useRef(true)

  useEffect(() => {
    mounted.current = true
    let unlistenState: UnlistenFn | undefined
    let unlistenConsole: UnlistenFn | undefined

    downloadQueue()
      .then((rows) => {
        if (!mounted.current) {
          return
        }
        setSnapshots(
          Object.fromEntries(rows.map((row) => [row.episodeId, row])),
        )
      })
      .catch((cause: unknown) => {
        if (mounted.current) {
          setError(toAppIpcError(cause))
        }
      })
      .finally(() => {
        if (mounted.current) {
          setLoading(false)
        }
      })

    void listenDownloadState((snapshot) => {
      setSnapshots((current) => ({ ...current, [snapshot.episodeId]: snapshot }))
    }).then((unlisten) => {
      unlistenState = unlisten
    })

    void listenDownloadConsole((line) => {
      setConsoleLines((current) => {
        const existing = current[line.episodeId] ?? []
        return {
          ...current,
          [line.episodeId]: [...existing, line.line].slice(-MAX_CONSOLE_LINES),
        }
      })
    }).then((unlisten) => {
      unlistenConsole = unlisten
    })

    return () => {
      mounted.current = false
      unlistenState?.()
      unlistenConsole?.()
    }
  }, [])

  const start = useCallback(async (episodeId: number) => {
    try {
      const snapshot = await enqueueEpisodeDownload(episodeId)
      setSnapshots((current) => ({ ...current, [episodeId]: snapshot }))
      return snapshot
    } catch (cause) {
      const appError = toAppIpcError(cause)
      setError(appError)
      throw appError
    }
  }, [])

  const cancel = useCallback(async (episodeId: number) => {
    try {
      await cancelEpisodeDownload(episodeId)
    } catch (cause) {
      setError(toAppIpcError(cause))
      throw toAppIpcError(cause)
    }
  }, [])

  const remove = useCallback(async (episodeId: number) => {
    try {
      const known = await removeFromQueue(episodeId)
      setSnapshots((current) => {
        if (!(episodeId in current)) {
          return current
        }
        const next = { ...current }
        delete next[episodeId]
        return next
      })
      setConsoleLines((current) => {
        if (!(episodeId in current)) {
          return current
        }
        const next = { ...current }
        delete next[episodeId]
        return next
      })
      return known
    } catch (cause) {
      setError(toAppIpcError(cause))
      throw toAppIpcError(cause)
    }
  }, [])

  const clear = useCallback(async () => {
    try {
      const cleared = await clearDownloadQueue()
      const dropped = new Set(cleared)
      setSnapshots((current) =>
        Object.fromEntries(
          Object.entries(current).filter(([id]) => !dropped.has(Number(id))),
        ),
      )
      setConsoleLines((current) =>
        Object.fromEntries(
          Object.entries(current).filter(([id]) => !dropped.has(Number(id))),
        ),
      )
    } catch (cause) {
      setError(toAppIpcError(cause))
      throw toAppIpcError(cause)
    }
  }, [])

  const clearError = useCallback(() => setError(null), [])

  const rows = useMemo(
    () =>
      Object.values(snapshots).sort((left, right) => left.episodeId - right.episodeId),
    [snapshots],
  )

  return {
    rows,
    consoleLines,
    loading,
    error,
    start,
    cancel,
    remove,
    clear,
    clearError,
  }
}
