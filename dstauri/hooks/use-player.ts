"use client"

import { useCallback, useEffect, useRef, useState } from "react"

import { createEngine } from "@/lib/player"
import {
  PlayerEngineUnavailable,
  type EnginePreference,
  type PlayerEngine,
  type PlayerEvent,
  type PlayerEventMap,
  type PlayerStatus,
  type PlayableSource,
  type Track,
  type Unsubscribe,
} from "@/lib/player/engine"

export interface UsePlayerResult {
  engine: PlayerEngine | null
  on: <E extends PlayerEvent>(
    event: E,
    callback: (payload: PlayerEventMap[E]) => void,
  ) => Unsubscribe
  status: PlayerStatus
  currentTime: number
  duration: number
  volume: number
  rate: number
  audioTracks: Track[]
  subtitleTracks: Track[]
  audioTrack: string | null
  subtitleTrack: string | null
  notice: string | null
  error: string | null
  load: (source: PlayableSource, startAt?: number) => Promise<void>
  toggle: () => Promise<void>
  seek: (seconds: number) => Promise<void>
  setVolume: (volume: number) => void
  setRate: (rate: number) => void
  selectAudio: (id: string) => void
  selectSubtitle: (id: string | null) => void
}

/** Owns one engine instance and keeps React state in sync with its events. */
export function usePlayer(
  video: HTMLVideoElement | null,
  preference: EnginePreference,
): UsePlayerResult {
  const engineRef = useRef<PlayerEngine | null>(null)
  const [engine, setEngine] = useState<PlayerEngine | null>(null)
  const [status, setStatus] = useState<PlayerStatus>("idle")
  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [volume, setVolumeState] = useState(1)
  const [rate, setRateState] = useState(1)
  const [audioTracks, setAudioTracks] = useState<Track[]>([])
  const [subtitleTracks, setSubtitleTracks] = useState<Track[]>([])
  const [audioTrack, setAudioTrack] = useState<string | null>(null)
  const [subtitleTrack, setSubtitleTrack] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!video) {
      return
    }
    let instance: PlayerEngine
    try {
      instance = createEngine(preference, video)
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause)
      const detail = cause instanceof PlayerEngineUnavailable ? cause.detail || null : null
      // Defer state updates out of the effect body (react-hooks/set-state-in-effect).
      queueMicrotask(() => {
        setError(message)
        setNotice(detail)
      })
      return
    }
    engineRef.current = instance
    queueMicrotask(() => setEngine(instance))
    const unsubscribers = [
      instance.on("ready", ({ duration: value }) => {
        setDuration(value)
        setStatus("ready")
      }),
      instance.on("timeupdate", (event) => {
        setCurrentTime(event.position)
        if (Number.isFinite(event.duration)) {
          setDuration(event.duration)
        }
      }),
      instance.on("state", (event) => setStatus(event.status)),
      instance.on("tracks", (event) => {
        setAudioTracks(event.audio)
        setSubtitleTracks(event.subtitle)
      }),
      instance.on("error", (event) => setError(event.message)),
    ]
    return () => {
      unsubscribers.forEach((unsubscribe) => unsubscribe())
      void instance.dispose()
      engineRef.current = null
      setEngine(null)
    }
  }, [video, preference])

  const on = useCallback(
    <E extends PlayerEvent>(event: E, callback: (payload: PlayerEventMap[E]) => void): Unsubscribe =>
      engineRef.current?.on(event, callback) ?? (() => {}),
    [],
  )

  const load = useCallback(async (source: PlayableSource, startAt?: number) => {
    const instance = engineRef.current
    if (!instance) {
      setError("The player engine is not ready yet.")
      return
    }
    setError(null)
    try {
      await instance.load(source, { startAt })
      if (startAt !== undefined && startAt > 0) {
        await instance.seek(startAt)
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    }
  }, [])

  const toggle = useCallback(async () => {
    const instance = engineRef.current
    if (!instance) return
    if (status === "playing") await instance.pause()
    else await instance.play()
  }, [status])

  const seek = useCallback(async (seconds: number) => {
    const instance = engineRef.current
    if (!instance) return
    await instance.seek(seconds)
    setCurrentTime(seconds)
  }, [])

  const setVolume = useCallback((value: number) => {
    const clamped = Math.min(1, Math.max(0, value))
    engineRef.current?.setVolume(clamped)
    setVolumeState(clamped)
  }, [])

  const setRate = useCallback((value: number) => {
    engineRef.current?.setRate(value)
    setRateState(value)
  }, [])

  const selectAudio = useCallback((id: string) => {
    engineRef.current?.selectAudio(id)
    setAudioTrack(id)
  }, [])

  const selectSubtitle = useCallback((id: string | null) => {
    engineRef.current?.selectSubtitle(id)
    setSubtitleTrack(id)
  }, [])

  return {
    engine,
    on,
    status,
    currentTime,
    duration,
    volume,
    rate,
    audioTracks,
    subtitleTracks,
    audioTrack,
    subtitleTrack,
    notice,
    error,
    load,
    toggle,
    seek,
    setVolume,
    setRate,
    selectAudio,
    selectSubtitle,
  }
}
