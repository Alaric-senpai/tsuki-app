"use client"

import Image from "next/image"
import { useState } from "react"
import { AlertTriangleIcon, CopyIcon, RefreshCwIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import type { AppIpcError } from "@/lib/ipc/client"
import { cn } from "@/lib/utils"

export function LoadingGrid({
  count = 6,
  className,
}: {
  count?: number
  className?: string
}) {
  return (
    <div
      aria-label="Loading data"
      aria-busy="true"
      className={cn(
        "flex flex-col items-center justify-center gap-4",
        className,
      )}
    >
      <Image
        src="/assets/loading.jpeg"
        alt=""
        width={96}
        height={96}
        unoptimized
        className="size-24 opacity-40 animate-pulse"
        loading="lazy"
        aria-hidden
      />
      <div
        className={cn(
          "grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4",
          className,
        )}
      >
        {Array.from({ length: count }, (_, index) => (
          <div
            key={index}
            className="overflow-hidden rounded-lg border border-border/50 bg-surface-low"
          >
            <Skeleton className="aspect-[2/3] w-full rounded-none" />
            <div className="space-y-2 p-3">
              <Skeleton className="h-3 w-3/4" />
              <Skeleton className="h-2 w-1/2" />
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

export function ErrorPanel({
  error,
  onRetry,
  title = "Tsuki could not load this view",
}: {
  error: AppIpcError
  onRetry?: () => void
  title?: string
}) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle")

  const copyDetails = async () => {
    const detail = [error.message, error.detail].filter(Boolean).join("\n\n")
    try {
      await navigator.clipboard.writeText(detail)
      setCopyState("copied")
    } catch {
      setCopyState("failed")
    }
  }

  return (
    <section
      role="alert"
      aria-live="polite"
      className="space-y-3 rounded-xl border border-destructive/40 bg-destructive/5 p-5"
    >
      <div className="flex items-start gap-3">
        <AlertTriangleIcon className="mt-0.5 size-4 shrink-0 text-destructive" />
        <div className="min-w-0 space-y-1">
          <h2 className="font-heading text-sm font-semibold text-foreground">{title}</h2>
          <p className="text-sm text-muted-foreground">{error.message}</p>
          <p className="telemetry text-[0.55rem] text-muted-foreground">
            Code: {error.code}
          </p>
        </div>
      </div>
      {error.detail ? (
        <pre className="max-h-40 overflow-auto rounded-lg bg-surface-lowest p-3 font-mono text-xs leading-relaxed text-muted-foreground">
          {error.detail}
        </pre>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {error.detail ? (
          <Button variant="outline" size="sm" onClick={copyDetails}>
            <CopyIcon />
            {copyState === "copied"
              ? "Copied"
              : copyState === "failed"
                ? "Copy failed"
                : "Copy details"}
          </Button>
        ) : null}
        {onRetry ? (
          <Button variant="ghost" size="sm" onClick={onRetry}>
            <RefreshCwIcon />
            Retry
          </Button>
        ) : null}
      </div>
    </section>
  )
}
