"use client"

import { ListIcon } from "lucide-react"

import { EmptyState } from "@/components/common/empty-state"
import { ErrorPanel } from "@/components/common/data-state"
import { Skeleton } from "@/components/ui/skeleton"
import type { AppIpcError } from "@/lib/ipc/client"
import type { Episode } from "@/lib/ipc/library"

import { EpisodeTable } from "./episode-table"
import type { EpisodeRowData } from "./episode-row"

function EpisodeSkeleton() {
  return (
    <div className="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-low px-4 py-3">
      <Skeleton className="size-8 rounded-lg" />
      <div className="flex-1 space-y-2">
        <Skeleton className="h-3 w-2/3" />
        <Skeleton className="h-2 w-1/3" />
      </div>
      <Skeleton className="h-[22px] w-20 rounded-full" />
    </div>
  )
}

export function EpisodeList({
  episodes,
  loading,
  error,
  onRetry,
  emptyTitle = "No episodes here",
  emptyDescription = "Try a different status filter or add episodes from the source catalog.",
}: {
  episodes: EpisodeRowData[] | Episode[]
  loading: boolean
  error: AppIpcError | null
  onRetry: () => void
  emptyTitle?: string
  emptyDescription?: string
}) {
  if (loading) {
    return (
      <div className="space-y-2" aria-busy="true" aria-label="Loading episodes">
        {Array.from({ length: 6 }, (_, index) => (
          <EpisodeSkeleton key={index} />
        ))}
      </div>
    )
  }

  if (error) {
    return <ErrorPanel error={error} onRetry={onRetry} />
  }

  if (episodes.length === 0) {
    return (
      <EmptyState
        icon={ListIcon}
        title={emptyTitle}
        description={emptyDescription}
      />
    )
  }

  return <EpisodeTable episodes={episodes} />
}
