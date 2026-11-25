"use client"

import { Suspense, useCallback } from "react"
import { useSearchParams } from "next/navigation"
import { RadioIcon } from "lucide-react"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { PageHeader } from "@/components/common/page-header"
import { PlayerSurface } from "@/components/player/player-surface"
import { useAsyncResource } from "@/hooks/use-async-resource"
import { EMPTY_PLAYABLE, resolvePlayable } from "@/lib/ipc/player"

function OnlinePlayer() {
  const params = useSearchParams()
  const pageUrl = params.get("url") ?? ""
  const title = params.get("title") ?? "Online stream"

  const load = useCallback(
    () => (pageUrl ? resolvePlayable(pageUrl) : Promise.resolve(EMPTY_PLAYABLE)),
    [pageUrl],
  )
  const playable = useAsyncResource(load, EMPTY_PLAYABLE)

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Player"
        title={title}
        description="Online playback · the source link is resolved when you open it."
        actions={
          playable.data?.source ? (
            <span className="telemetry rounded-md bg-surface-high px-2 py-1 text-[0.6rem] uppercase tracking-wide text-muted-foreground">
              {playable.data.source}
              {playable.data.isHls ? " · HLS" : ""}
            </span>
          ) : undefined
        }
      />

      {playable.loading ? (
        <LoadingGrid count={1} />
      ) : playable.error ? (
        <ErrorPanel error={playable.error} title="This stream could not be resolved" />
      ) : playable.data.url ? (
        <PlayerSurface key={playable.data.url} src={playable.data.url} />
      ) : (
        <EmptyState
          icon={RadioIcon}
          title="No stream selected"
          description="Open an episode from Explore or an online item to start streaming."
        />
      )}
    </div>
  )
}

export default function StreamPage() {
  return (
    <Suspense fallback={<LoadingGrid count={1} />}>
      <OnlinePlayer />
    </Suspense>
  )
}
