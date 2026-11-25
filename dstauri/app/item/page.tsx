"use client"

import { Suspense, useCallback, useState } from "react"
import { useRouter, useSearchParams } from "next/navigation"
import { ArrowLeftIcon, PlusIcon, RefreshCwIcon, SearchXIcon } from "lucide-react"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { PageHeader } from "@/components/common/page-header"
import { Button } from "@/components/ui/button"
import { AddToLibraryDialog } from "@/components/features/explore/add-to-library-dialog"
import { ItemEpisodesTable } from "@/components/features/explore/item-episodes-table"
import { ItemHero } from "@/components/features/explore/item-hero"
import { PosterGrid } from "@/components/features/explore/poster-grid"
import { useExploreSeries } from "@/hooks/use-explore"

function OnlineItemDetails() {
  const router = useRouter()
  const params = useSearchParams()
  const source = params.get("source") ?? "donghuastream"
  const id = params.get("id") ?? ""
  const [refreshToken, setRefreshToken] = useState(0)
  const [addUrls, setAddUrls] = useState<string[] | null>(null)
  const series = useExploreSeries(source, id, refreshToken)

  const openAdd = useCallback((urls: string[]) => setAddUrls(urls), [])
  const addAll = useCallback(() => {
    openAdd((series.data?.episodes ?? []).map((episode) => episode.url))
  }, [openAdd, series.data])

  if (series.loading) {
    return (
      <div className="space-y-5">
        <PageHeader eyebrow="Explore" title="Item details" />
        <LoadingGrid count={1} />
      </div>
    )
  }
  if (series.error) {
    return (
      <div className="space-y-5">
        <PageHeader eyebrow="Explore" title="Item details" />
        <ErrorPanel
          error={series.error}
          onRetry={() => setRefreshToken((token) => token + 1)}
        />
      </div>
    )
  }
  const data = series.data
  if (!data) {
    return (
      <EmptyState
        icon={SearchXIcon}
        title="Item not found"
        description="This source returned no details."
      />
    )
  }

  const allUrls = data.episodes.map((episode) => episode.url)

  return (
    <div className="space-y-8">
      <PageHeader
        eyebrow={source.replace(/_/g, " ")}
        title={data.title}
        description="Review the details, stream an episode, or add a selection to your library."
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="ghost" size="sm" onClick={() => router.back()}>
              <ArrowLeftIcon />
              Back
            </Button>
            <Button variant="outline" size="sm" onClick={() => setRefreshToken((token) => token + 1)}>
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

      <ItemHero series={data} />

      <section className="space-y-3">
        <div className="flex items-baseline justify-between gap-3">
          <h2 className="font-heading text-lg font-semibold text-foreground">Episodes</h2>
          <span className="telemetry text-[0.6rem] text-muted-foreground">
            {data.episodes.length} available
          </span>
        </div>
        <ItemEpisodesTable
          episodes={data.episodes}
          source={source}
          seriesId={id}
          onAddSelected={openAdd}
        />
      </section>

      {data.recommendations.length > 0 ? (
        <section className="space-y-3">
          <h2 className="font-heading text-lg font-semibold text-foreground">Recommendations</h2>
          <PosterGrid
            cards={data.recommendations}
            loading={false}
            error={null}
            onSelect={(card) =>
              router.push(`/item?source=${encodeURIComponent(card.source)}&id=${encodeURIComponent(card.id)}`)
            }
          />
        </section>
      ) : null}

      <AddToLibraryDialog
        open={addUrls !== null}
        onOpenChange={(open) => {
          if (!open) setAddUrls(null)
        }}
        source={source}
        seriesId={id}
        episodeUrls={addUrls ?? []}
        defaultName={data.title}
        onDone={() => setAddUrls(null)}
      />
    </div>
  )
}

export default function ItemPage() {
  return (
    <Suspense fallback={<LoadingGrid count={1} />}>
      <OnlineItemDetails />
    </Suspense>
  )
}
