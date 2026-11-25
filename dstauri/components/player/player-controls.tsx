"use client"

import { useEffect, useState } from "react"
import {
  MaximizeIcon,
  Minimize2Icon,
  PauseIcon,
  PlayIcon,
  Volume2Icon,
  VolumeXIcon,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { formatTime, type Track } from "@/lib/player/engine"
import type { usePlayer } from "@/hooks/use-player"
import { cn } from "@/lib/utils"

const RATES = [0.5, 0.75, 1, 1.25, 1.5, 2] as const

type Player = ReturnType<typeof usePlayer>

function TrackSelect({
  label,
  tracks,
  value,
  onChange,
}: {
  label: string
  tracks: Track[]
  value: string | null
  onChange: (id: string | null) => void
}) {
  if (tracks.length === 0) return null
  return (
    <Select value={value ?? "off"} onValueChange={(next) => onChange(next)}>
      <SelectTrigger size="sm" aria-label={label} className="w-auto gap-1">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {tracks.map((track) => (
          <SelectItem key={track.id} value={track.id}>
            {track.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

export function PlayerControls({
  player,
  onMiniPlayer,
}: {
  player: Player
  onMiniPlayer?: () => void
}) {
  const [muted, setMuted] = useState(false)
  const duration = Number.isFinite(player.duration) ? player.duration : 0
  const percent = duration > 0 ? (player.currentTime / duration) * 100 : 0

  const toggleMute = () => {
    const next = !muted
    setMuted(next)
    player.setVolume(next ? 0 : 1)
  }

  const fullscreen = () => {
    const surface = document.querySelector("[data-player-surface]")
    if (surface instanceof HTMLElement) {
      void surface.requestFullscreen?.()
    }
  }

  // Keyboard shortcuts mirror the theater-mode controls: space/k play-pause,
  // arrows seek ±10s, j/l seek ±60s, f fullscreen, m mute. They are ignored
  // while the user is typing in a form field.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT")
      ) {
        return
      }
      const seekBy = (seconds: number) => {
        void player.seek(Math.max(0, player.currentTime + seconds))
      }
      switch (event.key.toLowerCase()) {
        case " ":
        case "k":
          event.preventDefault()
          void player.toggle()
          break
        case "arrowleft":
          event.preventDefault()
          seekBy(-10)
          break
        case "arrowright":
          event.preventDefault()
          seekBy(10)
          break
        case "j":
          seekBy(-60)
          break
        case "l":
          seekBy(60)
          break
        case "f":
          fullscreen()
          break
        case "m":
          player.setVolume(muted ? 1 : 0)
          setMuted(!muted)
          break
        default:
          break
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  })

  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 border-t border-outline-variant/30",
        "bg-surface-container-lowest/85 px-3 py-2 backdrop-blur-md",
      )}
      role="group"
      aria-label="Playback controls"
    >
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={() => void player.toggle()}
        aria-label={player.status === "playing" ? "Pause" : "Play"}
      >
        {player.status === "playing" ? <PauseIcon /> : <PlayIcon />}
      </Button>

      <span className="font-telemetry-micro text-telemetry-micro tabular-nums text-on-surface-variant">
        {formatTime(player.currentTime)}
      </span>

      <input
        type="range"
        min={0}
        max={100}
        step={0.1}
        value={percent}
        onChange={(event) => player.seek((Number(event.target.value) / 100) * duration)}
        aria-label="Seek"
        className="h-1 min-w-24 flex-1 cursor-pointer accent-primary"
      />

      <span className="font-telemetry-micro text-telemetry-micro tabular-nums text-on-surface-variant">
        {formatTime(duration)}
      </span>

      <Button
        variant="ghost"
        size="icon-sm"
        onClick={toggleMute}
        aria-label={muted ? "Unmute" : "Mute"}
      >
        {muted ? <VolumeXIcon /> : <Volume2Icon />}
      </Button>

      <Select
        value={String(player.rate)}
        onValueChange={(value) => player.setRate(Number(value))}
      >
        <SelectTrigger size="sm" className="w-auto" aria-label="Playback speed">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {RATES.map((rate) => (
            <SelectItem key={rate} value={String(rate)}>
              {rate}×
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <TrackSelect
        label="Audio track"
        tracks={player.audioTracks}
        value={player.audioTrack}
        onChange={(id) => { if (id) player.selectAudio(id) }}
      />
      <TrackSelect
        label="Subtitle track"
        tracks={player.subtitleTracks}
        value={player.subtitleTrack}
        onChange={(id) => player.selectSubtitle(id === "off" ? null : id)}
      />

      {onMiniPlayer ? (
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onMiniPlayer}
          aria-label="Mini player"
        >
          <Minimize2Icon />
        </Button>
      ) : null}

      <Button
        variant="ghost"
        size="icon-sm"
        onClick={fullscreen}
        aria-label="Fullscreen"
      >
        <MaximizeIcon />
      </Button>
    </div>
  )
}
