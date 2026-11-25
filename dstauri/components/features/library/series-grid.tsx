"use client"

import { LibraryIcon, PencilIcon } from "lucide-react"

import { EmptyState } from "@/components/common/empty-state"
import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { Button } from "@/components/ui/button"
import type { AppIpcError } from "@/lib/ipc/client"
import type { SeriesSummary } from "@/lib/ipc/library"

import { SeriesCard } from "./series-card"

export function SeriesGrid({
  series,
  loading,
  error,
  onRetry,
  onEdit,
  emptyTitle = "No series match this view",
  emptyDescription = "Add a series from Explore or change the current filter.",
}: {
  series: SeriesSummary[]
  loading: boolean
  error: AppIpcError | null
  onRetry: () => void
  onEdit?: (series: SeriesSummary) => void
  emptyTitle?: string
  emptyDescription?: string
}) {
  if (loading) {
    return <LoadingGrid />
  }

  if (error) {
    return <ErrorPanel error={error} onRetry={onRetry} />
  }

  if (series.length === 0) {
    return (
      <EmptyState
        icon={LibraryIcon}
        title={emptyTitle}
        description={emptyDescription}
      />
    )
  }

  return (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {series.map((item) => (
        <div key={item.id} className="group/card relative">
          <SeriesCard series={item} />
          {onEdit ? (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={`Edit ${item.name}`}
              className="absolute right-2 top-2 z-10 bg-background/70 opacity-0 backdrop-blur transition-opacity focus-visible:opacity-100 group-hover/card:opacity-100"
              onClick={() => onEdit(item)}
            >
              <PencilIcon />
            </Button>
          ) : null}
        </div>
      ))}
    </div>
  )
}
