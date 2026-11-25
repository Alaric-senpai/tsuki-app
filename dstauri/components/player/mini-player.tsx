"use client"

import { useCallback, useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { Maximize2Icon, XIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { usePlayer } from "@/hooks/use-player"
import { useMiniPlayer } from "@/stores/mini-player"

/**
 * Picture-in-picture-style player that survives route changes. It owns a real
 * engine instance, so play/pause and seeking work exactly as on the full page.
 */
export function MiniPlayer() {
  const { source, title, close } = useMiniPlayer()
  const router = useRouter()
  const [video, setVideo] = useState<HTMLVideoElement | null>(null)
  const player = usePlayer(video, "auto")
  const { engine, load } = player

  useEffect(() => {
    if (source && engine) void load(source)
  }, [source, engine, load])

  const expand = useCallback(() => {
    if (!source) return
    const query =
      source.kind === "file"
        ? `/watch?path=${encodeURIComponent(source.path)}`
        : `/stream?url=${encodeURIComponent(source.url)}`
    close()
    router.push(query)
  }, [source, close, router])

  if (!source) return null

  return (
    <div
      data-player-surface
      className="fixed bottom-20 right-5 z-50 w-80 overflow-hidden rounded-xl border border-outline-variant/40 bg-surface-container-lowest shadow-xl"
      role="region"
      aria-label="Mini player"
    >
      <div className="flex items-center justify-between gap-2 px-3 py-1.5">
        <span className="truncate text-telemetry-micro text-on-surface-variant">{title}</span>
        <div className="flex shrink-0 items-center gap-1">
          <Button variant="ghost" size="icon-sm" onClick={expand} aria-label="Expand player">
            <Maximize2Icon />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={close} aria-label="Close mini player">
            <XIcon />
          </Button>
        </div>
      </div>
      <video ref={setVideo} playsInline className="aspect-video w-full bg-black" />
      <div className="flex justify-center border-t border-outline-variant/30 py-1">
        <Button variant="ghost" size="sm" onClick={() => void player.toggle()}>
          {player.status === "playing" ? "Pause" : "Play"}
        </Button>
      </div>
    </div>
  )
}
