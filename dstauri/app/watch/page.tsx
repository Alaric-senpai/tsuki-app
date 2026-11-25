"use client"

import { Suspense, useCallback, useRef } from "react"
import { useSearchParams } from "next/navigation"
import { convertFileSrc } from "@tauri-apps/api/core"
import { ExternalLinkIcon, FilmIcon } from "lucide-react"
import { toast } from "sonner"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { PageHeader } from "@/components/common/page-header"
import { Button } from "@/components/ui/button"
import { useAsyncResource } from "@/hooks/use-async-resource"
import { openInExternalPlayer } from "@/lib/ipc/config"
import { downloadState, downloadedEpisodePath } from "@/lib/ipc/downloads"
import {
  getWatchProgress,
  isWatched,
  isResumable,
  saveWatchProgress,
  type WatchProgress,
} from "@/lib/ipc/watch-progress"
import { formatClock } from "@/lib/utils"

type LocalSource = {
  path: string
  title: string
  /** Saved position to resume from, already filtered to "worth resuming". */
  resume: WatchProgress | null
}

/** Minimum playback progress (seconds) before a write is sent to SQLite. */
const SAVE_INTERVAL_SEC = 5

function LocalPlayer() {
  const params = useSearchParams()
  const rawPath = params.get("path") ?? ""
  const episodeIdParam = params.get("episodeId")
  const title = params.get("title") ?? "Local file"

  const episodeId = Number(episodeIdParam ?? "")
  const hasEpisodeId = Number.isFinite(episodeId) && episodeId > 0

  const videoRef = useRef<HTMLVideoElement | null>(null)
  const lastSavedRef = useRef(0)

  const load = useCallback(async (): Promise<LocalSource> => {
    let path = rawPath
    let resolvedTitle = title

    if (!path && hasEpisodeId) {
      const snapshot = await downloadState(episodeId)
      if (snapshot?.filePath) {
        path = snapshot.filePath
        resolvedTitle = snapshot.title || title
      } else {
        // Queue rows are in-memory; the catalog keeps the path across launches.
        path = (await downloadedEpisodePath(episodeId)) ?? ""
      }
    }

    if (!path || !hasEpisodeId) {
      return { path, title: resolvedTitle, resume: null }
    }

    // Resume state is a nicety: a failure here must never block playback.
    const progress = await getWatchProgress(episodeId).catch(() => null)
    return { path, title: resolvedTitle, resume: isResumable(progress) ? progress : null }
  }, [rawPath, title, hasEpisodeId, episodeId])
  const source = useAsyncResource(load, { path: "", title, resume: null })

  const openExternal = useCallback(() => {
    const path = source.data.path
    if (!path) return
    void openInExternalPlayer(path).then(
      () => toast.success("Opened in your external player"),
      (cause: unknown) =>
        toast.error("Could not open the external player", {
          description: cause instanceof Error ? cause.message : String(cause),
        }),
    )
  }, [source.data.path])

  const persist = useCallback(
    (options?: { force?: boolean; completed?: boolean }) => {
      const video = videoRef.current
      if (!video || !hasEpisodeId) return

      const position = Number.isFinite(video.currentTime) ? video.currentTime : 0
      if (!options?.force && Math.abs(position - lastSavedRef.current) < SAVE_INTERVAL_SEC) {
        return
      }
      lastSavedRef.current = position

      const duration = Number.isFinite(video.duration) ? video.duration : 0
      const completed = options?.completed ?? isWatched(position, duration)
      void saveWatchProgress(episodeId, position, duration, completed).catch(() => {
        // Losing a resume point is not worth interrupting playback.
      })
    },
    [episodeId, hasEpisodeId],
  )

  const applyResume = useCallback(
    (video: HTMLVideoElement) => {
      const resume = source.data.resume
      if (!resume) return
      const duration = Number.isFinite(video.duration) ? video.duration : 0
      // Never seek into the credits, and never seek past the end.
      if (duration > 0 && resume.positionSec >= duration - SAVE_INTERVAL_SEC) return
      video.currentTime = resume.positionSec
      lastSavedRef.current = resume.positionSec
      toast.info("Resumed where you left off", {
        description: `Continuing at ${formatClock(resume.positionSec)}`,
      })
    },
    [source.data.resume],
  )

  const playablePath = source.data.path

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Player"
        title={source.data.title || "Local playback"}
        description="Downloaded files play here without a system media player."
        actions={
          playablePath ? (
            <Button variant="outline" size="sm" onClick={openExternal}>
              <ExternalLinkIcon />
              Open in my player
            </Button>
          ) : undefined
        }
      />

      {source.loading ? (
        <LoadingGrid count={1} />
      ) : source.error ? (
        <ErrorPanel error={source.error} title="This file could not be opened" />
      ) : playablePath ? (
        <video
          key={playablePath}
          ref={videoRef}
          controls
          playsInline
          className="aspect-video w-full rounded-xl bg-black"
          src={convertFileSrc(playablePath)}
          onLoadedMetadata={(event) => applyResume(event.currentTarget)}
          onTimeUpdate={() => persist()}
          onPause={() => persist({ force: true })}
          onSeeked={() => persist({ force: true })}
          onEnded={() => persist({ force: true, completed: true })}
          onError={(event) => {
            const media = event.currentTarget.error
            toast.error("Playback failed", {
              description: media
                ? `${media.code}: ${media.message}`
                : "The video could not be decoded by this webview. Try “Open in my player”.",
            })
          }}
        />
      ) : (
        <EmptyState
          icon={FilmIcon}
          title="No local file selected"
          description="Download an episode, then choose Watch from the library."
        />
      )}
    </div>
  )
}

export default function WatchPage() {
  return (
    <Suspense fallback={<LoadingGrid count={1} />}>
      <LocalPlayer />
    </Suspense>
  )
}
