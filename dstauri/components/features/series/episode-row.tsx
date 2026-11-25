import Link from "next/link"
import { ArrowUpRightIcon, PlayIcon } from "lucide-react"

import { StatusChip, type StatusTone } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"

export interface EpisodeRowData {
  id: number
  seriesId?: number
  seriesName?: string
  videoId: string
  episodeNumber: number | null
  title: string
  status: string
  downloadedAt: string | null
  createdAt: string
}

const STATUS_TONES: Record<string, StatusTone> = {
  downloaded: "active",
  queued: "buffering",
  pending: "idle",
  not_started: "idle",
  failed: "error",
}

export function statusTone(status: string): StatusTone {
  return STATUS_TONES[status.toLowerCase()] ?? "stalled"
}

export function episodeLabel(episode: EpisodeRowData): string {
  const title = episode.title.trim() || "Untitled episode"
  if (episode.episodeNumber === null) {
    return title
  }
  return `EP ${String(episode.episodeNumber).padStart(2, "0")} · ${title}`
}

function formatDate(value: string | null): string {
  if (!value) {
    return "Not saved"
  }
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) {
    return value
  }
  return parsed.toLocaleDateString()
}

export function EpisodeRow({ episode }: { episode: EpisodeRowData }) {
  return (
    <article className="flex flex-wrap items-center gap-3 rounded-lg border border-border/60 bg-surface-low px-4 py-3">
      <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-surface-high ring-1 ring-border/60">
        <PlayIcon className="size-3.5 text-primary" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium text-foreground">
          {episodeLabel(episode)}
        </p>
        <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
          {episode.seriesName ? `${episode.seriesName} · ` : ""}
          {episode.videoId} · {formatDate(episode.downloadedAt ?? episode.createdAt)}
        </p>
      </div>
      <StatusChip
        tone={statusTone(episode.status)}
        label={episode.status.toUpperCase()}
      />
      <Button
        size="xs"
        variant="ghost"
        nativeButton={false}
        render={<Link href={`/watch?episodeId=${episode.id}`} />}
        aria-label={`Open ${episodeLabel(episode)} in player`}
      >
        <ArrowUpRightIcon />
      </Button>
    </article>
  )
}
