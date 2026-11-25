"use client"

import { CalendarIcon, ClapperboardIcon, ClockIcon, LayersIcon } from "lucide-react"

import { StatusChip } from "@/components/common/status-chip"
import type { SourceSeries } from "@/lib/ipc/explore"
import { cn } from "@/lib/utils"

function MetaItem({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof CalendarIcon
  label: string
  value: string
}) {
  if (!value.trim()) return null
  return (
    <div className="flex items-start gap-2">
      <Icon className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
      <div className="min-w-0">
        <p className="telemetry text-[0.55rem] uppercase tracking-wide text-muted-foreground">
          {label}
        </p>
        <p className="truncate text-xs text-foreground">{value}</p>
      </div>
    </div>
  )
}

/** Poster + metadata for an online item. Purely presentational. */
export function ItemHero({ series }: { series: SourceSeries }) {
  const ongoing = /ongoing|airing|currently/i.test(series.status)
  const meta = [
    { icon: CalendarIcon, label: "Released", value: series.released },
    { icon: ClapperboardIcon, label: "Studio", value: series.studio },
    { icon: LayersIcon, label: "Episodes", value: String(series.episodeCount) },
    { icon: ClockIcon, label: "Duration", value: series.duration },
  ].filter((item) => item.value.trim() && item.value !== "0")

  return (
    <section className="relative overflow-hidden rounded-2xl border border-border/50 bg-surface-low">
      {series.thumbnail ? (
        <div
          className="absolute inset-0 scale-105 bg-cover bg-center opacity-20 blur-2xl"
          style={{ backgroundImage: `url(${series.thumbnail})` }}
          aria-hidden
        />
      ) : null}
      <div className="relative flex flex-col gap-6 p-5 sm:flex-row sm:p-7">
        {series.thumbnail ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={series.thumbnail}
            alt=""
            className="aspect-[2/3] h-56 w-auto shrink-0 rounded-xl border border-border/50 object-cover sm:h-64"
            loading="lazy"
            decoding="async"
          />
        ) : null}
        <div className="min-w-0 flex-1 space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            {series.kind ? (
              <span className="telemetry rounded-md bg-primary/15 px-2 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide text-primary">
                {series.kind}
              </span>
            ) : null}
            {series.status ? (
              <StatusChip
                tone={ongoing ? "active" : "idle"}
                label={series.status.toUpperCase()}
              />
            ) : null}
            {series.season ? (
              <span className="telemetry rounded-md bg-surface-high px-2 py-0.5 text-[0.6rem] uppercase tracking-wide text-muted-foreground">
                {series.season}
              </span>
            ) : null}
          </div>

          <div className="space-y-1">
            <h1 className="font-heading text-2xl font-bold tracking-tight text-foreground sm:text-3xl">
              {series.title}
            </h1>
            {series.network ? (
              <p className="text-sm text-muted-foreground">on {series.network}</p>
            ) : null}
          </div>

          <div className={cn("grid grid-cols-2 gap-3 sm:grid-cols-4")}>
            {meta.map((item) => (
              <MetaItem key={item.label} {...item} />
            ))}
          </div>

          {series.genres.length > 0 ? (
            <div className="flex flex-wrap gap-1.5">
              {series.genres.map((genre) => (
                <span
                  key={genre}
                  className="rounded-full border border-border/50 bg-surface-high/60 px-2.5 py-0.5 text-[0.65rem] text-muted-foreground"
                >
                  {genre}
                </span>
              ))}
            </div>
          ) : null}

          {series.description ? (
            <p className="max-w-3xl whitespace-pre-line text-sm leading-relaxed text-muted-foreground">
              {series.description}
            </p>
          ) : null}
        </div>
      </div>
    </section>
  )
}
