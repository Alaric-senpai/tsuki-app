import { PlayerEngineUnavailable } from "../engine"
import { ExternalPlayerEngine } from "./external"

/**
 * Embedded mpv is deliberately not shipped in v1 — see docs/decisions/player.md.
 * The preference still exists so old configs keep working; it degrades to the
 * external hand-off with a visible, non-blocking notice.
 */
export class MpvEngine extends ExternalPlayerEngine {
  readonly name = "mpv"

  constructor() {
    super()
    throw new PlayerEngineUnavailable(
      "Embedded mpv is not available in this build.",
      "Tsuki uses the built-in webview player. Install mpv and choose the External engine to play in your own player.",
    )
  }
}
