"use client"

import { useCallback, useEffect, useMemo, useState } from "react"
import { DownloadIcon } from "lucide-react"
import { toast } from "sonner"

import { PageHeader } from "@/components/common/page-header"
import { StatusChip } from "@/components/common/status-chip"
import { ConsoleDrawer } from "@/components/features/downloads/console-drawer"
import { DiskUsageCard } from "@/components/features/downloads/disk-usage-card"
import { EpisodeSections } from "@/components/features/downloads/episode-sections"
import { LiveQueueSection } from "@/components/features/downloads/live-queue-section"
import { SessionOverview } from "@/components/features/downloads/session-overview"
import { Button } from "@/components/ui/button"
import { useDownloadQueue } from "@/hooks/use-downloads"
import {
  useArchiveSync,
  useLibraryStats,
  useRecentEpisodes,
} from "@/hooks/use-library"

const ACTIVE_PHASES = new Set(["queued", "paused", "resolving", "downloading", "retrying"])

export default function DownloadsPage() {
  const stats = useLibraryStats()
  const notStarted = useRecentEpisodes("not_started", 50)
  const queued = useRecentEpisodes("queued", 50)
  const failed = useRecentEpisodes("failed", 50)
  const downloaded = useRecentEpisodes("downloaded", 50)
  const archive = useArchiveSync()
  const queue = useDownloadQueue()
  const [bulkBusy, setBulkBusy] = useState(false)

  // The refresh callbacks are stable; the resource objects around them are not.
  const { refresh: refreshNotStarted } = notStarted
  const { refresh: refreshQueued } = queued
  const { refresh: refreshFailed } = failed
  const { refresh: refreshDownloaded } = downloaded
  const { refresh: refreshStats } = stats

  const rows = queue.rows
  const active = useMemo(
    () => rows.filter((row) => ACTIVE_PHASES.has(row.state)),
    [rows],
  )
  const finished = rows.length - active.length

  // Session telemetry: aggregate what the active jobs report so the second
  // stats row says something useful even when nothing is running.
  const session = useMemo(() => {
    const speedBps = active.reduce((sum, row) => sum + row.speedBps, 0)
    const downloadedBytes = active.reduce(
      (sum, row) => sum + row.downloadedBytes,
      0,
    )
    const etas = active
      .map((row) => row.etaSec)
      .filter((value) => Number.isFinite(value) && value > 0)
    return {
      speedBps,
      downloadedBytes,
      etaSec: etas.length > 0 ? Math.max(...etas) : 0,
    }
  }, [active])

  // One flat, copyable console log across every job in this session.
  const consoleLines = useMemo(
    () =>
      Object.entries(queue.consoleLines).flatMap(([episodeId, lines]) =>
        lines.map((line) => `[ep ${episodeId}] ${line}`),
      ),
    [queue.consoleLines],
  )

  const refreshCatalog = useCallback(() => {
    refreshNotStarted()
    refreshQueued()
    refreshFailed()
    refreshDownloaded()
    refreshStats()
  }, [refreshNotStarted, refreshQueued, refreshFailed, refreshDownloaded, refreshStats])

  // When a job reaches a terminal state the Rust worker has already written the
  // new status, so re-read the catalog lists to keep the counts honest.
  const settled = rows
    .filter((row) => !ACTIVE_PHASES.has(row.state))
    .map((row) => `${row.episodeId}:${row.state}`)
    .join(",")

  useEffect(() => {
    if (!settled) {
      return
    }
    refreshCatalog()
  }, [settled, refreshCatalog])

  const retry = (episodeId: number) => {
    void queue
      .start(episodeId)
      .then(() => toast.success("Download restarted", { description: `Episode #${episodeId}` }))
      .catch((error) => {
        const message = error instanceof Error ? error.message : String(error)
        toast.error("Could not restart download", { description: message })
      })
  }

  /** Queue a batch of episodes, then re-read the catalog lists. */
  const queueMany = useCallback(
    async (episodeIds: number[]) => {
      if (episodeIds.length === 0) return
      setBulkBusy(true)
      let started = 0
      for (const episodeId of episodeIds) {
        try {
          await queue.start(episodeId)
          started += 1
        } catch {
          // A single failure must not abort the batch; the queue shows it.
        }
      }
      setBulkBusy(false)
      refreshCatalog()
      if (started > 0) {
        toast.success(
          `Queued ${started} download${started === 1 ? "" : "s"}`,
          {
            description:
              started < episodeIds.length
                ? `${episodeIds.length - started} could not be queued — see the live queue.`
                : "Downloads run in the background; track them from the tray.",
          },
        )
      } else {
        toast.error("No downloads were queued", {
          description: "Check the live queue for the error.",
        })
      }
    },
    [queue, refreshCatalog],
  )

  const syncArchives = () => {
    const task = archive.sync()
    toast.promise(task, {
      loading: "Syncing downloaded archives…",
      success: "Archives synced",
      error: "Archive sync failed",
    })
  }

  const cancelActiveJobs = () => {
    for (const row of active) {
      void queue.cancel(row.episodeId)
    }
    toast("Cancelled active downloads", {
      description: `${active.length} job${active.length === 1 ? "" : "s"} stopped.`,
    })
  }

  const toggleQueue = () => {
    const next = !queue.paused
    toast.promise(queue.setPaused(next), {
      loading: next ? "Pausing the queue…" : "Resuming the queue…",
      success: next
        ? "Downloads paused — running jobs will finish"
        : "Downloads resumed",
      error: "Could not change the queue state",
    })
  }

  const clearFinishedJobs = () => {
    void queue.clear().then(() =>
      toast.success("Cleared finished downloads", {
        description: "Saved and failed rows were removed from the queue.",
      }),
    )
  }

  const removeQueued = (episodeId: number) => {
    void queue.remove(episodeId).then(() =>
      toast.success("Removed from queue", { description: `Episode #${episodeId}` }),
    )
  }

  const queueAllNotStarted = () =>
    void queueMany(notStarted.data.map((episode) => episode.id))

  return (
    <>
      <PageHeader
        eyebrow="Pipeline"
        title="Downloads"
        description="DonghuaStream episodes are resolved to a playable target at download time, retried with backoff, and only marked complete once every stream is saved and the merged file exists on disk."
        actions={
          <>
            <StatusChip
              tone={active.length > 0 ? "active" : "idle"}
              label={
                active.length > 0
                  ? `Engine running · ${active.length}`
                  : "Engine idle"
              }
            />
            <Button
              onClick={queueAllNotStarted}
              disabled={bulkBusy || notStarted.data.length === 0}
            >
              <DownloadIcon />
              Download all
            </Button>
            <ConsoleDrawer lines={consoleLines} />
            <Button
              variant="outline"
              onClick={syncArchives}
              disabled={archive.syncing}
            >
              {archive.syncing ? "Syncing" : "Sync archives"}
            </Button>
          </>
        }
      />

      <div className="space-y-6 px-6 py-6">
        <SessionOverview
          stats={stats}
          archive={archive}
          activeCount={active.length}
          session={session}
        />

        <LiveQueueSection
          queue={queue}
          activeCount={active.length}
          finished={finished}
          onTogglePause={toggleQueue}
          onCancelActive={cancelActiveJobs}
          onClearFinished={clearFinishedJobs}
        />

        <DiskUsageCard />

        <EpisodeSections
          notStarted={notStarted}
          queued={queued}
          failed={failed}
          downloaded={downloaded}
          bulkBusy={bulkBusy}
          onQueueMany={queueMany}
          onQueueAllNotStarted={queueAllNotStarted}
          onRemoveQueued={removeQueued}
          onRetry={retry}
        />
      </div>
    </>
  )
}
