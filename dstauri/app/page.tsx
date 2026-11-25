"use client"

import Link from "next/link"
import { CompassIcon, RadioIcon } from "lucide-react"

import { StatusChip } from "@/components/common/status-chip"
import {
  HomeLibrary,
  HomeStats,
  RecentDownloads,
} from "@/components/features/home/home-sections"
import { HeroSpotlight } from "@/components/features/home/hero-spotlight"
import { UpdatesSection } from "@/components/features/home/updates-section"
import { Button } from "@/components/ui/button"
import {
  useLibraryStats,
  useRecentEpisodes,
  useSeries,
} from "@/hooks/use-library"

const SOURCES = [
  {
    name: "YouTube",
    description: "Search the full YouTube catalog with your own API key.",
    status: "API key required",
    tone: "stalled" as const,
  },
  {
    name: "AnimeCube",
    description: "Browse the catalog and pick the episodes you want.",
    status: "Ready",
    tone: "active" as const,
  },
  {
    name: "DonghuaStream",
    description: "Trending series and the latest releases, no account needed.",
    status: "Ready",
    tone: "active" as const,
  },
]

export default function HomePage() {
  const stats = useLibraryStats()
  const series = useSeries()
  const recent = useRecentEpisodes("downloaded", 8)

  return (
    <div className="space-y-10 px-6 py-8">
      <HeroSpotlight recent={recent} series={series} />

      <HomeStats stats={stats} />
      <UpdatesSection />
      <RecentDownloads recent={recent} />
      <HomeLibrary series={series} />

      <section>
        <div className="mb-4">
          <p className="telemetry text-[0.55rem] text-muted-foreground">
            Connected catalogs
          </p>
          <h2 className="font-heading text-lg font-semibold tracking-tight text-foreground">
            Sources
          </h2>
        </div>
        <div className="grid gap-3 md:grid-cols-3">
          {SOURCES.map((source) => (
            <article
              key={source.name}
              className="rounded-xl border border-border/60 bg-surface-low p-4"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex min-w-0 items-center gap-2">
                  <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-surface-high ring-1 ring-border/60">
                    <RadioIcon className="size-4 text-primary" />
                  </span>
                  <h3 className="truncate font-heading text-sm font-semibold text-foreground">
                    {source.name}
                  </h3>
                </div>
                <StatusChip tone={source.tone} label={source.status} />
              </div>
              <p className="mt-3 text-sm text-muted-foreground">{source.description}</p>
            </article>
          ))}
        </div>
      </section>

      <div className="flex justify-center pb-2">
        <Button variant="ghost" nativeButton={false} render={<Link href="/explore" />}>
          <CompassIcon />
          Find something new
        </Button>
      </div>
    </div>
  )
}
