"use client"

import { useCallback, useState } from "react"
import { useRouter } from "next/navigation"
import { ArrowLeftIcon, PlusIcon, RefreshCwIcon, SearchXIcon } from "lucide-react"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { PageHeader } from "@/components/common/page-header"
import { Button } from "@/components/ui/button"
import type { AppIpcError } from "@/lib/ipc/client"
import type { SourceSeries } from "@/lib/ipc/explore"

import { AddToLibraryDialog } from "./add-to-library-dialog"
import { itemDetailHref, type DetailRoute } from "./item-detail-navigation"
import { ItemEpisodesTable, type EpisodeHrefBuilder } from "./item-episodes-table"
import { ItemHero, type ItemHeroLabels } from "./item-hero"
import { PosterGrid } from "./poster-grid"

/**
 * All presentation shared by every remote detail screen. Source-specific
 * components only resolve the canonical id and pass their own labels/URL
 * builders; loading, empty, error, episodes and recommendations stay here.
 */
export function ItemDetailView({
  source,
  seriesId,
  eyebrow,
  contextEyebrow,
  loadingTitle,
  emptyTitle,
  route,
  series,
  loading,
  error,
  onRetry,
  onRefresh,
  heroLabels,
  buildEpisodeHref,
}: {
  source: string
  seriesId: string
  eyebrow: string
  contextEyebrow: string
  loadingTitle: string
  emptyTitle: string
  route: DetailRoute
  series: SourceSeries | null
  loading: boolean
  error: AppIpcError | null
  onRetry: () => void
  onRefresh: () => void
  heroLabels?: ItemHeroLabels
  buildEpisodeHref?: EpisodeHrefBuilder
}) {
  const router = useRouter()
  const [addUrls, setAddUrls] = useState<string[] | null>(null)

  const openAdd = useCallback((urls: string[]) => setAddUrls(urls), [])
  const addAll = useCallback(() => {
    openAdd((series?.episodes ?? []).map((episode) => episode.url))
  }, [openAdd, series])

  if (loading) {
    return (
      <div className="space-y-5">
        <PageHeader eyebrow={contextEyebrow} title={loadingTitle} />
        <LoadingGrid count={1} />
      </div>
    )
  }
  if (error) {
    return (
      <div className="space-y-5">
        <PageHeader eyebrow={contextEyebrow} title={loadingTitle} />
        <ErrorPanel error={error} onRetry={onRetry} />
      </div>
    )
  }
  if (!series) {
    return (
      <EmptyState
        icon={SearchXIcon}
        title={emptyTitle}
        description="This source returned no details."
      />
    )
  }

  const allUrls = series.episodes.map((episode) => episode.url)

  return (
    <div className="space-y-8">
      <PageHeader
        eyebrow={eyebrow}
        title={series.title}
        description="Review the details, stream an episode, or add a selection to your library."
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="ghost" size="sm" onClick={() => router.back()}>
              <ArrowLeftIcon />
              Back
            </Button>
            <Button variant="outline" size="sm" onClick={onRefresh}>
              <RefreshCwIcon />
              Refresh
            </Button>
            <Button size="sm" disabled={allUrls.length === 0} onClick={addAll}>
              <PlusIcon />
              Add all to library
            </Button>
          </div>
        }
      />

      <ItemHero series={series} labels={heroLabels} />

      <section className="space-y-3">
        <div className="flex items-baseline justify-between gap-3">
          <h2 className="font-heading text-lg font-semibold text-foreground">Episodes</h2>
          <span className="telemetry text-[0.6rem] text-muted-foreground">
            {series.episodes.length} available
          </span>
        </div>
        <ItemEpisodesTable
          episodes={series.episodes}
          source={source}
          seriesId={seriesId}
          onAddSelected={openAdd}
          buildStreamHref={buildEpisodeHref}
        />
      </section>

      {series.recommendations.length > 0 ? (
        <section className="space-y-3">
          <h2 className="font-heading text-lg font-semibold text-foreground">Recommendations</h2>
          <PosterGrid
            cards={series.recommendations}
            loading={false}
            error={null}
            onSelect={(card) => router.push(itemDetailHref(card, route))}
          />
        </section>
      ) : null}

      <AddToLibraryDialog
        open={addUrls !== null}
        onOpenChange={(open) => {
          if (!open) setAddUrls(null)
        }}
        source={source}
        seriesId={seriesId}
        episodeUrls={addUrls ?? []}
        defaultName={series.title}
        onDone={() => setAddUrls(null)}
      />
    </div>
  )
}
