import { createElement } from "react"
import { renderToString } from "react-dom/server"
import { act, cleanup, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it } from "vitest"

import { parseLibraryFilter } from "@/components/features/library/library-view"

import { usePersistedState } from "./use-persisted-state"

const KEY = "tsuki.library.filter"

beforeEach(() => window.localStorage.clear())
afterEach(cleanup)

describe("usePersistedState", () => {
  it("renders the fallback on the server without touching localStorage", () => {
    window.localStorage.setItem(KEY, JSON.stringify("Failed"))
    function Probe() {
      const [value] = usePersistedState(KEY, "All", parseLibraryFilter)
      return createElement("span", null, value)
    }
    expect(renderToString(createElement(Probe))).toContain("All")
  })

  it("restores a valid stored value after mounting", async () => {
    window.localStorage.setItem(KEY, JSON.stringify("Watching"))
    const { result } = renderHook(() =>
      usePersistedState(KEY, "All", parseLibraryFilter),
    )
    await act(async () => {})
    expect(result.current[0]).toBe("Watching")
    expect(window.localStorage.getItem(KEY)).toBe(JSON.stringify("Watching"))
  })

  it("writes changes back to the same key", async () => {
    const { result } = renderHook(() =>
      usePersistedState(KEY, "All", parseLibraryFilter),
    )
    await act(async () => result.current[1]("Failed"))
    expect(result.current[0]).toBe("Failed")
    expect(window.localStorage.getItem(KEY)).toBe(JSON.stringify("Failed"))
  })

  it("ignores malformed JSON and values rejected by the validator", async () => {
    window.localStorage.setItem(KEY, "{nope")
    const malformed = renderHook(() => usePersistedState(KEY, "All", parseLibraryFilter))
    await act(async () => {})
    expect(malformed.result.current[0]).toBe("All")

    cleanup()
    window.localStorage.setItem(KEY, JSON.stringify("Not a filter"))
    const invalid = renderHook(() => usePersistedState(KEY, "All", parseLibraryFilter))
    await act(async () => {})
    expect(invalid.result.current[0]).toBe("All")
  })

  it("falls back for a fresh key without writing a default on mount", async () => {
    const { result } = renderHook(() =>
      usePersistedState(KEY, "All", parseLibraryFilter),
    )
    await act(async () => {})
    expect(result.current[0]).toBe("All")
    expect(window.localStorage.getItem(KEY)).toBeNull()
  })
})
