"use client"

import { useCallback, useState } from "react"

import { useExploreSeries } from "@/hooks/use-explore"

import {
  animekhorEpisodeStreamHref,
  animekhorSeriesId,
  type DetailRoute,
} from "./item-detail-navigation"
import { ItemDetailView } from "./item-detail-view"

/** Copy chosen by the route that hosts the shared detail view. */
export interface DetailCopy {
  contextEyebrow: string
  loadingTitle: string
  emptyTitle: string
}

export interface SourceItemDetailsProps extends DetailCopy {
  rawId: string
  rawUrl?: string | null
  route: DetailRoute
}

/** DonghuaStream ids are already canonical page ids/slugs. */
export function DonghuaStreamItemDetails({
  rawId,
  route,
  ...copy
}: SourceItemDetailsProps) {
  const [refreshToken, setRefreshToken] = useState(0)
  const refresh = useCallback(() => setRefreshToken((token) => token + 1), [])
  const series = useExploreSeries("donghuastream", rawId, refreshToken)

  return (
    <ItemDetailView
      {...copy}
      route={route}
      source="donghuastream"
      seriesId={rawId}
      eyebrow="donghuastream"
      series={series.data}
      loading={series.loading}
      error={series.error}
      onRetry={refresh}
      onRefresh={refresh}
    />
  )
}

/**
 * AnimeKhor cards use a numeric `rel` id, so navigation passes the canonical
 * series URL instead. Accept both forms here and reduce them to the slug the
 * Rust source expects. Episode links stay article pages resolved at play time.
 */
export function AnimekhorItemDetails({
  rawId,
  rawUrl,
  route,
  ...copy
}: SourceItemDetailsProps) {
  const [refreshToken, setRefreshToken] = useState(0)
  const refresh = useCallback(() => setRefreshToken((token) => token + 1), [])
  const seriesId = animekhorSeriesId(rawId, rawUrl)
  const series = useExploreSeries("animekhor", seriesId, refreshToken)

  return (
    <ItemDetailView
      {...copy}
      route={route}
      source="animekhor"
      seriesId={seriesId}
      eyebrow="animekhor"
      series={series.data}
      loading={series.loading}
      error={series.error}
      onRetry={refresh}
      onRefresh={refresh}
      heroLabels={{ duration: "Runtime" }}
      buildEpisodeHref={animekhorEpisodeStreamHref}
    />
  )
}

/** Unknown/unconfigured sources keep the legacy generic behaviour. */
export function GenericItemDetails({
  rawId,
  route,
  source,
  ...copy
}: SourceItemDetailsProps & { source: string }) {
  const [refreshToken, setRefreshToken] = useState(0)
  const refresh = useCallback(() => setRefreshToken((token) => token + 1), [])
  const series = useExploreSeries(source, rawId, refreshToken)

  return (
    <ItemDetailView
      {...copy}
      route={route}
      source={source}
      seriesId={rawId}
      eyebrow={source.replace(/_/g, " ")}
      series={series.data}
      loading={series.loading}
      error={series.error}
      onRetry={refresh}
      onRefresh={refresh}
    />
  )
}
