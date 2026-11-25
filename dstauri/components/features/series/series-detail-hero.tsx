"use client"

import Link from "next/link"
import {
  ArrowDownIcon,
  ArrowRightIcon,
  HardDriveIcon,
  Layers3Icon,
  PlayIcon,
  RefreshCwIcon,
} from "lucide-react"

import { PosterCard } from "@/components/common/poster-card"
import { StatGrid } from "@/components/common/stat-grid"
import { StatusChip } from "@/components/common/status-chip"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { Episode, EpisodeWithSeries, SeriesSummary } from "@/lib/ipc/library"
import { backgroundImageUrl } from "@/lib/utils"

export function SeriesDetailHero({
  series,
  episodes,
  onSyncArchives,
  syncing = false,
}: {
  series: SeriesSummary
  episodes: Episode[] | EpisodeWithSeries[]
  onSyncArchives?: () => void
  syncing?: boolean
}) {
  const backdrop = backgroundImageUrl(series.bannerUrl)
  const firstEpisode = episodes[0]
  const watchHref = firstEpisode ? `/watch?episodeId=${firstEpisode.id}` : "/watch"
  const completion = series.totalEpisodes
    ? Math.round((series.downloadedEpisodes / series.totalEpisodes) * 100)
    : 0

  return (
    <section className="space-y-4">
      <div className="relative overflow-hidden rounded-2xl border border-border/60 bg-surface-low">
        {backdrop ? (
          <div
            aria-hidden
            className="absolute inset-0 bg-cover bg-center opacity-40"
            style={{ backgroundImage: backdrop }}
          />
        ) : null}
        <div
          aria-hidden
          className="absolute inset-0 bg-gradient-to-r from-surface-low via-surface-low/90 to-surface-low/40"
        />
        <div className="relative flex flex-col gap-6 p-6 lg:flex-row lg:items-end lg:p-8">
          <PosterCard
            title={series.name}
            meta={`${series.totalEpisodes} episodes`}
            imageUrl={series.bannerUrl}
            className="w-full shrink-0 lg:w-48"
            quickAction={null}
          />
          <div className="min-w-0 flex-1 space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <StatusChip
                tone={series.failedEpisodes > 0 ? "error" : completion === 100 ? "active" : "buffering"}
                label={
                  series.failedEpisodes > 0
                    ? "Needs attention"
                    : completion === 100
                      ? "Complete"
                      : "In progress"
                }
              />
              <Badge variant="outline">{completion}% archived</Badge>
              {series.bannerSource ? (
                <Badge variant="outline">Artwork · {series.bannerSource}</Badge>
              ) : null}
            </div>

            <div className="space-y-1">
              <p className="telemetry text-[0.55rem] text-muted-foreground">Dossier</p>
              <h2 className="font-heading text-headline-lg text-foreground">
                {series.name}
              </h2>
              <p className="text-body-md text-muted-foreground">
                Native key · <span className="font-mono">{series.key || "unassigned"}</span>
              </p>
            </div>

            <div className="flex flex-wrap gap-2">
              <Badge variant="secondary">{series.totalEpisodes} episodes</Badge>
              <Badge variant="secondary">{series.downloadedEpisodes} saved</Badge>
              <Badge variant="secondary">{series.queuedEpisodes} queued</Badge>
              <Badge variant="secondary">MP4 · MKV · SRT</Badge>
            </div>

            <div className="rounded-xl border border-border/60 bg-surface-lowest/70 p-4">
              <p className="telemetry text-[0.55rem] text-muted-foreground">Synopsis</p>
              <p className="mt-1.5 max-w-3xl text-body-md leading-relaxed text-foreground">
                {series.description || "No synopsis has been stored for this series yet."}
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Button nativeButton={false} render={<Link href={watchHref} />}>
                <PlayIcon />
                Resume
              </Button>
              <Button
                variant="outline"
                nativeButton={false}
                render={<Link href="#episodes" />}
              >
                <ArrowDownIcon />
                Jump to episodes
              </Button>
              {onSyncArchives ? (
                <Button variant="ghost" onClick={onSyncArchives} disabled={syncing}>
                  <RefreshCwIcon className={syncing ? "animate-spin" : undefined} />
                  {syncing ? "Syncing" : "Sync archives"}
                </Button>
              ) : null}
            </div>
          </div>
        </div>
      </div>

      <StatGrid
        items={[
          { label: "Episodes", value: series.totalEpisodes },
          { label: "Saved", value: series.downloadedEpisodes },
          { label: "Queued", value: series.queuedEpisodes },
          { label: "Failed", value: series.failedEpisodes },
        ]}
      />

      <div className="flex flex-wrap items-center gap-x-5 gap-y-2 rounded-xl border border-border/60 bg-surface-low px-4 py-3 text-xs text-muted-foreground">
        <span className="inline-flex min-w-0 items-center gap-2">
          <HardDriveIcon className="size-3.5 shrink-0 text-primary" />
          <span className="telemetry shrink-0">Save folder</span>
          <span className="truncate font-mono text-foreground">{series.saveDir || "—"}</span>
        </span>
        <span className="inline-flex min-w-0 items-center gap-2">
          <Layers3Icon className="size-3.5 shrink-0 text-primary" />
          <span className="telemetry shrink-0">Mirror</span>
          <span className="truncate font-mono text-foreground">
            {series.bannerSource || "Local catalog"}
          </span>
        </span>
        <Link
          href="/library"
          className="ml-auto inline-flex items-center gap-1 text-primary hover:underline"
        >
          Library
          <ArrowRightIcon className="size-3" />
        </Link>
      </div>
    </section>
  )
}
