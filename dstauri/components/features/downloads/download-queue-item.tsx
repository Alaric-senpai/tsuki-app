"use client"

import { DownloadIcon, XIcon } from "lucide-react"

import { ConsoleView } from "@/components/common/console-view"
import { ErrorPanel } from "@/components/common/data-state"
import { ProgressBar } from "@/components/common/progress-bar"
import { StatusChip, type StatusTone } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import type { DownloadSnapshot } from "@/lib/ipc/downloads"
import { cn, formatBytes, formatEta, formatSpeed } from "@/lib/utils"

const ACTIVE_STATES = new Set(["queued", "resolving", "downloading", "retrying"])

const PHASE_META: Record<string, { tone: StatusTone; label: string }> = {
  queued: { tone: "buffering", label: "QUEUED" },
  resolving: { tone: "buffering", label: "RESOLVING" },
  downloading: { tone: "active", label: "DOWNLOADING" },
  retrying: { tone: "stalled", label: "RETRYING" },
  done: { tone: "active", label: "SAVED" },
  failed: { tone: "error", label: "FAILED" },
  cancelled: { tone: "idle", label: "CANCELLED" },
}

function phaseMeta(snapshot: DownloadSnapshot) {
  const meta = PHASE_META[snapshot.state] ?? { tone: "stalled" as StatusTone, label: snapshot.state.toUpperCase() }
  if (snapshot.state === "retrying") {
    return {
      ...meta,
      label: `RETRYING ${snapshot.attempt}/${snapshot.maxRetries}`,
    }
  }
  return meta
}

export function DownloadQueueItem({
  snapshot,
  consoleLines = [],
  onCancel,
  className,
}: {
  snapshot: DownloadSnapshot
  consoleLines?: string[]
  onCancel?: (episodeId: number) => void
  className?: string
}) {
  const meta = phaseMeta(snapshot)
  const active = ACTIVE_STATES.has(snapshot.state)
  const percent =
    snapshot.state === "done"
      ? 100
      : snapshot.totalBytes > 0
        ? (snapshot.downloadedBytes / snapshot.totalBytes) * 100
        : 0

  const telemetry = [
    snapshot.totalBytes > 0
      ? `${formatBytes(snapshot.downloadedBytes)} / ${formatBytes(snapshot.totalBytes)}`
      : formatBytes(snapshot.downloadedBytes),
    formatSpeed(snapshot.speedBps),
    `ETA ${formatEta(snapshot.etaSec)}`,
    snapshot.currentFormat,
  ]
    .filter(Boolean)
    .join(" · ")

  return (
    <article
      className={cn(
        "rounded-lg border border-border/60 bg-surface-low px-4 py-3",
        className,
      )}
      aria-live="polite"
    >
      <div className="flex items-center gap-3">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-surface-high ring-1 ring-border/60">
          <DownloadIcon className="size-3.5 text-primary" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-foreground">
            {snapshot.title || `Episode #${snapshot.episodeId}`}
          </p>
          <p className="telemetry truncate text-[0.55rem] text-muted-foreground">
            {telemetry}
          </p>
        </div>
        <StatusChip tone={meta.tone} label={meta.label} />
        {active && onCancel ? (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Cancel download of ${snapshot.title}`}
            onClick={() => onCancel(snapshot.episodeId)}
          >
            <XIcon />
          </Button>
        ) : null}
      </div>

      {active || snapshot.state === "done" ? (
        <ProgressBar
          value={percent}
          label={`${Math.round(percent)}%`}
          className="mt-3"
        />
      ) : null}

      {snapshot.filePath ? (
        <p className="mt-2 truncate font-mono text-[0.6rem] text-muted-foreground">
          {snapshot.filePath}
        </p>
      ) : null}

      {snapshot.state === "failed" && snapshot.error ? (
        <div className="mt-3">
          <ErrorPanel
            title="Download failed"
            error={{
              code: "download",
              message: snapshot.error,
              detail: consoleLines.join("\n"),
            }}
          />
        </div>
      ) : null}

      {consoleLines.length > 0 && active ? (
        <details className="mt-3">
          <summary className="cursor-pointer text-xs text-muted-foreground">
            Show console ({consoleLines.length})
          </summary>
          <ConsoleView lines={consoleLines} className="mt-2" />
        </details>
      ) : null}
    </article>
  )
}
