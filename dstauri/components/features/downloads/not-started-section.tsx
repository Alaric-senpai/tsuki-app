"use client"

import { useMemo } from "react"
import { DownloadIcon } from "lucide-react"
import { type ColumnDef, type TableFeatures } from "@tanstack/table-core"
import { createColumnHelper, tableFeatures } from "@tanstack/react-table"

import { DataTable } from "@/components/common/data-table"
import { StatusChip } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import { episodeHeading } from "@/components/features/downloads/episode-status-list"
import type { EpisodeWithSeries } from "@/lib/ipc/library"

const helper = createColumnHelper<
  ReturnType<typeof tableFeatures>,
  EpisodeWithSeries
>()

/**
 * Catalog episodes that are in the library but have no download running yet.
 * Selecting rows drives the bulk queue action.
 */
export function NotStartedSection({
  episodes,
  loading,
  busy,
  onDownload,
}: {
  episodes: EpisodeWithSeries[]
  loading: boolean
  busy: boolean
  onDownload: (episodeIds: number[]) => Promise<void>
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
          cell: () => <StatusChip tone="idle" label="NOT STARTED" />,
        }),
      ]) as unknown as ColumnDef<TableFeatures, EpisodeWithSeries, unknown>[],
    [],
  )

  return (
    <DataTable
      columns={columns}
      data={episodes}
      getRowId={(episode) => String(episode.id)}
      loading={loading}
      enableSelection
      selectionToolbar={(ids, clear) => (
        <Button
          size="sm"
          disabled={busy}
          onClick={() =>
            void onDownload(
              episodes
                .filter((episode) => ids.has(String(episode.id)))
                .map((episode) => episode.id),
            ).then(clear)
          }
        >
          <DownloadIcon />
          Download selected
        </Button>
      )}
      emptyTitle="Nothing waiting"
      emptyDescription="Every episode in the library already has a download or a saved file."
    />
  )
}
