"use client"

import { useMemo } from "react"
import Link from "next/link"
import { PlayIcon } from "lucide-react"
import { type ColumnDef, type TableFeatures } from "@tanstack/table-core"
import { createColumnHelper, tableFeatures } from "@tanstack/react-table"

import { DataTable } from "@/components/common/data-table"
import { Button } from "@/components/ui/button"
import type { SourceEpisode } from "@/lib/ipc/explore"

const helper = createColumnHelper<ReturnType<typeof tableFeatures>, SourceEpisode>()

const streamHref = (episode: SourceEpisode, source: string, seriesId: string) => {
  const params = new URLSearchParams({
    url: episode.url,
    title: episode.title,
    source,
    seriesId,
  })
  return `/stream?${params.toString()}`
}

/**
 * Episode list for an online item. The user ticks only the episodes they want
 * and the toolbar adds that subset to a library entry (existing or new).
 * Every row also has a direct Stream action.
 */
export function ItemEpisodesTable({
  episodes,
  source,
  seriesId,
  loading,
  onAddSelected,
}: {
  episodes: SourceEpisode[]
  source: string
  seriesId: string
  loading?: boolean
  onAddSelected: (urls: string[]) => void
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
                {info.row.original.numberText
                  ? `EP ${info.row.original.numberText} · ${info.row.original.title}`
                  : info.row.original.title}
              </p>
              {info.row.original.date ? (
                <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
                  {info.row.original.date}
                </p>
              ) : null}
            </div>
          ),
        }),
        helper.display({
          id: "stream",
          header: "Stream",
          cell: (info) => (
            <Button variant="outline" size="xs" nativeButton={false} render={<Link href={streamHref(info.row.original, source, seriesId)} />}>
              <PlayIcon />
              Stream
            </Button>
          ),
        }),
      ]) as unknown as ColumnDef<TableFeatures, SourceEpisode, unknown>[],
    [source, seriesId],
  )

  return (
    <DataTable
      columns={columns}
      data={episodes}
      getRowId={(episode) => episode.url}
      loading={loading}
      enableSelection
      emptyTitle="No episodes found"
      emptyDescription="This source did not list any episodes for the item."
      selectionToolbar={(selected, clear) => (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="xs"
            onClick={() => {
              onAddSelected([...selected])
              clear()
            }}
          >
            Add selected to library
          </Button>
          <Button size="xs" variant="ghost" onClick={clear}>
            Deselect
          </Button>
        </div>
      )}
    />
  )
}
