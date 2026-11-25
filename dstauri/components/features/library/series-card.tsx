import Link from "next/link"

import { PosterCard } from "@/components/common/poster-card"
import { Badge } from "@/components/ui/badge"
import type { SeriesSummary } from "@/lib/ipc/library"

export function SeriesCard({
  series,
}: {
  series: SeriesSummary
}) {
  return (
    <article className="group/card relative">
      <Link
        href={`/series?id=${series.id}`}
        className="block rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={`Open ${series.name}`}
      >
        <PosterCard
          title={series.name}
          meta={`${series.totalEpisodes} episodes · ${series.saveDir}`}
          imageUrl={series.bannerUrl}
          quickAction={null}
        />
        <div className="mt-2 flex flex-wrap items-center gap-1.5 px-0.5">
          <Badge variant="secondary">
            {series.downloadedEpisodes}/{series.totalEpisodes} saved
          </Badge>
          {series.failedEpisodes > 0 ? (
            <Badge variant="destructive">{series.failedEpisodes} failed</Badge>
          ) : null}
          {series.queuedEpisodes > 0 ? (
            <Badge variant="outline">{series.queuedEpisodes} queued</Badge>
          ) : null}
        </div>
      </Link>
    </article>
  )
}
