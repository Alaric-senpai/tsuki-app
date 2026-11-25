import { afterEach, describe, expect, it, vi } from "vitest"

import { HtmlVideoEngine } from "./html-video"
import type { PlayerEventMap } from "../engine"

describe("HtmlVideoEngine playback errors", () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it("reports NotSupportedError instead of rejecting play()", async () => {
    const play = vi
      .spyOn(HTMLMediaElement.prototype, "play")
      .mockRejectedValue(
        new DOMException("The operation is not supported.", "NotSupportedError"),
      )
    const video = document.createElement("video")
    video.src = "https://example.com/episode.mp4"
    const engine = new HtmlVideoEngine(video)
    const errors: PlayerEventMap["error"][] = []
    const states: string[] = []
    engine.on("error", (event) => errors.push(event))
    engine.on("state", (event) => states.push(event.status))

    await expect(engine.play()).resolves.toBeUndefined()

    expect(play).toHaveBeenCalledOnce()
    expect(errors).toHaveLength(1)
    expect(errors[0]?.message).toContain("Playback is not supported")
    expect(states).toContain("error")
  })

  it("does not call play() when no video is loaded", async () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined)
    const engine = new HtmlVideoEngine(document.createElement("video"))
    const errors: PlayerEventMap["error"][] = []
    engine.on("error", (event) => errors.push(event))

    await expect(engine.play()).resolves.toBeUndefined()

    expect(play).not.toHaveBeenCalled()
    expect(errors[0]?.message).toBe("No video is loaded yet.")
  })
})
