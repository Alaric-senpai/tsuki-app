import { describe, expect, it } from "vitest"

import {
  isResumable,
  isWatched,
  WATCHED_THRESHOLD,
  type WatchProgress,
} from "@/lib/ipc/watch-progress"

function progress(overrides: Partial<WatchProgress> = {}): WatchProgress {
  return {
    episodeId: 1,
    positionSec: 300,
    durationSec: 1440,
    updatedAt: "2026-01-01T00:00:00Z",
    completed: false,
    ...overrides,
  }
}

describe("isWatched", () => {
  it("requires a usable duration", () => {
    expect(isWatched(10, 0)).toBe(false)
    expect(isWatched(10, Number.NaN)).toBe(false)
  })

  it("counts playback at or past the threshold", () => {
    expect(isWatched(1440 * WATCHED_THRESHOLD - 1, 1440)).toBe(false)
    expect(isWatched(1440 * WATCHED_THRESHOLD, 1440)).toBe(true)
    expect(isWatched(1440, 1440)).toBe(true)
  })
})

describe("isResumable", () => {
  it("ignores missing and completed rows", () => {
    expect(isResumable(null)).toBe(false)
    expect(isResumable(undefined)).toBe(false)
    expect(isResumable(progress({ completed: true }))).toBe(false)
  })

  it("ignores positions too close to the start", () => {
    expect(isResumable(progress({ positionSec: 4 }))).toBe(false)
    expect(isResumable(progress({ positionSec: 5 }))).toBe(true)
  })

  it("ignores positions inside the credits", () => {
    expect(isResumable(progress({ positionSec: 1440 * WATCHED_THRESHOLD }))).toBe(false)
    expect(isResumable(progress({ positionSec: 1000 }))).toBe(true)
  })

  it("trusts the position when the duration is unknown", () => {
    expect(isResumable(progress({ durationSec: 0, positionSec: 42 }))).toBe(true)
    expect(isResumable(progress({ durationSec: 0, positionSec: 1 }))).toBe(false)
  })
})
