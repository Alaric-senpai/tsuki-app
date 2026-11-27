"use client"

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react"

/**
 * SSR-safe localStorage state. The server and the first client render both use
 * `initial`; the stored value is read in an effect (no hydration mismatch) and
 * only user-driven changes are written back. Corrupt or invalid values are
 * ignored instead of breaking the page.
 */
export function usePersistedState<T>(
  key: string,
  initial: T,
  validate?: (raw: unknown) => T | null,
): readonly [T, Dispatch<SetStateAction<T>>] {
  const [value, setValue] = useState<T>(initial)
  const changedByUser = useRef(false)

  useEffect(() => {
    if (typeof window === "undefined") return
    try {
      const stored = window.localStorage.getItem(key)
      if (stored !== null) {
        const parsed: unknown = JSON.parse(stored)
        const accepted = validate ? validate(parsed) : (parsed as T)
        if (accepted !== null && accepted !== undefined) {
          // Defer hydration out of the effect body so it cannot cause a
          // cascading synchronous render on mount. A change made immediately
          // after mount (e.g. a ?q= search) wins over the stored value.
          queueMicrotask(() => {
            if (!changedByUser.current) setValue(accepted)
          })
        }
      }
    } catch {
      // Private browsing, quota errors and malformed JSON are non-fatal.
    }
  }, [key, validate])

  useEffect(() => {
    if (!changedByUser.current || typeof window === "undefined") return
    try {
      window.localStorage.setItem(key, JSON.stringify(value))
    } catch {
      // Storage may be unavailable; the in-memory state still works.
    }
  }, [key, value])

  const setPersisted = useCallback<Dispatch<SetStateAction<T>>>((action) => {
    changedByUser.current = true
    setValue(action)
  }, [])

  return [value, setPersisted] as const
}
