import { describe, expect, it } from "vitest"

import {
  applyClearedIds,
  applyDownloadState,
  applyRemovedId,
} from "./download-events"
import type { DownloadSnapshot } from "./downloads"

const row = (episodeId: number, state = "done"): DownloadSnapshot => ({
  episodeId,
  seriesId: 1,
  title: `Episode ${episodeId}`,
  state,
  attempt: 0,
  maxRetries: 3,
  downloadedBytes: 100,
  totalBytes: 100,
  speedBps: 0,
  etaSec: 0,
  currentFormat: "",
  error: null,
  filePath: null,
})

describe("download queue event helpers", () => {
  it("drops a row when the backend reports it removed", () => {
    const current = { 7: row(7), 9: row(9, "queued") }
    expect(applyRemovedId(current, 7)).toEqual({ 9: row(9, "queued") })
  })

  it("drops every cleared row and its console lines", () => {
    const current = { 7: row(7), 8: row(8, "failed"), 9: row(9, "queued") }
    expect(applyClearedIds(current, [7, 8])).toEqual({ 9: row(9, "queued") })
  })

  it("ignores late state events for cleared rows until they are restarted", () => {
    const cleared = new Set([7])
    expect(applyDownloadState({}, row(7), cleared)).toEqual({})
    cleared.delete(7)
    expect(applyDownloadState({}, row(7, "queued"), cleared)).toEqual({
      7: row(7, "queued"),
    })
  })
})
