"use client"

import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react"
import { DownloadIcon } from "lucide-react"
import { toast } from "sonner"

import { ErrorPanel } from "@/components/common/data-state"
import { PageHeader } from "@/components/common/page-header"
import { StatGrid } from "@/components/common/stat-grid"
import { StatusChip } from "@/components/common/status-chip"
import { ConsoleDrawer } from "@/components/features/downloads/console-drawer"
import { DownloadQueueItem } from "@/components/features/downloads/download-queue-item"
import { EpisodeStatusTable } from "@/components/features/downloads/episode-status-table"
import { NotStartedSection } from "@/components/features/downloads/not-started-section"
import { Button } from "@/components/ui/button"
import { useDownloadQueue } from "@/hooks/use-downloads"
import {
  useArchiveSync,
  useLibraryStats,
  useRecentEpisodes,
} from "@/hooks/use-library"
import { formatBytes, formatEta, formatSpeed } from "@/lib/utils"

const ACTIVE_PHASES = new Set(["queued", "resolving", "downloading", "retrying"])

function SectionTitle({
  children,
  actions,
}: {
  children: string
  actions?: ReactNode
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <div className="flex items-center gap-2">
        <DownloadIcon className="size-4 text-primary" />
        <h2 className="font-heading text-lg font-semibold tracking-tight">
          {children}
        </h2>
      </div>
      {actions}
    </div>
  )
}

export default function DownloadsPage() {
  const stats = useLibraryStats()
  const notStarted = useRecentEpisodes("not_started", 50)
  const queued = useRecentEpisodes("queued", 50)
  const failed = useRecentEpisodes("failed", 50)
  const downloaded = useRecentEpisodes("downloaded", 50)
  const archive = useArchiveSync()
  const queue = useDownloadQueue()
  const [bulkBusy, setBulkBusy] = useState(false)

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
        <StatGrid
          items={[
            { label: "Queued", value: stats.data.queuedCount },
            {
              label: "Failed",
              value: stats.data.failedCount,
              hint: "Retry budget already spent",
            },
            { label: "Downloaded", value: stats.data.downloadedCount },
            {
              label: "Not started",
              value: stats.data.pendingCount,
              hint: "In the library, no download run yet",
            },
          ]}
        />

        <section className="space-y-3">
          <SectionTitle>This session</SectionTitle>
          <StatGrid
            items={[
              {
                label: "Active downloads",
                value: active.length,
                hint: "Running in the background",
              },
              { label: "Total speed", value: formatSpeed(session.speedBps) },
              { label: "Longest ETA", value: formatEta(session.etaSec) },
              {
                label: "Downloaded this session",
                value: formatBytes(session.downloadedBytes),
              },
            ]}
          />
        </section>

        {archive.error ? (
          <p className="text-sm text-destructive" role="alert">
            Archive sync failed: {archive.error.message}
          </p>
        ) : null}

        <section className="space-y-3">
          <SectionTitle
            actions={
              <div className="flex items-center gap-2">
                {active.length > 0 ? (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={cancelActiveJobs}
                  >
                    Cancel active
                  </Button>
                ) : null}
                {finished > 0 ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={clearFinishedJobs}
                  >
                    Clear finished
                  </Button>
                ) : null}
              </div>
            }
          >
            Live queue
          </SectionTitle>

          {queue.error ? (
            <ErrorPanel
              title="The download queue could not be read"
              error={queue.error}
            />
          ) : queue.loading ? (
            <div className="space-y-2" aria-busy="true" aria-label="Loading queue">
              {Array.from({ length: 2 }, (_, index) => (
                <div
                  key={index}
                  className="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-low px-4 py-3"
                >
                  <div className="size-8 shrink-0 animate-pulse rounded-lg bg-muted" />
                  <div className="flex-1 space-y-2">
                    <div className="h-3 w-2/3 animate-pulse rounded bg-muted" />
                    <div className="h-2 w-1/3 animate-pulse rounded bg-muted" />
                  </div>
                </div>
              ))}
            </div>
          ) : rows.length === 0 ? (
            <div className="flex items-center justify-between rounded-lg border border-border/60 bg-surface-low px-4 py-3">
              <span className="text-sm text-muted-foreground">
                Nothing has been queued in this session. Start a download from a
                series page or from the list below.
              </span>
            </div>
          ) : (
            <div className="space-y-2">
              {rows.map((snapshot) => (
                <DownloadQueueItem
                  key={snapshot.episodeId}
                  snapshot={snapshot}
                  consoleLines={queue.consoleLines[snapshot.episodeId]}
                  onCancel={(episodeId) => {
                    void queue.cancel(episodeId)
                  }}
                />
              ))}
            </div>
          )}
        </section>

        <section className="space-y-3">
          <SectionTitle
            actions={
              <Button
                variant="outline"
                size="sm"
                disabled={bulkBusy || notStarted.data.length === 0}
                onClick={queueAllNotStarted}
              >
                <DownloadIcon />
                Download all ({notStarted.data.length})
              </Button>
            }
          >
            Not started
          </SectionTitle>
          <NotStartedSection
            episodes={notStarted.data}
            loading={notStarted.loading}
            busy={bulkBusy}
            onDownload={queueMany}
          />
        </section>

        <section className="space-y-3">
          <SectionTitle>Waiting in queue</SectionTitle>
          <EpisodeStatusTable
            episodes={queued.data}
            loading={queued.loading}
            tone="buffering"
            label="QUEUED"
            emptyTitle="Nothing waiting"
            emptyDescription="No episodes are marked queued in the catalog."
            onRemove={removeQueued}
          />
        </section>

        <section className="space-y-3">
          <SectionTitle>Needs attention</SectionTitle>
          <EpisodeStatusTable
            episodes={failed.data}
            loading={failed.loading}
            tone="error"
            label="FAILED"
            emptyTitle="No failed downloads"
            emptyDescription="The retry budget has not been exhausted anywhere."
            actionLabel="Retry"
            onRetry={retry}
          />
        </section>

        <section className="space-y-3">
          <SectionTitle>Completed</SectionTitle>
          <EpisodeStatusTable
            episodes={downloaded.data}
            loading={downloaded.loading}
            tone="active"
            label="DOWNLOADED"
            emptyTitle="Nothing downloaded yet"
            emptyDescription="Saved episodes will appear here."
            watch
          />
        </section>
      </div>
    </>
  )
}
