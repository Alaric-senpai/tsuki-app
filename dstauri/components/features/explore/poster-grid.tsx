"use client"

import { CompassIcon } from "lucide-react"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { PosterCard } from "@/components/common/poster-card"
import type { AppIpcError } from "@/lib/ipc/client"
import type { SourceCard } from "@/lib/ipc/explore"

import { detailIdForCard } from "./item-detail-navigation"

export function PosterGrid({
  cards,
  loading,
  error,
  onSelect,
  emptyTitle = "No series found",
  emptyDescription = "Try a different genre, order or search term.",
}: {
  cards: SourceCard[]
  loading: boolean
  error: AppIpcError | null
  onSelect?: (card: SourceCard) => void
  emptyTitle?: string
  emptyDescription?: string
}) {
  if (loading) {
    return (
      <LoadingGrid
        count={12}
        className="grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6"
      />
    )
  }

  if (error) {
    return <ErrorPanel title="The catalog could not be loaded" error={error} />
  }

  if (cards.length === 0) {
    return (
      <EmptyState
        icon={CompassIcon}
        title={emptyTitle}
        description={emptyDescription}
      />
    )
  }

  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
      {cards.map((card) => (
        <button
          key={card.url}
          type="button"
          className="block w-full cursor-pointer text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-lg"
          onClick={() =>
            onSelect?.({
              ...card,
              // AnimeKhor cards carry a numeric `rel` id; detail pages need the
              // canonical series URL instead. Other sources keep their id.
              id: detailIdForCard(card),
            })
          }
        >
          <PosterCard
            title={card.title}
            meta={card.episodeText || card.kind || undefined}
            imageUrl={card.thumbnail || null}
            badges={
              card.status ? (
                <span className="inline-flex items-center rounded-md bg-primary-container px-1.5 py-0.5 font-heading text-[0.5rem] font-semibold uppercase text-on-primary-container">
                  {card.status}
                </span>
              ) : undefined
            }
            quickAction={null}
            className="w-full"
          />
        </button>
      ))}
    </div>
  )
}
