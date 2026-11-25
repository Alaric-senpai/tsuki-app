import { PauseIcon, PlayIcon } from "lucide-react"

import { ErrorPanel } from "@/components/common/data-state"
import { SectionTitle } from "@/components/common/section-title"
import { Button } from "@/components/ui/button"
import { DownloadQueueItem } from "@/components/features/downloads/download-queue-item"
import type { DownloadQueue } from "@/hooks/use-downloads"

/** The live queue, including its loading, empty and error states. */
export function LiveQueueSection({
  queue,
  activeCount,
  finished,
  onTogglePause,
  onCancelActive,
  onClearFinished,
}: {
  queue: DownloadQueue
  activeCount: number
  finished: number
  onTogglePause: () => void
  onCancelActive: () => void
  onClearFinished: () => void
}) {
  const rows = queue.rows
  return (
    <section className="space-y-3">
      <SectionTitle
        actions={
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={onTogglePause}>
              {queue.paused ? (
                <PlayIcon className="size-3.5" />
              ) : (
                <PauseIcon className="size-3.5" />
              )}
              {queue.paused ? "Resume queue" : "Pause queue"}
            </Button>
            {activeCount > 0 ? (
              <Button variant="outline" size="sm" onClick={onCancelActive}>
                Cancel active
              </Button>
            ) : null}
            {finished > 0 ? (
              <Button variant="ghost" size="sm" onClick={onClearFinished}>
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
              onPause={(episodeId) => {
                void queue.pause(episodeId)
              }}
              onResume={(episodeId) => {
                void queue.resume(episodeId)
              }}
              onMove={(episodeId, direction) => {
                void queue.move(episodeId, direction)
              }}
            />
          ))}
        </div>
      )}
    </section>
  )
}
