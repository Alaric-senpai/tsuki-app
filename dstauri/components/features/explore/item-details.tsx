"use client"

import { type DetailRoute } from "./item-detail-navigation"
import {
  AnimekhorItemDetails,
  DonghuaStreamItemDetails,
  GenericItemDetails,
  type DetailCopy,
} from "./source-item-details"

const COPY: Record<DetailRoute, DetailCopy> = {
  item: {
    contextEyebrow: "Explore",
    loadingTitle: "Item details",
    emptyTitle: "Item not found",
  },
  show: {
    contextEyebrow: "Source catalog",
    loadingTitle: "Show detail",
    emptyTitle: "Show not found",
  },
}

/** Route-level mapping from `?source=` to a thin, source-specific detail view. */
export function ItemDetails({
  source,
  id,
  url = null,
  route = "item",
}: {
  source: string
  id: string
  url?: string | null
  route?: DetailRoute
}) {
  const copy = COPY[route]
  const key = source.trim().toLowerCase()

  if (key === "donghuastream") {
    return <DonghuaStreamItemDetails {...copy} route={route} rawId={id} />
  }
  if (key === "animekhor") {
    return (
      <AnimekhorItemDetails {...copy} route={route} rawId={id} rawUrl={url} />
    )
  }
  return (
    <GenericItemDetails {...copy} route={route} rawId={id} source={source} />
  )
}
