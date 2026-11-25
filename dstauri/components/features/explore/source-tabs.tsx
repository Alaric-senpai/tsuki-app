"use client"

import { cn } from "@/lib/utils"
import type { SourceInfo } from "@/lib/ipc/explore"

/**
 * Per-source switcher for Explore. Mirrors the Stitch "Explore YouTube" and
 * "Explore Donghua" screens as a single tab strip; unavailable sources are
 * omitted rather than shown as dead controls.
 */
export function SourceTabs({
  sources,
  active,
  onSelect,
}: {
  sources: SourceInfo[]
  active: string
  onSelect: (key: string) => void
}) {
  const available = sources.filter((source) => source.available)

  return (
    <div
      role="tablist"
      aria-label="Explore sources"
      className="flex flex-wrap gap-1 rounded-lg border border-border/60 bg-surface-low p-1"
    >
      {available.map((source) => (
        <button
          key={source.key}
          type="button"
          role="tab"
          aria-selected={source.key === active}
          onClick={() => onSelect(source.key)}
          className={cn(
            "rounded-md px-3 py-1.5 text-sm transition-colors",
            source.key === active
              ? "bg-surface-container-high font-medium text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {source.label}
        </button>
      ))}
    </div>
  )
}
