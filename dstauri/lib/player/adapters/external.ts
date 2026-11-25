import { openInExternalPlayer } from "@/lib/ipc/config"

import {
  PlayerEngineUnavailable,
  type PlayableSource,
  type PlayerEngine,
  type PlayerEvent,
  type PlayerEventMap,
  type PlayerStatus,
  type Track,
  type Unsubscribe,
} from "../engine"

/** Hands local files to the user's own player via the Rust IPC command. */
export class ExternalPlayerEngine implements PlayerEngine {
  name: string = "external"

  private readonly listeners = new Map<PlayerEvent, Set<(value: never) => void>>()
  private status: PlayerStatus = "idle"

  private emit<E extends PlayerEvent>(event: E, value: PlayerEventMap[E]): void {
    this.listeners.get(event)?.forEach((cb) => (cb as (v: PlayerEventMap[E]) => void)(value))
  }

  private setState(status: PlayerStatus): void {
    this.status = status
    this.emit("state", { status })
  }

  async load(source: PlayableSource): Promise<void> {
    if (source.kind !== "file") {
      throw new PlayerEngineUnavailable(
        "External player needs a local file.",
        "Streams cannot be handed to an external player from this build.",
      )
    }
    this.setState("loading")
    await openInExternalPlayer(source.path)
    this.emit("ready", { duration: 0 })
    this.setState("playing")
    this.emit("play", undefined)
  }

  async play(): Promise<void> {
    // The external process owns playback; nothing to resume from here.
  }

  async pause(): Promise<void> {
    // The external process owns playback; nothing to pause from here.
  }

  async seek(): Promise<void> {
    // Progress sync for mpv's IPC server is a later refinement.
  }

  setVolume(): void {}
  setRate(): void {}
  audioTracks(): Track[] {
    return []
  }
  subtitleTracks(): Track[] {
    return []
  }
  selectAudio(): void {}
  selectSubtitle(): void {}

  on<E extends PlayerEvent>(event: E, cb: (payload: PlayerEventMap[E]) => void): Unsubscribe {
    const set = this.listeners.get(event) ?? new Set()
    set.add(cb as (value: never) => void)
    this.listeners.set(event, set)
    return () => set.delete(cb as (value: never) => void)
  }

  async dispose(): Promise<void> {
    this.listeners.clear()
    this.setState("idle")
  }
}
