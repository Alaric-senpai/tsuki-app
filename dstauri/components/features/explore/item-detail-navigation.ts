import type { SourceCard, SourceEpisode } from "@/lib/ipc/explore"

export type DetailRoute = "item" | "show"

const ANIMEKHOR_SERIES_PREFIX = "https://animekhor.org/anime/"

/**
 * Detail pages need the canonical series slug/URL. AnimeKhor catalog cards
 * expose a numeric `rel` id as `card.id` and the real series page as `card.url`,
 * so prefer the URL there. Other sources already carry a usable id.
 */
export function detailIdForCard(card: SourceCard): string {
  if (card.source.trim().toLowerCase() === "animekhor" && card.url.trim()) {
    return card.url.trim()
  }
  return card.id
}

/** Build a detail link from a card without losing the canonical source id. */
export function itemDetailHref(card: SourceCard, route: DetailRoute = "item"): string {
  const params = new URLSearchParams({
    source: card.source,
    id: detailIdForCard(card),
  })
  return `/${route}?${params.toString()}`
}

/**
 * AnimeKhor may arrive with a full canonical URL (preferred), a bare slug, or
 * a legacy numeric `rel` id. Only the first two identify a detail page, so
 * strip the URL down to the slug the Rust source expects.
 */
export function animekhorSeriesId(
  rawId: string | null | undefined,
  url?: string | null,
): string {
  const candidate = (url && url.trim()) || (rawId && rawId.trim()) || ""
  if (!candidate || /^\d+$/.test(candidate)) return ""
  if (candidate.toLowerCase().startsWith(ANIMEKHOR_SERIES_PREFIX)) {
    const rest = candidate.slice(ANIMEKHOR_SERIES_PREFIX.length).split(/[?#]/)[0]
    return rest.replace(/\/+$/, "").split("/")[0]
  }
  if (/^https?:\/\//i.test(candidate)) {
    try {
      const parsed = new URL(candidate)
      return parsed.pathname.replace(/^\/+|\/+$/g, "").split("/").slice(-1)[0] ?? ""
    } catch {
      return candidate.replace(/^\/+|\/+$/g, "")
    }
  }
  return candidate.replace(/^\/+|\/+$/g, "")
}

/** Stream links carry the episode page URL; it is resolved at play time. */
export function episodeStreamHref(
  episode: SourceEpisode,
  source: string,
  seriesId: string,
): string {
  const params = new URLSearchParams({
    url: episode.url,
    title: episode.title,
    source,
    seriesId,
  })
  return `/stream?${params.toString()}`
}

/**
 * AnimeKhor episode links are article pages, never direct media URLs. Keep the
 * absolute page URL (the player resolves it) and pass the canonical series id
 * alongside it, even when the source returned a relative href.
 */
export function animekhorEpisodeStreamHref(
  episode: SourceEpisode,
  source: string,
  seriesId: string,
): string {
  const raw = episode.url.trim()
  const url = /^https?:\/\//i.test(raw)
    ? raw
    : `https://animekhor.org/${raw.replace(/^\/+/, "")}`
  const params = new URLSearchParams({
    url,
    title: episode.title,
    source,
    seriesId,
  })
  return `/stream?${params.toString()}`
}
