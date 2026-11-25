import { SectionTitle } from "@/components/common/section-title"
import { StatGrid } from "@/components/common/stat-grid"
import type { useArchiveSync, useLibraryStats } from "@/hooks/use-library"
import { formatBytes, formatEta, formatSpeed } from "@/lib/utils"

type Stats = ReturnType<typeof useLibraryStats>
type Archive = ReturnType<typeof useArchiveSync>

export interface SessionSummary {
  speedBps: number
  downloadedBytes: number
  etaSec: number
}

/** Library totals, live telemetry and archive-sync errors. */
export function SessionOverview({
  stats,
  archive,
  activeCount,
  session,
}: {
  stats: Stats
  archive: Archive
  activeCount: number
  session: SessionSummary
}) {
  return (
    <>
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
              value: activeCount,
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
    </>
  )
}
