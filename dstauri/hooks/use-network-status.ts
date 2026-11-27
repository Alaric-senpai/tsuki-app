"use client"

import { useEffect, useRef, useState } from "react"

import {
  listenNetworkSignals,
  listenNetworkStatus,
  networkStatus,
} from "@/lib/ipc/network"
import { isTauri } from "@/lib/tauri"

/**
 * Connectivity state seeded from Rust and kept live by `network://…` events.
 * `lastChanged` is the timestamp of the first real transition (never the seed),
 * or `null` while there has been none.
 */
export function useNetworkStatus() {
  const [online, setOnline] = useState(true)
  const [lastChanged, setLastChanged] = useState<number | null>(null)
  const current = useRef<boolean | null>(null)

  useEffect(() => {
    if (!isTauri()) return
    let active = true
    const stops: Array<() => void> = []

    const apply = (value: boolean, recordChange: boolean) => {
      if (current.current === value) return
      const first = current.current === null
      current.current = value
      setOnline(value)
      if (!first && recordChange) setLastChanged(Date.now())
    }

    void networkStatus()
      .then((value) => {
        if (active) apply(value, false)
      })
      .catch(() => {
        // Keep the optimistic default; events can still correct it later.
      })

    void listenNetworkStatus((status) => {
      if (active) apply(status.online, true)
    })
      .then((stop) => {
        if (active) stops.push(stop)
        else stop()
      })
      .catch(() => {
        // A listener failure must never break the app shell.
      })

    void listenNetworkSignals((status) => {
      if (active) apply(status.online, true)
    })
      .then((unlisteners) => {
        for (const stop of unlisteners) {
          if (active) stops.push(stop)
          else stop()
        }
      })
      .catch(() => {
        // A listener failure must never break the app shell.
      })

    return () => {
      active = false
      for (const stop of stops) stop()
    }
  }, [])

  return { online, lastChanged }
}
