"use client"

import { useCallback, useEffect, useState } from "react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { scanDiskUsage, updateYtDlp, type DiskUsage } from "@/lib/ipc/downloads"
import { cn, formatBytes } from "@/lib/utils"

/** Disk usage of the download folders plus the yt-dlp updater. */
export function DiskUsageCard({ className }: { className?: string }) {
  const [usage, setUsage] = useState<DiskUsage | null>(null)

  useEffect(() => {
    let active = true
    void scanDiskUsage()
      .then((result) => {
        if (active) {
          setUsage(result)
        }
      })
      .catch(() => {
        // A missing folder is not worth breaking the page over.
      })
    return () => {
      active = false
    }
  }, [])

  const update = useCallback(() => {
    toast.promise(updateYtDlp(), {
      loading: "Downloading the latest yt-dlp…",
      success: (version: string) => `yt-dlp ${version} installed`,
      error: "Could not update yt-dlp — see the console for details",
    })
  }, [])

  return (
    <section
      className={cn(
        "rounded-lg border border-border/60 bg-surface-low px-4 py-3",
        className,
      )}
      aria-live="polite"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="text-sm font-medium text-foreground">Disk usage</p>
          <p className="telemetry text-[0.6rem] text-muted-foreground">
            {usage
              ? `${formatBytes(usage.totalBytes)} across ${usage.totalFiles} file${usage.totalFiles === 1 ? "" : "s"}`
              : "Scanning download folders…"}
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={update}>
          Update yt-dlp
        </Button>
      </div>
      {usage && usage.series.length > 0 ? (
        <ul className="mt-3 space-y-1">
          {usage.series.map((row) => (
            <li
              key={row.seriesId}
              className="flex items-center justify-between gap-3 text-xs text-muted-foreground"
            >
              <span className="truncate">{row.name}</span>
              <span className="shrink-0 tabular-nums">
                {formatBytes(row.bytes)} · {row.files} file{row.files === 1 ? "" : "s"}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  )
}
