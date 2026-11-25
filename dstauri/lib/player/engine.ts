/**
 * Engine-agnostic player contract.
 *
 * UI components never touch `<video>`, ReactPlayer, or a sidecar directly —
 * they talk to this interface. Concrete adapters live in `./adapters`.
 */

export interface SubtitleTrack {
  url: string
  label: string
  language?: string
}

export type PlayableSource =
  | { kind: "file"; path: string; title?: string }
  | {
      kind: "stream"
      url: string
      title?: string
      headers?: Record<string, string>
      subtitles?: SubtitleTrack[]
      isHls?: boolean
    }

export interface Track {
  id: string
  label: string
  language?: string
}

export type PlayerStatus =
  | "idle"
  | "loading"
  | "ready"
  | "playing"
  | "paused"
  | "ended"
  | "error"

export interface PlayerEventMap {
  ready: { duration: number }
  timeupdate: { position: number; duration: number }
  play: undefined
  pause: undefined
  ended: undefined
  error: { message: string; detail?: string }
  state: { status: PlayerStatus }
  tracks: { audio: Track[]; subtitle: Track[] }
}

export type PlayerEvent = keyof PlayerEventMap
export type Unsubscribe = () => void

export interface PlayerEngine {
  /** Stable identifier shown in the engine badge. */
  readonly name: string
  load(source: PlayableSource, opts?: { startAt?: number }): Promise<void>
  play(): Promise<void>
  pause(): Promise<void>
  seek(seconds: number): Promise<void>
  setVolume(volume: number): void
  setRate(rate: number): void
  audioTracks(): Track[]
  subtitleTracks(): Track[]
  selectAudio(id: string): void
  selectSubtitle(id: string | null): void
  on<E extends PlayerEvent>(
    event: E,
    callback: (payload: PlayerEventMap[E]) => void,
  ): Unsubscribe
  dispose(): Promise<void>
}

export type EnginePreference = "auto" | "mpv" | "html" | "external"

export class PlayerEngineUnavailable extends Error {
  readonly detail: string

  constructor(message: string, detail = "") {
    super(message)
    this.name = "PlayerEngineUnavailable"
    this.detail = detail
  }
}

/** Format seconds as `m:ss` / `h:mm:ss` without pulling in a date library. */
export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00"
  const total = Math.floor(seconds)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const secs = total % 60
  const pad = (value: number) => String(value).padStart(2, "0")
  return hours > 0
    ? `${hours}:${pad(minutes)}:${pad(secs)}`
    : `${minutes}:${pad(secs)}`
}
