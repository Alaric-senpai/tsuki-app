import { DownloadIcon } from "lucide-react"

import { SectionTitle } from "@/components/common/section-title"
import { Button } from "@/components/ui/button"
import { EpisodeStatusTable } from "@/components/features/downloads/episode-status-table"
import { NotStartedSection } from "@/components/features/downloads/not-started-section"
import type { useRecentEpisodes } from "@/hooks/use-library"

type Episodes = ReturnType<typeof useRecentEpisodes>

/** The four catalog-backed sections below the live queue. */
export function EpisodeSections({
  notStarted,
  queued,
  failed,
  downloaded,
  bulkBusy,
  onQueueMany,
  onQueueAllNotStarted,
  onRemoveQueued,
  onRetry,
}: {
  notStarted: Episodes
  queued: Episodes
  failed: Episodes
  downloaded: Episodes
  bulkBusy: boolean
  onQueueMany: (ids: number[]) => Promise<void>
  onQueueAllNotStarted: () => void
  onRemoveQueued: (id: number) => void
  onRetry: (id: number) => void
}) {
  return (
    <>
      <section className="space-y-3">
        <SectionTitle
          actions={
            <Button
              variant="outline"
              size="sm"
              disabled={bulkBusy || notStarted.data.length === 0}
              onClick={onQueueAllNotStarted}
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
          onDownload={onQueueMany}
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
          onRemove={onRemoveQueued}
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
          onRetry={onRetry}
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
    </>
  )
}
