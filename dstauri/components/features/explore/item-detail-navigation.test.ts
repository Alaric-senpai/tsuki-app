import { describe, expect, it } from "vitest"

import type { SourceCard, SourceEpisode } from "@/lib/ipc/explore"

import {
  animekhorEpisodeStreamHref,
  animekhorSeriesId,
  detailIdForCard,
  episodeStreamHref,
  itemDetailHref,
} from "./item-detail-navigation"

function card(overrides: Partial<SourceCard> = {}): SourceCard {
  return {
    id: "28305",
    title: "Renegade Immortal",
    url: "https://animekhor.org/anime/renegade-immortal/",
    thumbnail: "",
    status: "",
    kind: "",
    episodeText: "",
    source: "animekhor",
    ...overrides,
  }
}

function episode(overrides: Partial<SourceEpisode> = {}): SourceEpisode {
  return {
    url: "https://animekhor.org/renegade-immortal-episode-168/",
    title: "Episode 168",
    episodeNumber: 168,
    numberText: "168",
    date: "",
    ...overrides,
  }
}

describe("detailIdForCard", () => {
  it("prefers the canonical URL for AnimeKhor's numeric rel ids", () => {
    expect(detailIdForCard(card())).toBe("https://animekhor.org/anime/renegade-immortal/")
  })

  it("keeps ids from other sources and cards without a URL", () => {
    expect(detailIdForCard(card({ source: "donghuastream" }))).toBe("28305")
    expect(detailIdForCard(card({ url: "" }))).toBe("28305")
  })
})

describe("itemDetailHref", () => {
  it("encodes the canonical id and remembers the host route", () => {
    expect(itemDetailHref(card())).toContain("id=https%3A%2F%2Fanimekhor.org%2Fanime%2Frenegade-immortal%2F")
    expect(itemDetailHref(card(), "show")).toMatch(/^\/show\?/)
    expect(itemDetailHref(card({ source: "donghuastream" }))).toContain("id=28305")
  })
})

describe("animekhorSeriesId", () => {
  it("accepts a full series URL", () => {
    expect(animekhorSeriesId("", "https://animekhor.org/anime/renegade-immortal/")).toBe(
      "renegade-immortal",
    )
  })

  it("accepts a slug passed as the id", () => {
    expect(animekhorSeriesId("renegade-immortal", null)).toBe("renegade-immortal")
  })

  it("rejects numeric rel ids that cannot identify a detail page", () => {
    expect(animekhorSeriesId("28305", null)).toBe("")
    expect(animekhorSeriesId(null, null)).toBe("")
  })

  it("reduces an episode URL to a best-effort slug", () => {
    expect(
      animekhorSeriesId("", "https://animekhor.org/anime/renegade-immortal/ep-168/"),
    ).toBe("renegade-immortal")
  })
})

describe("stream links", () => {
  it("preserves an absolute episode page URL for the generic table", () => {
    const href = episodeStreamHref(episode(), "donghuastream", "id-1")
    expect(href).toContain("url=https%3A%2F%2Fanimekhor.org")
    expect(href).toContain("seriesId=id-1")
  })

  it("makes AnimeKhor relative links absolute", () => {
    const href = animekhorEpisodeStreamHref(
      episode({ url: "/renegade-immortal-episode-168/" }),
      "animekhor",
      "renegade-immortal",
    )
    expect(href).toContain("url=https%3A%2F%2Fanimekhor.org%2Frenegade-immortal-episode-168%2F")
  })
})
