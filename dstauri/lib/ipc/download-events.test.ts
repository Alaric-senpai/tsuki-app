import { describe, expect, it } from "vitest"

import { isBulkRemovable, partitionBulkRemovable } from "./download-events"

describe("bulk queue helpers", () => {
  it("keeps only running phases", () => {
    for (const state of ["resolving", "downloading", "retrying"]) {
      expect(isBulkRemovable(state)).toBe(false)
    }
    for (const state of ["queued", "paused", "done", "failed", "cancelled", "idle"]) {
      expect(isBulkRemovable(state)).toBe(true)
    }
  })

  it("partitions rows for unqueue and cancel-all", () => {
    const rows = [
      { episodeId: 1, state: "downloading" },
      { episodeId: 2, state: "queued" },
      { episodeId: 3, state: "done" },
      { episodeId: 4, state: "retrying" },
    ]
    expect(partitionBulkRemovable(rows)).toEqual({ removable: [2, 3], running: [1, 4] })
  })
})
