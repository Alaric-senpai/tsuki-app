import { convertFileSrc } from "@tauri-apps/api/core"

import type {
  PlayableSource,
  PlayerEngine,
  PlayerEvent,
  PlayerEventMap,
  PlayerStatus,
  Track,
  Unsubscribe,
} from "../engine"

/**
 * Webview engine: a plain <video> element.
 *
 * On Linux the webview's codec support depends on the user's GStreamer
 * plugins, so this engine reports playback errors instead of trying to hide
 * them. Custom headers cannot be injected into a <video> request — streams
 * that need them must go through the Rust loopback proxy (or be handed to an
 * external player).
 */
export class HtmlVideoEngine implements PlayerEngine {
  readonly name = "html"

  private readonly listeners = new Map<PlayerEvent, Set<(value: never) => void>>()
  private status: PlayerStatus = "idle"
  private disposed = false

  constructor(readonly video: HTMLVideoElement) {
    this.attach()
  }

  private emit<E extends PlayerEvent>(event: E, value: PlayerEventMap[E]): void {
    this.listeners.get(event)?.forEach((cb) => (cb as (v: PlayerEventMap[E]) => void)(value))
  }

  private setState(status: PlayerStatus): void {
    this.status = status
    this.emit("state", { status })
  }

  private attach(): void {
    this.video.addEventListener("loadedmetadata", () => {
      this.setState("ready")
      this.emit("ready", { duration: this.video.duration })
      this.emit("timeupdate", {
        position: this.video.currentTime,
        duration: this.video.duration,
      })
    })
    this.video.addEventListener("timeupdate", () => {
      this.emit("timeupdate", {
        position: this.video.currentTime,
        duration: this.video.duration,
      })
    })
    this.video.addEventListener("play", () => {
      this.setState("playing")
      this.emit("play", undefined)
    })
    this.video.addEventListener("pause", () => {
      if (this.status !== "ended") {
        this.setState("paused")
        this.emit("pause", undefined)
      }
    })
    this.video.addEventListener("ended", () => {
      this.setState("ended")
      this.emit("ended", undefined)
    })
    this.video.addEventListener("error", () => {
      const mediaError = this.video.error
      const message = mediaError
        ? `Media error ${mediaError.code}: ${mediaError.message}`
        : "The video could not be played."
      this.setState("error")
      this.emit("error", { message, detail: this.video.currentSrc })
    })
  }

  async load(source: PlayableSource, opts?: { startAt?: number }): Promise<void> {
    this.setState("loading")
    Array.from(this.video.querySelectorAll("track")).forEach((node) => node.remove())
    for (const sub of source.kind === "stream" ? (source.subtitles ?? []) : []) {
      const track = document.createElement("track")
      track.kind = "subtitles"
      track.label = sub.label
      track.src = sub.url
      track.srclang = sub.language ?? "und"
      this.video.appendChild(track)
    }
    if (source.kind === "file") {
      this.video.src = convertFileSrc(source.path)
    } else {
      if (source.headers && Object.keys(source.headers).length > 0) {
        console.warn(
          "[player] custom headers are not supported by the webview engine; use the stream proxy or an external player",
        )
      }
      this.video.src = source.url
    }
    this.video.load()
    if (opts?.startAt && Number.isFinite(opts.startAt)) {
      this.video.currentTime = opts.startAt
    }
  }

  async play(): Promise<void> {
    if (this.disposed) {
      return
    }
    if (!this.video.src && !this.video.currentSrc) {
      this.setState("error")
      this.emit("error", { message: "No video is loaded yet." })
      return
    }
    try {
      await this.video.play()
    } catch (cause) {
      // WebKitGTK rejects play() with NotSupportedError for codecs its
      // GStreamer build cannot decode. Never let that escape as an unhandled
      // promise rejection from a button click.
      const reason = cause instanceof Error ? cause.message : String(cause)
      const unsupported = cause instanceof DOMException && cause.name === "NotSupportedError"
      this.setState("error")
      this.emit("error", {
        message: unsupported
          ? "Playback is not supported by this browser. Try “Open in my player”."
          : `Playback failed: ${reason}`,
        detail: this.video.currentSrc || this.video.src,
      })
    }
  }

  async pause(): Promise<void> {
    this.video.pause()
  }

  async seek(seconds: number): Promise<void> {
    if (Number.isFinite(seconds)) {
      this.video.currentTime = Math.max(0, seconds)
    }
  }

  setVolume(value: number): void {
    this.video.volume = Math.min(1, Math.max(0, value))
  }

  setRate(value: number): void {
    if (value > 0) this.video.playbackRate = value
  }

  audioTracks(): Track[] {
    // audioTracks is not implemented in WebKitGTK; fall back to a single
    // implicit track so the UI can still render the menu.
    const list = (
      this.video as HTMLVideoElement & {
        audioTracks?: {
          length: number
          [index: number]: { id: string; label: string; language: string; enabled: boolean }
        }
      }
    ).audioTracks
    if (!list) return [{ id: "default", label: "Default", language: "" }]
    return Array.from({ length: list.length }, (_, i): Track => {
      const track = list[i]
      return {
        id: track.id || String(i),
        label: track.label || `Track ${i + 1}`,
        language: track.language ?? "",
      }
    })
  }

  subtitleTracks(): Track[] {
    const tracks: Track[] = Array.from(this.video.textTracks).map((track, i) => ({
      id: track.id || String(i),
      label: track.label || `Subtitles ${i + 1}`,
      language: track.language ?? "",
    }))
    return [{ id: "off", label: "Off", language: "" }, ...tracks]
  }

  selectAudio(id: string): void {
    const list = (
      this.video as HTMLVideoElement & {
        audioTracks?: {
          length: number
          [index: number]: { id: string; enabled: boolean }
        }
      }
    ).audioTracks
    if (!list) return
    for (let i = 0; i < list.length; i += 1) {
      list[i].enabled = list[i].id === id
    }
  }

  selectSubtitle(id: string | null): void {
    for (const track of Array.from(this.video.textTracks)) {
      track.mode = track.id === id && id !== "off" ? "showing" : "hidden"
    }
  }

  on<E extends PlayerEvent>(event: E, cb: (payload: PlayerEventMap[E]) => void): Unsubscribe {
    const set = this.listeners.get(event) ?? new Set()
    set.add(cb as (value: never) => void)
    this.listeners.set(event, set)
    return () => set.delete(cb as (value: never) => void)
  }

  async dispose(): Promise<void> {
    if (this.disposed) return
    this.disposed = true
    this.video.pause()
    this.video.removeAttribute("src")
    this.video.load()
    this.listeners.clear()
  }
}
