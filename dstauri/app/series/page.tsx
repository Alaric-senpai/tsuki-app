"use client"

import { Suspense, useCallback, useMemo, useState } from "react"
import Link from "next/link"
import { useSearchParams } from "next/navigation"
import { toast } from "sonner"
import {
  ArrowLeftIcon,
  BellIcon,
  BellOffIcon,
  LayersIcon,
  PlusIcon,
  MoreHorizontalIcon,
  DownloadIcon,
  PlayIcon,
  RadioIcon,
} from "lucide-react"

import { EmptyState } from "@/components/common/empty-state"
import { ErrorPanel } from "@/components/common/data-state"
import { PageHeader } from "@/components/common/page-header"
import { SeriesDetailHero } from "@/components/features/series/series-detail-hero"
import { DataTable } from "@/components/common/data-table"
import { StatusChip } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { type ColumnDef, type TableFeatures } from "@tanstack/table-core"
import { tableFeatures, createColumnHelper } from "@tanstack/react-table"
import { Skeleton } from "@/components/ui/skeleton"
import {
  useArchiveSync,
  useEpisodes,
  useSeriesDetail,
  useSeriesMutations,
  useEpisodeMutations,
  useBannerPick,
  useUpdatePreference,
} from "@/hooks/use-library"
import { EpisodeDialog } from "@/components/features/series/episode-dialog"
import { SeriesEditDialog } from "@/components/features/series/series-edit-dialog"
import { useDownloadQueue } from "@/hooks/use-downloads"
import { toAppIpcError, type AppIpcError } from "@/lib/ipc/client"
import type { Episode } from "@/lib/ipc/library"
import { cn } from "@/lib/utils"

const STATUS_FILTERS = [
  { label: "All", value: undefined },
  { label: "Not started", value: "not_started" },
  { label: "Queued", value: "queued" },
  { label: "Downloaded", value: "downloaded" },
  { label: "Failed", value: "failed" },
] as const

type Status = (typeof STATUS_FILTERS)[number]["label"]

const helper = createColumnHelper<ReturnType<typeof tableFeatures>, Episode>()

const getEpisodeId = (episode: Episode) => String(episode.id)

function DeleteConfirmDialog({
  open,
  onOpenChange,
  episode,
  onConfirm,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  episode: Episode
  onConfirm: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete episode</DialogTitle>
          <DialogDescription>
            Are you sure you want to remove “{episode.title}” from the library?
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={onConfirm}>
            Delete
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function SeriesDetail() {
  const id = useSearchParams().get("id")
  const [status, setStatus] = useState<Status>("All")
  const [query, setQuery] = useState("")
  const [editOpen, setEditOpen] = useState(false)
  const [addOpen, setAddOpen] = useState(false)
  const [editEpisode, setEditEpisode] = useState<Episode | null>(null)
  const [deleteEpisode, setDeleteEpisode] = useState<Episode | null>(null)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [downloadError, setDownloadError] = useState<AppIpcError | null>(null)
  const series = useSeriesDetail(id)
  const archive = useArchiveSync()
  const episodes = useEpisodes(
    series.data?.id ?? null,
    STATUS_FILTERS.find((option) => option.label === status)?.value,
  )
  const seriesMutations = useSeriesMutations()
  const episodeMutations = useEpisodeMutations()
  const bannerPick = useBannerPick()
  const updatePreference = useUpdatePreference()
  const { start: startJob } = useDownloadQueue()

  const startDownload = useCallback(
    (episode: Episode) => {
      setDownloadError(null)
      void startJob(episode.id)
        .then(() => {
          toast.success("Download started", { description: episode.title })
        })
        .catch((cause: unknown) => {
          const error = toAppIpcError(cause)
          setDownloadError(error)
          toast.error("Could not start download", {
            description: error.detail ?? error.message,
          })
        })
    },
    [startJob],
  )

  const [bulkBusy, setBulkBusy] = useState(false)
  const [bulkDeleteOpen, setBulkDeleteOpen] = useState(false)

  /** Queue every episode that is neither downloaded nor already queued. */
  const queueEpisodes = useCallback(
    async (list: Episode[]) => {
      setDownloadError(null)
      const targets = list.filter(
        (episode) =>
          episode.status !== "downloaded" && episode.status !== "queued",
      )
      if (targets.length === 0) {
        return
      }
      setBulkBusy(true)
      const failures: string[] = []
      for (const episode of targets) {
        try {
          await startJob(episode.id)
        } catch (cause) {
          const error = toAppIpcError(cause)
          failures.push(`${episode.title}: ${error.message}`)
        }
      }
      setBulkBusy(false)
      const started = targets.length - failures.length
      if (started > 0) {
        toast.success(`Queued ${started} episode(s) for download`)
      }
      if (failures.length > 0) {
        setDownloadError({
          code: "bulk_download",
          message: `${failures.length} episode(s) could not be queued.`,
          detail: failures.join("\n"),
        })
        toast.error(`Could not queue ${failures.length} episode(s)`, {
          description: failures.join("\n"),
        })
      }
    },
    [startJob],
  )

  const visible = useMemo(() => {
    const list = episodes.data ?? []
    const needle = query.trim().toLowerCase()
    if (!needle) {
      return list
    }
    return list.filter(
      (episode) =>
        episode.title.toLowerCase().includes(needle) ||
        episode.videoId.toLowerCase().includes(needle),
    )
  }, [episodes.data, query])

  const detail = series.data

  // DataTable owns the selection checkbox column, so it is not defined here.
  const columns = useMemo(() => {
    return helper.columns([
      helper.accessor((row) => row.title, {
        id: "episode",
        header: "Episode",
        cell: (info) => (
          <div className="min-w-0">
            <p className="truncate text-sm font-medium text-foreground">
              {info.row.original.episodeNumber === null
                ? info.row.original.title
                : `EP ${String(info.row.original.episodeNumber).padStart(2, "0")} · ${info.row.original.title}`}
            </p>
            <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
              {info.row.original.videoId}
            </p>
          </div>
        ),
      }),
      helper.accessor((row) => row.status, {
        id: "status",
        header: "Status",
        cell: (info) => (
          <StatusChip
            tone={
              info.getValue() === "downloaded"
                ? "active"
                : info.getValue() === "queued"
                  ? "buffering"
                  : info.getValue() === "failed"
                    ? "error"
                    : "idle"
            }
            label={info.getValue().replace(/_/g, " ").toUpperCase()}
          />
        ),
      }),
      helper.display({
        id: "play",
        header: "Stream / Watch",
        cell: (info) => {
          const episode = info.row.original
          const downloaded = episode.status === "downloaded"
          const params = new URLSearchParams({
            title: episode.title,
          })
          const href = downloaded
            ? `/watch?episodeId=${episode.id}&${params.toString()}`
            : `/stream?url=${encodeURIComponent(episode.url)}&${params.toString()}`
          return (
            <Button
              variant="outline"
              size="xs"
              nativeButton={false}
              render={<Link href={href} />}
            >
              {downloaded ? <PlayIcon /> : <RadioIcon />}
              {downloaded ? "Watch" : "Stream"}
            </Button>
          )
        },
      }),
      helper.display({
        id: "actions",
        header: "Actions",
        cell: (info) => {
          const episode = info.row.original
          return (
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`Actions for ${episode.title}`}
                  >
                    <MoreHorizontalIcon />
                  </Button>
                }
              />
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  disabled={episode.status === "downloaded"}
                  onClick={() => startDownload(episode)}
                >
                  <DownloadIcon />
                  Download
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setEditEpisode(episode)}>
                  Edit
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setDeleteEpisode(episode)}>
                  Delete
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )
        },
      }),
    ]) as unknown as ColumnDef<TableFeatures, Episode, unknown>[]
  }, [startDownload])

  const handleSaveSeries = useCallback(
    (values: {
      name: string
      description: string
      saveDir: string
      bannerSource: string
    }) => {
      if (!detail) return
      void seriesMutations.updateSeries({
        id: detail.id,
        name: values.name,
        saveDir: values.saveDir,
        description: values.description,
      })
      setEditOpen(false)
    },
    [detail, seriesMutations],
  )

  const handlePickBanner = useCallback(() => {
    void bannerPick.pick(detail?.bannerSource ?? undefined)
  }, [bannerPick, detail])

  const handleCreateEpisode = useCallback(
    (values: {
      videoId: string
      url: string
      episodeNumber: number | null
      title: string
      status: string
    }) => {
      if (!detail) return
      void episodeMutations.createEpisode({
        seriesId: detail.id,
        videoId: values.videoId,
        url: values.url,
        episodeNumber: values.episodeNumber,
        title: values.title,
      })
      setAddOpen(false)
    },
    [detail, episodeMutations],
  )

  const handleUpdateEpisode = useCallback(
    (values: {
      videoId: string
      url: string
      episodeNumber: number | null
      title: string
      status: string
    }) => {
      if (!editEpisode) return
      void episodeMutations.updateEpisode({
        id: editEpisode.id,
        seriesId: editEpisode.seriesId,
        videoId: values.videoId,
        url: values.url,
        episodeNumber: values.episodeNumber,
        title: values.title,
        status: values.status,
      })
      setEditEpisode(null)
    },
    [editEpisode, episodeMutations],
  )

  const handleDeleteEpisode = useCallback(async () => {
    if (!deleteEpisode) return
    const removed = await episodeMutations.deleteEpisode(deleteEpisode.id)
    setDeleteEpisode(null)
    if (removed) {
      episodes.refresh()
      series.refresh()
    }
  }, [deleteEpisode, episodeMutations, episodes, series])

  const handleBulkDelete = useCallback(async () => {
    const ids = visible
      .filter((episode) => selectedIds.has(String(episode.id)))
      .map((episode) => episode.id)
    setBulkDeleteOpen(false)
    if (ids.length === 0) return
    setBulkBusy(true)
    for (const episodeId of ids) {
      await episodeMutations.deleteEpisode(episodeId)
    }
    setBulkBusy(false)
    setSelectedIds(new Set())
    episodes.refresh()
    series.refresh()
  }, [visible, selectedIds, episodeMutations, episodes, series])

  const toggleUpdates = useCallback(async () => {
    if (!detail) return
    const enabled = !detail.checkUpdates
    const ok = await updatePreference.set(detail.id, enabled)
    if (ok) {
      toast.success(
        enabled ? "Checking this series for updates" : "Update checks disabled",
      )
      series.refresh()
    } else {
      toast.error("Could not change the update setting")
    }
  }, [detail, updatePreference, series])

  return (
    <>
      <PageHeader
        eyebrow="Series"
        title={detail?.name ?? "Series detail"}
        description={" "}
        actions={
          <>
            <Button
              variant="ghost"
              nativeButton={false}
              render={<Link href="/library" />}
            >
              <ArrowLeftIcon />
              Back to library
            </Button>
            {detail && (
              <Button variant="outline" onClick={() => setEditOpen(true)}>
                Edit series
              </Button>
            )}
            {detail && (
              <Button variant="outline" onClick={() => setAddOpen(true)}>
                <PlusIcon />
                Add episode
              </Button>
            )}
            {detail && detail.sourceUrl ? (
              <Button
                variant="outline"
                disabled={updatePreference.busy}
                onClick={() => void toggleUpdates()}
                title="Check this series for new episodes from its source"
              >
                {detail.checkUpdates ? <BellIcon /> : <BellOffIcon />}
                {detail.checkUpdates ? "Updates on" : "Check for updates"}
              </Button>
            ) : null}
          </>
        }
      />

      <div className="space-y-5 px-6 py-6">
        {!id ? (
          <EmptyState
            icon={LayersIcon}
            title="Choose a series first"
            description="Open a series from your library to inspect its episode registry."
          />
        ) : series.error ? (
          <ErrorPanel error={series.error} onRetry={series.refresh} />
        ) : series.loading || !detail ? (
          <Skeleton className="h-64 rounded-2xl" />
        ) : (
          <>
            <SeriesDetailHero
              series={detail}
              episodes={episodes.data}
              onSyncArchives={() => void archive.sync()}
              syncing={archive.syncing}
            />

            {archive.error ? (
              <p className="text-sm text-destructive" role="alert">
                Archive sync failed: {archive.error.message}
              </p>
            ) : null}

            {downloadError ? (
              <ErrorPanel
                title="The episode could not be queued"
                error={downloadError}
              />
            ) : null}

            <section id="episodes" className="scroll-mt-6 space-y-5">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex flex-wrap items-center gap-2">
                  {STATUS_FILTERS.map((option) => (
                    <Button
                      key={option.label}
                      variant="ghost"
                      onClick={() => setStatus(option.label)}
                      className={cn(
                        "h-7 rounded-full border px-3 text-xs font-medium transition-colors",
                        status === option.label
                          ? "border-primary/40 bg-primary/10 text-primary"
                          : "border-border/60 bg-surface-low text-muted-foreground hover:text-foreground",
                      )}
                    >
                      {option.label}
                    </Button>
                  ))}
                </div>
                <div className="flex w-full items-center gap-2 sm:w-auto">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={bulkBusy}
                    onClick={() => void queueEpisodes(visible)}
                  >
                    <DownloadIcon />
                    Download all
                  </Button>
                  <div className="relative flex-1 sm:w-64 sm:flex-none">
                    <input
                      type="search"
                      value={query}
                      onChange={(event) => setQuery(event.target.value)}
                      placeholder="Search episodes"
                      aria-label="Search episodes"
                      className="h-9 w-full rounded-lg border border-input bg-surface-low pl-9 pr-3 text-sm outline-none focus:border-ring focus:ring-3 focus:ring-ring/50 disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-input/50 disabled:opacity-50 md:text-sm dark:bg-input/30 dark:disabled:bg-input/80 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40"
                    />
                  </div>
                </div>
              </div>

              <DataTable
                columns={columns}
                data={visible}
                getRowId={getEpisodeId}
                loading={episodes.loading}
                enableSelection
                onSelectionChange={setSelectedIds}
                selectionToolbar={(ids) => (
                  <>
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={bulkBusy}
                      onClick={() =>
                        void queueEpisodes(
                          visible.filter((episode) =>
                            ids.has(String(episode.id)),
                          ),
                        )
                      }
                    >
                      <DownloadIcon />
                      Download selected
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={bulkBusy}
                      className="text-destructive"
                      onClick={() => setBulkDeleteOpen(true)}
                    >
                      Delete selected
                    </Button>
                  </>
                )}
                emptyTitle="No episodes"
                emptyDescription="Try a different status filter or add episodes from the source catalog."
              />
            </section>
          </>
        )}
      </div>

      {detail && (
        <SeriesEditDialog
          open={editOpen}
          onOpenChange={setEditOpen}
          series={{
            name: detail.name,
            description: detail.description,
            saveDir: detail.saveDir,
            bannerSource: detail.bannerSource,
          }}
          onPickBanner={handlePickBanner}
          onSave={handleSaveSeries}
          saving={bannerPick.picking}
        />
      )}

      <EpisodeDialog
        open={addOpen || editEpisode !== null}
        onOpenChange={(open) => {
          if (!open) {
            setAddOpen(false)
            setEditEpisode(null)
          }
        }}
        episode={editEpisode}
        onSave={editEpisode ? handleUpdateEpisode : handleCreateEpisode}
        saving={episodeMutations.busy}
      />

      {deleteEpisode && (
        <DeleteConfirmDialog
          open={true}
          onOpenChange={() => setDeleteEpisode(null)}
          episode={deleteEpisode}
          onConfirm={handleDeleteEpisode}
        />
      )}

      <Dialog open={bulkDeleteOpen} onOpenChange={setBulkDeleteOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete {selectedIds.size} episode(s)</DialogTitle>
            <DialogDescription>
              These episodes will be removed from the library. Files already on
              disk are left untouched.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setBulkDeleteOpen(false)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={() => void handleBulkDelete()}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

export default function SeriesPage() {
  return (
    <Suspense fallback={<Skeleton className="m-6 h-64 rounded-xl" />}>
      <SeriesDetail />
    </Suspense>
  )
}