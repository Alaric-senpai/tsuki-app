"use client"

import { useMemo } from "react"
import Link from "next/link"
import { PlayIcon, RefreshCwIcon, Trash2Icon } from "lucide-react"
import { type ColumnDef, type TableFeatures } from "@tanstack/table-core"
import { createColumnHelper, tableFeatures } from "@tanstack/react-table"

import { DataTable } from "@/components/common/data-table"
import { StatusChip, type StatusTone } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import { episodeHeading } from "@/components/features/downloads/episode-status-list"
import type { EpisodeWithSeries } from "@/lib/ipc/library"

const helper = createColumnHelper<
  ReturnType<typeof tableFeatures>,
  EpisodeWithSeries
>()

/**
 * Catalog episodes in one status, rendered as a sortable/paginated table with
 * per-row actions. Purely presentational: the parent owns the data and the
 * action handlers, so this is safe to reuse for queued, failed and downloaded
 * episodes alike.
 */
export function EpisodeStatusTable({
  episodes,
  loading,
  tone,
  label,
  emptyTitle,
  emptyDescription,
  actionLabel,
  onRetry,
  onRemove,
  watch,
}: {
  episodes: EpisodeWithSeries[]
  loading: boolean
  tone: StatusTone
  label: string
  emptyTitle: string
  emptyDescription: string
  actionLabel?: string
  onRetry?: (episodeId: number) => void
  onRemove?: (episodeId: number) => void
  /** Show a Watch column linking to local playback for saved episodes. */
  watch?: boolean
}) {
  const columns = useMemo(
    () =>
      helper.columns([
        helper.accessor((row) => row.title, {
          id: "episode",
          header: "Episode",
          cell: (info) => (
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-foreground">
                {episodeHeading(info.row.original)}
              </p>
              <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
                {info.row.original.videoId}
              </p>
            </div>
          ),
        }),
        helper.accessor((row) => row.seriesName, {
          id: "series",
          header: "Series",
          cell: (info) => (
            <span className="text-xs text-muted-foreground">
              {info.getValue()}
            </span>
          ),
        }),
        helper.display({
          id: "status",
          header: "Status",
          cell: () => <StatusChip tone={tone} label={label} />,
        }),
        helper.display({
          id: "play",
          header: "Stream / Watch",
          cell: (info) =>
            watch ? (
              <Button
                variant="outline"
                size="xs"
                nativeButton={false}
                render={
                  <Link
                    href={`/watch?episodeId=${info.row.original.id}&title=${encodeURIComponent(info.row.original.title)}`}
                  />
                }
              >
                <PlayIcon />
                Watch
              </Button>
            ) : (
              <span className="text-xs text-muted-foreground">—</span>
            ),
        }),
        helper.display({
          id: "actions",
          header: "Actions",
          cell: (info) => (
            <div className="flex items-center justify-end gap-1">
              {onRetry ? (
                <Button
                  variant="ghost"
                  size="xs"
                  onClick={() => onRetry(info.row.original.id)}
                  aria-label={`${actionLabel ?? "Retry"} ${info.row.original.title}`}
                >
                  <RefreshCwIcon />
                  {actionLabel ?? "Retry"}
                </Button>
              ) : null}
              {onRemove ? (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => onRemove(info.row.original.id)}
                  aria-label={`Remove ${info.row.original.title} from queue`}
                >
                  <Trash2Icon />
                </Button>
              ) : null}
            </div>
          ),
        }),
      ]) as unknown as ColumnDef<TableFeatures, EpisodeWithSeries, unknown>[],
    [tone, label, watch, onRetry, onRemove, actionLabel],
  )

  const bulkAction = onRetry
    ? { label: "Retry selected", run: onRetry }
    : onRemove
      ? { label: "Remove selected from queue", run: onRemove }
      : null

  return (
    <DataTable
      columns={columns}
      data={episodes}
      getRowId={(episode) => String(episode.id)}
      loading={loading}
      enableSelection={Boolean(bulkAction)}
      selectionToolbar={
        bulkAction
          ? (ids, clear) => (
              <Button
                size="sm"
                onClick={() => {
                  for (const id of ids) {
                    bulkAction.run(Number(id))
                  }
                  clear()
                }}
              >
                {bulkAction.label}
              </Button>
            )
          : undefined
      }
      emptyTitle={emptyTitle}
      emptyDescription={emptyDescription}
    />
  )
}
