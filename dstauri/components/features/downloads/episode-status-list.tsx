"use client"

import type { ReactNode } from "react"
import { DownloadIcon, RefreshCwIcon } from "lucide-react"

import { StatusChip, type StatusTone } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import type { EpisodeWithSeries } from "@/lib/ipc/library"
import { cn } from "@/lib/utils"

function skeletonRows(count: number) {
  return Array.from({ length: count }, (_, index) => (
    <div
      key={index}
      className="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-low px-4 py-3"
      aria-hidden
    >
      <div className="size-8 shrink-0 animate-pulse rounded-lg bg-muted" />
      <div className="flex-1 space-y-2">
        <div className="h-3 w-2/3 animate-pulse rounded bg-muted" />
        <div className="h-2 w-1/3 animate-pulse rounded bg-muted" />
      </div>
      <div className="h-[22px] w-20 animate-pulse rounded-full bg-muted" />
    </div>
  ))
}

export function episodeHeading(episode: EpisodeWithSeries): string {
  return episode.episodeNumber === null
    ? episode.title
    : `EP ${String(episode.episodeNumber).padStart(2, "0")} · ${episode.title}`
}

/**
 * Catalog episodes in one status, rendered as the compact queue rows used by
 * the Downloads page. Purely presentational: the parent owns the data.
 */
export function EpisodeStatusList({
  episodes,
  loading,
  tone,
  label,
  emptyLabel,
  onRetry,
  actionLabel = "Retry",
  className,
}: {
  episodes: EpisodeWithSeries[]
  loading: boolean
  tone: StatusTone
  label: string
  emptyLabel: string
  onRetry?: (episodeId: number) => void
  actionLabel?: string
  className?: string
}): ReactNode {
  if (loading) {
    return <div className={cn("space-y-2", className)}>{skeletonRows(3)}</div>
  }

  if (episodes.length === 0) {
    return (
      <div
        className={cn(
          "flex items-center justify-between rounded-lg border border-border/60 bg-surface-low px-4 py-3",
          className,
        )}
      >
        <span className="text-sm text-muted-foreground">{emptyLabel}</span>
      </div>
    )
  }

  return (
    <div className={cn("space-y-2", className)}>
      {episodes.map((episode) => (
        <div
          key={episode.id}
          className="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-low px-4 py-3"
        >
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-surface-high ring-1 ring-border/60">
            <DownloadIcon className="size-3.5 text-primary" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium text-foreground">
              {episodeHeading(episode)}
            </p>
            <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
              {episode.seriesName} · {episode.videoId}
            </p>
          </div>
          {onRetry ? (
            <Button
              variant="ghost"
              size="xs"
              onClick={() => onRetry(episode.id)}
              aria-label={`${actionLabel} ${episode.title}`}
            >
              <RefreshCwIcon />
              {actionLabel}
            </Button>
          ) : null}
          <StatusChip tone={tone} label={label} />
        </div>
      ))}
    </div>
  )
}
