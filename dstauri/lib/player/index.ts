//! Engine selection. The UI only talks to `PlayerEngine`; this is the single
//! place that knows which concrete adapter backs a preference.

import type { EnginePreference, PlayerEngine } from "./engine"
import { ExternalPlayerEngine } from "./adapters/external"
import { HtmlVideoEngine } from "./adapters/html-video"
import { MpvEngine } from "./adapters/mpv"

export * from "./engine"

/// Build the best engine for a preference and a mounted <video> element.
/// `auto` and `html` both use the webview; mpv falls back to the external
/// hand-off when it is not bundled (see docs/decisions/player.md).
export function createEngine(
  preference: EnginePreference,
  video?: HTMLVideoElement | null,
): PlayerEngine {
  switch (preference) {
    case "external":
      return new ExternalPlayerEngine()
    case "mpv":
      try {
        return new MpvEngine()
      } catch {
        return new ExternalPlayerEngine()
      }
    case "html":
    case "auto":
    default:
      if (!video) {
        throw new Error("The HTML player needs a mounted video element.")
      }
      return new HtmlVideoEngine(video)
  }
}
