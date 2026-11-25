"use client"

import Link from "next/link"
import { useMemo } from "react"
import {
  ArrowUpRightIcon,
  MoreHorizontalIcon,
  PlayIcon,
  TvIcon,
} from "lucide-react"
import {
  createColumnHelper,
  tableFeatures,
  useTable,
} from "@tanstack/react-table"

import { StatusChip } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { Episode } from "@/lib/ipc/library"

import {
  episodeLabel,
  statusTone,
  type EpisodeRowData,
} from "./episode-row"

const features = tableFeatures({})
const helper = createColumnHelper<typeof features, EpisodeRowData>()
const NO_EPISODES: EpisodeRowData[] = []

function formatDate(value: string | null): string {
  if (!value) {
    return "Not saved"
  }
  const parsed = new Date(value)
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleDateString()
}

function EpisodeActions({ episode }: { episode: EpisodeRowData }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Actions for ${episodeLabel(episode)}`}
          />
        }
      >
        <MoreHorizontalIcon />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem>
          <Link
            href={`/watch?episodeId=${episode.id}`}
            className="flex w-full items-center gap-2"
          >
            <PlayIcon className="size-3.5 text-primary" />
            Open in player
          </Link>
        </DropdownMenuItem>
        <DropdownMenuItem>
          <Link href={`/series?id=${episode.seriesId ?? ""}`} className="flex w-full items-center gap-2">
            <TvIcon className="size-3.5 text-primary" />
            View series
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export function EpisodeTable({
  episodes,
  showSeries,
}: {
  episodes: EpisodeRowData[] | Episode[]
  showSeries?: boolean
}) {
  const rows = useMemo<EpisodeRowData[]>(() => episodes, [episodes])
  const hasSeries =
    showSeries ?? rows.some((row) => Boolean((row as EpisodeRowData).seriesName))
  const columns = useMemo(
    () =>
      helper.columns([
        helper.accessor((row) => row.title, {
          id: "episode",
          header: "Episode",
          cell: (info) => (
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-foreground">
                {episodeLabel(info.row.original)}
              </p>
              <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
                {info.row.original.videoId}
              </p>
            </div>
          ),
        }),
        ...(hasSeries
          ? [
              helper.accessor((row) => (row as EpisodeRowData).seriesName ?? "—", {
                id: "series",
                header: "Series",
                cell: (info) => (
                  <span className="text-xs text-muted-foreground">{info.getValue()}</span>
                ),
              }),
            ]
          : []),
        helper.accessor((row) => String(row.id), {
          id: "id",
          header: "ID",
          cell: (info) => (
            <span className="font-mono text-xs text-muted-foreground">
              #{info.getValue()}
            </span>
          ),
        }),
        helper.accessor((row) => row.status, {
          id: "status",
          header: "Status",
          cell: (info) => (
            <StatusChip tone={statusTone(info.getValue())} label={info.getValue().toUpperCase()} />
          ),
        }),
        helper.accessor((row) => row.downloadedAt ?? row.createdAt, {
          id: "saved",
          header: "Saved date",
          cell: (info) => (
            <span className="text-xs text-muted-foreground">
              {formatDate(info.row.original.downloadedAt ?? info.getValue())}
            </span>
          ),
        }),
        helper.display({
          id: "actions",
          header: "Actions",
          cell: (info) => <EpisodeActions episode={info.row.original} />,
        }),
      ]),
    [hasSeries],
  )

  const table = useTable({
    features,
    columns,
    data: rows.length ? rows : NO_EPISODES,
  })

  return (
    <div className="overflow-hidden rounded-xl border border-border/60 bg-surface-low">
      <Table className="min-w-[42rem]">
        <TableHeader>
          {table.getHeaderGroups().map((group) => (
            <TableRow key={group.id}>
              {group.headers.map((header) => (
                <TableHead key={header.id}>
                  {header.isPlaceholder ? null : (
                    <table.FlexRender header={header} />
                  )}
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {table.getRowModel().rows.map((row) => (
            <TableRow key={row.id}>
              {row.getAllCells().map((cell) => (
                <TableCell key={cell.id}>
                  <table.FlexRender cell={cell} />
                </TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <div className="flex items-center justify-between border-t border-border/60 px-3 py-2">
        <p className="telemetry text-[0.55rem] text-muted-foreground">
          {rows.length} episode{rows.length === 1 ? "" : "s"}
        </p>
        <Link
          href="/watch"
          className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
        >
          Open player
          <ArrowUpRightIcon className="size-3" />
        </Link>
      </div>
    </div>
  )
}
