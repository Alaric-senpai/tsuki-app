"use client"

import Link from "next/link"
import { ArrowRightIcon, CompassIcon, PlayIcon, SparklesIcon } from "lucide-react"

import { ErrorPanel } from "@/components/common/data-state"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import type { AsyncResource } from "@/hooks/use-async-resource"
import type { EpisodeWithSeries, SeriesSummary } from "@/lib/ipc/library"
import { backgroundImageUrl, cn } from "@/lib/utils"

function timestamp(episode: EpisodeWithSeries): number {
  const value = episode.downloadedAt ?? episode.createdAt
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? 0 : parsed
}

function HeroBackground({ imageUrl }: { imageUrl: string | null }) {
  const image = backgroundImageUrl(imageUrl)
  return (
    <>
      {image ? (
        <div
          aria-hidden
          className="absolute inset-0 bg-cover bg-center opacity-70 transition-transform duration-700 hover:scale-[1.02]"
          style={{ backgroundImage: image }}
        />
      ) : null}
      <div
        aria-hidden
        className="absolute inset-0 bg-gradient-to-r from-surface-lowest via-surface-lowest/85 to-surface-lowest/20"
      />
      <div
        aria-hidden
        className="absolute inset-0 bg-gradient-to-t from-surface-lowest via-transparent to-surface-lowest/60"
      />
      <div
        aria-hidden
        className="pointer-events-none absolute -right-24 -top-40 size-[28rem] rounded-full bg-[radial-gradient(circle,var(--emerald),transparent_68%)] opacity-10 blur-3xl"
      />
    </>
  )
}

function HeroSkeleton() {
  return (
    <section
      className="relative min-h-[22rem] overflow-hidden rounded-2xl border border-border/60 bg-surface-low p-8 lg:p-12"
      aria-busy="true"
      aria-label="Loading spotlight"
    >
      <div className="relative max-w-2xl space-y-5">
        <Skeleton className="h-5 w-40 rounded-full" />
        <Skeleton className="h-12 w-4/5 rounded-lg" />
        <Skeleton className="h-3 w-2/3 rounded" />
        <Skeleton className="h-3 w-1/2 rounded" />
        <div className="flex gap-3 pt-2">
          <Skeleton className="h-10 w-32 rounded-lg" />
          <Skeleton className="h-10 w-36 rounded-lg" />
        </div>
      </div>
    </section>
  )
}

export function HeroSpotlight({
  recent,
  series,
}: {
  recent: AsyncResource<EpisodeWithSeries[]>
  series: AsyncResource<SeriesSummary[]>
}) {
  if (recent.error || series.error) {
    return (
      <ErrorPanel
        error={(recent.error ?? series.error) as NonNullable<typeof recent.error>}
        title="Spotlight could not be loaded"
        onRetry={() => {
          if (recent.error) {
            recent.refresh()
          }
          if (series.error) {
            series.refresh()
          }
        }}
      />
    )
  }

  if (recent.loading || series.loading) {
    return <HeroSkeleton />
  }

  const episode = [...recent.data].sort((a, b) => timestamp(b) - timestamp(a))[0]
  const match = episode
    ? series.data.find((item) => item.id === episode.seriesId)
    : undefined
  const fallbackSeries = series.data.find((item) => item.bannerUrl) ?? series.data[0]
  const title = episode?.title || match?.name || fallbackSeries?.name
  const description =
    match?.description ||
    fallbackSeries?.description ||
    "Your library, downloads and streaming sources orbit together in one cinematic hub."
  const bannerUrl = match?.bannerUrl ?? fallbackSeries?.bannerUrl ?? null
  const watchHref = episode ? `/watch?episodeId=${episode.id}` : "/watch"
  const detailHref = match || fallbackSeries
    ? `/series?id=${(match ?? fallbackSeries)?.id}`
    : "/explore"

  return (
    <section className="relative min-h-[22rem] overflow-hidden rounded-2xl border border-border/60 bg-surface-low lg:min-h-[26rem]">
      <HeroBackground imageUrl={bannerUrl} />
      <div className="relative flex min-h-[22rem] flex-col justify-end p-8 lg:min-h-[26rem] lg:p-12">
        <div className="max-w-2xl space-y-5">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="default" className="gap-1.5 bg-primary/15 text-primary">
              <SparklesIcon className="size-3" />
              Spotlight Arc
            </Badge>
            <Badge variant="outline">1080p ready</Badge>
            <Badge variant="outline">Soft subtitles</Badge>
            {match ? (
              <Badge variant="outline">
                {match.downloadedEpisodes}/{match.totalEpisodes} episodes available
              </Badge>
            ) : null}
          </div>
          <div className="space-y-2">
            <p className="telemetry text-[0.55rem] text-muted-foreground">
              {episode ? `EP ${episode.episodeNumber ?? "—"} · ${episode.seriesName}` : "Now boarding"}
            </p>
            <h1
              className={cn(
                "font-heading text-4xl font-extrabold leading-[1.05] tracking-tight text-foreground",
                "lg:text-6xl",
              )}
            >
              {title ?? "Everything you follow, in one orbit."}
            </h1>
          </div>
          <p className="line-clamp-2 max-w-xl text-sm leading-relaxed text-muted-foreground">
            {description}
          </p>
          <div className="flex flex-wrap items-center gap-3 pt-1">
            <Button size="lg" nativeButton={false} render={<Link href={watchHref} />}>
              <PlayIcon />
              Resume
            </Button>
            <Button
              size="lg"
              variant="outline"
              nativeButton={false}
              render={<Link href={detailHref} />}
            >
              View details
              <ArrowRightIcon />
            </Button>
            {!episode ? (
              <Button
                size="lg"
                variant="ghost"
                nativeButton={false}
                render={<Link href="/explore" />}
              >
                <CompassIcon />
                Explore sources
              </Button>
            ) : null}
          </div>
        </div>
      </div>
    </section>
  )
}
