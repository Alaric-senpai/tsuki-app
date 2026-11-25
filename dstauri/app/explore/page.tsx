"use client"

import { useCallback, useMemo, useState } from "react"
import { useRouter } from "next/navigation"
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { PageHeader } from "@/components/common/page-header"
import { Button } from "@/components/ui/button"
import { SourceDropdown } from "@/components/features/explore/source-dropdown"
import {
  FilterBar,
  defaultFilters,
  type ExploreFiltersState,
} from "@/components/features/explore/filter-bar"
import { PosterGrid } from "@/components/features/explore/poster-grid"
import {
  useExploreCatalog,
  useExploreFilters,
  useExploreSources,
} from "@/hooks/use-explore"
import type { CatalogQuery } from "@/lib/ipc/explore"

const DEFAULT_SOURCE = "donghuastream"

export default function ExplorePage() {
  const router = useRouter()
  const sources = useExploreSources()
  const [activeSource, setActiveSource] = useState(DEFAULT_SOURCE)
  const [filters, setFilters] = useState<ExploreFiltersState>(defaultFilters)
  const [page, setPage] = useState(1)
  const [catalogRefresh, setCatalogRefresh] = useState(0)

  const handleSourceChange = useCallback((key: string) => {
    setActiveSource(key)
    setFilters(defaultFilters())
    setPage(1)
  }, [])

  const handleFiltersChange = useCallback((next: ExploreFiltersState) => {
    setFilters(next)
    setPage(1)
  }, [])

  const catalogFilters = useExploreFilters(activeSource)

  const query = useMemo<CatalogQuery>(() => {
    const searching = filters.search.trim().length > 0
    return {
      search: searching ? filters.search.trim() : undefined,
      genre: searching ? undefined : filters.genre || undefined,
      order: searching ? undefined : filters.order || undefined,
      status: searching ? undefined : filters.status || undefined,
      kind: searching ? undefined : filters.kind || undefined,
      page,
    }
  }, [
    filters.search,
    filters.genre,
    filters.order,
    filters.status,
    filters.kind,
    page,
  ])

  const catalog = useExploreCatalog(activeSource, query, catalogRefresh)

  const handleSync = useCallback(() => {
    setCatalogRefresh((n) => n + 1)
  }, [])

  const handleSearch = useCallback(
    (search: string) => {
      setFilters((prev) => ({ ...prev, search }))
      setPage(1)
      setCatalogRefresh((n) => n + 1)
    },
    [],
  )

  const openItem = useCallback(
    (source: string, id: string) => {
      router.push(
        `/item?source=${encodeURIComponent(source)}&id=${encodeURIComponent(id)}`,
      )
    },
    [router],
  )

  const result = catalog.data
  const canGoBack = page > 1
  const canGoNext = result.hasMore && result.cards.length > 0

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        eyebrow="Discover"
        title="Explore"
        description="Remote catalog: search, genres, and trending."
        actions={
          <SourceDropdown
            sources={sources.data}
            active={activeSource}
            onSelect={handleSourceChange}
          />
        }
      />

      <main className="flex-1 overflow-y-auto px-6 py-6 space-y-6">
        <FilterBar
          filters={filters}
          catalogFilters={catalogFilters.data}
          onFiltersChange={handleFiltersChange}
          onSearch={handleSearch}
          onRefresh={handleSync}
          refreshing={catalog.loading}
          searching={false}
        />

        <PosterGrid
          cards={result.cards}
          loading={catalog.loading}
          error={catalog.error}
          onSelect={(card) => openItem(card.source, card.id)}
        />

        {(canGoBack || canGoNext) && !catalog.loading && !catalog.error ? (
          <nav
            aria-label="Catalog pages"
            className="flex items-center justify-center gap-4 pt-2"
          >
            <Button
              variant="outline"
              size="sm"
              disabled={!canGoBack || catalog.loading}
              onClick={() => setPage((current) => Math.max(1, current - 1))}
            >
              <ChevronLeftIcon />
              Previous
            </Button>
            <span className="telemetry text-xs text-muted-foreground">
              Page {result.page || page}
              {result.total > 0 ? ` · ${result.total} this page` : ""}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={!canGoNext || catalog.loading}
              onClick={() => setPage((current) => current + 1)}
            >
              Next
              <ChevronRightIcon />
            </Button>
          </nav>
        ) : null}
      </main>
    </div>
  )
}
