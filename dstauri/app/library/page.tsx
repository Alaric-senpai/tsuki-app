"use client"

import { useMemo, useState } from "react"
import Link from "next/link"
import { PlusIcon, RefreshCwIcon } from "lucide-react"

import { Pagination, type PageSizeOption } from "@/components/common/pagination"
import { PageHeader } from "@/components/common/page-header"
import { StatusChip } from "@/components/common/status-chip"
import { LibraryToolbar } from "@/components/features/library/library-toolbar"
import { SeriesEditSheet } from "@/components/features/library/series-edit-sheet"
import { SeriesGrid } from "@/components/features/library/series-grid"
import { Button } from "@/components/ui/button"
import { useArchiveSync, useSeries } from "@/hooks/use-library"
import type { SeriesSummary } from "@/lib/ipc/library"

const FILTERS = ["All", "Watching", "Completed", "Failed"] as const
type Filter = (typeof FILTERS)[number]

const PAGE_SIZES: readonly PageSizeOption[] = [
  {value: "5", label: "5 per page"},
  {value: "10", label: "10 per page"},
  { value: "15", label: "15 per page" },
  { value: "30", label: "30 per page" },
  { value: "50", label: "50 per page" },
  { value: "all", label: "All" },
]

function matchesFilter(series: SeriesSummary, filter: Filter): boolean {
  if (filter === "All") return true
  if (filter === "Failed") return series.failedEpisodes > 0
  if (filter === "Completed") {
    return series.totalEpisodes > 0 && series.downloadedEpisodes >= series.totalEpisodes
  }
  return series.downloadedEpisodes < series.totalEpisodes && series.failedEpisodes === 0
}

export default function LibraryPage() {
  const [filter, setFilter] = useState<Filter>("All")
  const [query, setQuery] = useState("")
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState("10")
  const [editing, setEditing] = useState<SeriesSummary | null>(null)
  const [editOpen, setEditOpen] = useState(false)
  const library = useSeries()
  const archive = useArchiveSync()

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return library.data.filter(
      (series) =>
        matchesFilter(series, filter) &&
        (!needle ||
          series.name.toLowerCase().includes(needle) ||
          series.key.includes(needle)),
    )
  }, [filter, library.data, query])

  const limit = pageSize === "all" ? Math.max(visible.length, 1) : Number(pageSize)
  const pageCount = Math.max(1, Math.ceil(visible.length / limit))
  const currentPage = Math.min(page, pageCount)
  const pageRows = useMemo(
    () => visible.slice((currentPage - 1) * limit, currentPage * limit),
    [currentPage, limit, visible],
  )

  return (
    <>
      <PageHeader
        eyebrow="Your collection"
        title="Library"
        description="Every migrated series, with its episode registry, download state and save folder."
        actions={
          <>
            <Button
              variant="outline"
              onClick={() => void archive.sync()}
              disabled={archive.syncing}
            >
              <RefreshCwIcon className={archive.syncing ? "animate-spin" : undefined} />
              {archive.syncing ? "Syncing" : "Sync archives"}
            </Button>
            <Button nativeButton={false} render={<Link href="/explore" />}>
              <PlusIcon />
              Add a series
            </Button>
          </>
        }
      />

      <div className="space-y-5 px-6 py-6">
        <LibraryToolbar
          filters={FILTERS}
          filter={filter}
          query={query}
          onFilterChange={(next) => {
            setFilter(next as Filter)
            setPage(1)
          }}
          onQueryChange={(next) => {
            setQuery(next)
            setPage(1)
          }}
        />

        {archive.error ? (
          <p className="text-sm text-destructive" role="alert">
            Archive sync failed: {archive.error.message}
          </p>
        ) : null}
        {archive.changed !== null && !archive.error ? (
          <StatusChip
            tone="active"
            label={`Archive sync updated ${archive.changed} episode(s)`}
          />
        ) : null}

        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="telemetry text-[0.55rem] text-muted-foreground">
            {visible.length} of {library.data.length} series
          </p>
          <Pagination
            page={currentPage}
            pageCount={pageCount}
            pageSize={pageSize}
            sizes={PAGE_SIZES}
            onPageChange={setPage}
            onPageSizeChange={(size) => {
              setPageSize(size)
              setPage(1)
            }}
          />
        </div>

        <SeriesGrid
          series={pageRows}
          loading={library.loading}
          error={library.error}
          onRetry={library.refresh}
          onEdit={(series) => {
            setEditing(series)
            setEditOpen(true)
          }}
          emptyTitle="No series match this view"
          emptyDescription="Clear the search or choose a different status filter."
        />
      </div>

      <SeriesEditSheet
        key={`${editing?.id ?? "none"}-${editOpen}`}
        series={editing}
        open={editOpen}
        onOpenChange={setEditOpen}
        onSaved={library.refresh}
      />
    </>
  )
}
