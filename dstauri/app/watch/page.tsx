"use client"

import { Suspense, useCallback, useRef } from "react"
import { useSearchParams } from "next/navigation"
import { ExternalLinkIcon, FilmIcon, Minimize2Icon } from "lucide-react"
import { toast } from "sonner"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { PageHeader } from "@/components/common/page-header"
import { VideoSurface } from "@/components/player/video-surface"
import { Button } from "@/components/ui/button"
import { useAsyncResource } from "@/hooks/use-async-resource"
import { getConfig, openInExternalPlayer } from "@/lib/ipc/config"
import { downloadState, downloadedEpisodePath } from "@/lib/ipc/downloads"
import {
  getWatchProgress,
  isWatched,
  isResumable,
  saveWatchProgress,
  type WatchProgress,
} from "@/lib/ipc/watch-progress"
import type { EnginePreference } from "@/lib/player/engine"
import { useMiniPlayer } from "@/stores/mini-player"
import { formatClock } from "@/lib/utils"

type LocalSource = {
  path: string
  title: string
  engine: EnginePreference
  /** Saved position to resume from, already filtered to "worth resuming". */
  resume: WatchProgress | null
}

/** Minimum playback progress (seconds) before a write is sent to SQLite. */
const SAVE_INTERVAL_SEC = 5
const ENGINE_PREFERENCES: EnginePreference[] = ["auto", "mpv", "html", "external"]

function LocalPlayer() {
  const params = useSearchParams()
  const rawPath = params.get("path") ?? ""
  const episodeIdParam = params.get("episodeId")
  const title = params.get("title") ?? "Local file"
  const mini = useMiniPlayer()

  const episodeId = Number(episodeIdParam ?? "")
  const hasEpisodeId = Number.isFinite(episodeId) && episodeId > 0
  const lastSavedRef = useRef(0)
  const lastDurationRef = useRef(0)

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

    const config = await getConfig().catch(() => null)
    const configured = config?.player.engine ?? "auto"
    const engine: EnginePreference = ENGINE_PREFERENCES.includes(configured as EnginePreference)
      ? (configured as EnginePreference)
      : "auto"

    if (!path || !hasEpisodeId) {
      return { path, title: resolvedTitle, engine, resume: null }
    }

    // Resume state is a nicety: a failure here must never block playback.
    const progress = await getWatchProgress(episodeId).catch(() => null)
    return {
      path,
      title: resolvedTitle,
      engine,
      resume: isResumable(progress) ? progress : null,
    }
  }, [rawPath, title, hasEpisodeId, episodeId])
  const source = useAsyncResource(load, {
    path: "",
    title,
    engine: "auto" as EnginePreference,
    resume: null,
  })

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
    (position: number, duration: number, completed = false) => {
      if (!hasEpisodeId || !Number.isFinite(position)) return
      if (!completed && Math.abs(position - lastSavedRef.current) < SAVE_INTERVAL_SEC) return
      lastSavedRef.current = position
      if (Number.isFinite(duration) && duration > 0) lastDurationRef.current = duration
      const done = completed || isWatched(position, duration)
      void saveWatchProgress(episodeId, position, duration, done).catch(() => {
        // Losing a resume point is not worth interrupting playback.
      })
    },
    [episodeId, hasEpisodeId],
  )

  const handleProgress = useCallback(
    (position: number, duration: number) => persist(position, duration),
    [persist],
  )
  const handleEnded = useCallback(() => {
    persist(lastSavedRef.current, lastDurationRef.current, true)
  }, [persist])

  const playablePath = source.data.path
  const resume = source.data.resume
  const startAt = resume && resume.positionSec > 0 ? resume.positionSec : undefined

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Player"
        title={source.data.title || "Local playback"}
        description="Downloaded files play here without a system media player."
        actions={
          playablePath ? (
            <>
              <Button variant="outline" size="sm" onClick={openExternal}>
                <ExternalLinkIcon />
                Open in my player
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  mini.open(
                    { kind: "file", path: playablePath, title: source.data.title },
                    source.data.title,
                  )
                }
              >
                <Minimize2Icon />
                Mini player
              </Button>
            </>
          ) : undefined
        }
      />

      {source.loading ? (
        <LoadingGrid count={1} />
      ) : source.error ? (
        <ErrorPanel error={source.error} title="This file could not be opened" />
      ) : playablePath ? (
        <>
          {resume ? (
            <p className="font-telemetry-micro text-telemetry-micro text-on-surface-variant">
              Resuming at {formatClock(resume.positionSec)}
            </p>
          ) : null}
          <VideoSurface
            key={playablePath}
            source={{ kind: "file", path: playablePath, title: source.data.title }}
            preference={source.data.engine}
            startAt={startAt}
            onProgress={handleProgress}
            onEnded={handleEnded}
            onMiniPlayer={() =>
              mini.open(
                { kind: "file", path: playablePath, title: source.data.title },
                source.data.title,
              )
            }
          />
          <p className="font-telemetry-micro text-telemetry-micro text-on-surface-variant">
            Shortcuts: space/k play-pause · ←/→ seek 10s · j/l seek 60s · f fullscreen · m mute
          </p>
        </>
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
