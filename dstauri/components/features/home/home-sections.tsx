"use client"

import Link from "next/link"
import { CompassIcon, LibraryIcon, PlayIcon } from "lucide-react"

import { EmptyState } from "@/components/common/empty-state"
import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { SectionHeader } from "@/components/common/section-header"
import { StatGrid } from "@/components/common/stat-grid"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import type { AsyncResource } from "@/hooks/use-async-resource"
import type {
  EpisodeWithSeries,
  LibraryStats,
  SeriesSummary,
} from "@/lib/ipc/library"

import { SeriesCard } from "../library/series-card"
import { EpisodeRow } from "../series/episode-row"

export function HomeStats({ stats }: { stats: AsyncResource<LibraryStats> }) {
  if (stats.error) {
    return <ErrorPanel error={stats.error} onRetry={stats.refresh} />
  }

  return (
    <StatGrid
      items={[
        { label: "Series", value: stats.data.seriesCount },
        { label: "Episodes", value: stats.data.episodeCount },
        { label: "Downloaded", value: stats.data.downloadedCount },
        { label: "Needs attention", value: stats.data.failedCount },
      ]}
    />
  )
}

export function RecentDownloads({
  recent,
}: {
  recent: AsyncResource<EpisodeWithSeries[]>
}) {
  return (
    <section>
      <SectionHeader
        eyebrow="Pick up where you left off"
        title="Recently downloaded"
        href="/downloads"
        actionLabel="Open downloads"
      />
      {recent.loading ? (
        <div className="space-y-2" aria-busy="true">
          {Array.from({ length: 3 }, (_, index) => (
            <Skeleton key={index} className="h-16 rounded-lg" />
          ))}
        </div>
      ) : recent.error ? (
        <ErrorPanel error={recent.error} onRetry={recent.refresh} />
      ) : recent.data.length === 0 ? (
        <EmptyState
          icon={PlayIcon}
          title="No completed episodes yet"
          description="Downloaded episodes will collect here with their series name and saved date."
        />
      ) : (
        <div className="space-y-2">
          {recent.data.slice(0, 4).map((episode) => (
            <EpisodeRow key={episode.id} episode={episode} />
          ))}
        </div>
      )}
    </section>
  )
}

export function HomeLibrary({
  series,
}: {
  series: AsyncResource<SeriesSummary[]>
}) {
  return (
    <section>
      <SectionHeader
        eyebrow="Your collection"
        title="In your library"
        href="/library"
        actionLabel="Open library"
      />
      {series.loading ? (
        <LoadingGrid count={4} />
      ) : series.error ? (
        <ErrorPanel error={series.error} onRetry={series.refresh} />
      ) : series.data.length === 0 ? (
        <EmptyState
          icon={LibraryIcon}
          title="Your library is empty"
          description="Add a series from Explore and it will appear here with its episode registry."
          action={
            <Button variant="outline" nativeButton={false} render={<Link href="/explore" />}>
              <CompassIcon />
              Browse sources
            </Button>
          }
        />
      ) : (
        <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
          {series.data.slice(0, 4).map((item) => (
            <SeriesCard key={item.id} series={item} />
          ))}
        </div>
      )}
    </section>
  )
}
