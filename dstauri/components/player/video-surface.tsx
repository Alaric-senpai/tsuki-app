"use client"

import { useEffect, useState } from "react"

import { usePlayer } from "@/hooks/use-player"
import type { EnginePreference, PlayableSource } from "@/lib/player/engine"
import { cn } from "@/lib/utils"
import { PlayerControls } from "./player-controls"

export type VideoSurfaceProps = {
  source: PlayableSource
  preference?: EnginePreference
  startAt?: number
  className?: string
  onProgress?: (seconds: number, duration: number) => void
  onEnded?: () => void
  onMiniPlayer?: () => void
}

/**
 * Theater surface shared by the stream route and the watch route. Owns the
 * media element, the engine adapter and the control overlay; pages only pass
 * a source and callbacks.
 */
export function VideoSurface({
  source,
  preference = "auto",
  startAt,
  className,
  onProgress,
  onEnded,
  onMiniPlayer,
}: VideoSurfaceProps) {
  const [video, setVideo] = useState<HTMLVideoElement | null>(null)
  const player = usePlayer(video, preference)
  const { engine, load, on } = player

  useEffect(() => {
    if (!engine) return
    void load(source, startAt ?? 0)
  }, [engine, load, source, startAt])

  useEffect(() => {
    if (!onProgress) return
    return on("timeupdate", ({ position, duration }) => {
      onProgress(position, duration)
    })
  }, [on, onProgress])

  useEffect(() => {
    if (!onEnded) return
    return on("ended", onEnded)
  }, [on, onEnded])

  return (
    <div
      data-player-surface
      className={cn(
        "group relative isolate aspect-video w-full overflow-hidden rounded-xl bg-black",
        className,
      )}
    >
      <video
        ref={setVideo}
        playsInline
        className="size-full object-contain"
        onClick={() => void player.toggle()}
      />
      {player.notice ? (
        <p className="pointer-events-none absolute left-3 top-3 rounded-md bg-surface-container-lowest/85 px-3 py-1 text-telemetry-micro text-on-surface-variant">
          {player.notice}
        </p>
      ) : null}
      <PlayerControls player={player} onMiniPlayer={onMiniPlayer} />
    </div>
  )
}
