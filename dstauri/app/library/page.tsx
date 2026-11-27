"use client"

import { Suspense, useEffect, useMemo, useState } from "react"
import Link from "next/link"
import { useSearchParams } from "next/navigation"
import { PlusIcon, RefreshCwIcon } from "lucide-react"

import { LoadingGrid } from "@/components/common/data-state"
import { Pagination } from "@/components/common/pagination"
import { PageHeader } from "@/components/common/page-header"
import { StatusChip } from "@/components/common/status-chip"
import {
  ALL_SOURCES,
  filterAndSortLibrary,
  LIBRARY_FILTERS,
  LIBRARY_PAGE_SIZES,
  paginateLibrary,
} from "@/components/features/library/library-view"
import { LibraryToolbar } from "@/components/features/library/library-toolbar"
import { SeriesEditSheet } from "@/components/features/library/series-edit-sheet"
import { SeriesGrid } from "@/components/features/library/series-grid"
import { Button } from "@/components/ui/button"
import { useArchiveSync, useSeries } from "@/hooks/use-library"
import { useLibraryView } from "@/hooks/use-library-view"
import type { SeriesSummary } from "@/lib/ipc/library"

function LibraryView() {
  const urlQuery = useSearchParams().get("q")
  const {
    view,
    setFilter,
    setSort,
    setQuery,
    setPage,
    setPageSize,
    setSource,
  } = useLibraryView()
  const [editing, setEditing] = useState<SeriesSummary | null>(null)
  const [editOpen, setEditOpen] = useState(false)
  const library = useSeries()
  const archive = useArchiveSync()

  // Top-bar searches navigate here with ?q=…; the URL wins over the persisted
  // query so every submission filters the library immediately.
  useEffect(() => {
    if (!urlQuery) return
    setQuery(urlQuery)
    setPage(1)
  }, [urlQuery, setQuery, setPage])

  const sources = useMemo(
    () => [
      ALL_SOURCES,
      ...Array.from(
        new Set(
          library.data
            .map((series) => series.sourceKey.trim())
            .filter((key) => key.length > 0),
        ),
      ).sort((a, b) => a.localeCompare(b)),
    ],
    [library.data],
  )

  // A persisted source can disappear after a re-sync; fall back to "all"
  // instead of leaving an empty, confusing Select value behind.
  const activeSource = sources.includes(view.source) ? view.source : ALL_SOURCES

  const visible = useMemo(
    () =>
      filterAndSortLibrary(library.data, {
        filter: view.filter,
        sort: view.sort,
        query: view.query,
        source: activeSource,
      }),
    [activeSource, library.data, view.filter, view.sort, view.query],
  )

  const { rows: pageRows, page: currentPage, pageCount } = useMemo(
    () => paginateLibrary(visible, view.page, view.pageSize),
    [visible, view.page, view.pageSize],
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
          filters={LIBRARY_FILTERS}
          filter={view.filter}
          query={view.query}
          onFilterChange={setFilter}
          onQueryChange={setQuery}
          sort={view.sort}
          onSortChange={setSort}
          sources={sources}
          source={activeSource}
          onSourceChange={setSource}
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
            pageSize={view.pageSize}
            sizes={LIBRARY_PAGE_SIZES}
            onPageChange={setPage}
            onPageSizeChange={setPageSize}
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

export default function LibraryPage() {
  return (
    <Suspense fallback={<LoadingGrid count={3} />}>
      <LibraryView />
    </Suspense>
  )
}
