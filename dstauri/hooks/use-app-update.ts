"use client"

import { useCallback, useEffect, useState } from "react"

import {
  listenUpdateAvailable,
  setAppUpdateSettings,
  startAppUpdate,
  type AvailableUpdate,
} from "@/lib/ipc/app-updater"
import { isTauri } from "@/lib/tauri"

const skippedKey = (version: string) => `tsuki.update.skipped.${version}`

/**
 * Drives the global changelog dialog. Rust performs the startup check and
 * emits `update://available`; this hook only owns UI state and local dismissal.
 */
export function useAppUpdate() {
  const [release, setRelease] = useState<AvailableUpdate | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!isTauri()) return
    let active = true
    let unlisten: (() => void) | undefined
    void listenUpdateAvailable((found) => {
      if (!active) return
      if (typeof window !== "undefined" && window.sessionStorage.getItem(skippedKey(found.version))) {
        return
      }
      setRelease(found)
      setError(null)
    })
      .then((stop) => {
        unlisten = stop
        if (!active) stop()
      })
      .catch(() => {
        // An event listener failure must never break the app shell.
      })
    return () => {
      active = false
      unlisten?.()
    }
  }, [])

  const skip = useCallback(() => {
    if (release && typeof window !== "undefined") {
      window.sessionStorage.setItem(skippedKey(release.version), "1")
    }
    setRelease(null)
  }, [release])

  const start = useCallback(async () => {
    if (!release) return
    setBusy(true)
    setError(null)
    try {
      await startAppUpdate(release)
      setRelease(null)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusy(false)
    }
  }, [release])

  const setAutomatic = useCallback(async (autoCheck: boolean, autoDownload: boolean) => {
    setError(null)
    try {
      await setAppUpdateSettings(autoCheck, autoDownload)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    }
  }, [])

  return { release, busy, error, start, skip, setAutomatic }
}
