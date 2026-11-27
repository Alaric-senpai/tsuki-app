"use client"

import { useEffect, useRef } from "react"
import { WifiOffIcon } from "lucide-react"
import { toast } from "sonner"

import { useNetworkStatus } from "@/hooks/use-network-status"
import { cn } from "@/lib/utils"

/**
 * Global, non-blocking connectivity notice. Mounted once in the root layout.
 * It never intercepts clicks and announces changes politely to screen readers.
 */
export function NetworkBanner() {
  const { online, lastChanged } = useNetworkStatus()
  const previous = useRef<boolean | null>(null)

  useEffect(() => {
    const before = previous.current
    previous.current = online
    if (before === null || before === online) return
    if (online) {
      toast.success("Back online", {
        description: "Queued downloads will resume automatically.",
      })
    } else {
      toast.warning("Connection lost", {
        description: "Downloads and source lookups will resume when you reconnect.",
      })
    }
  }, [online, lastChanged])

  if (online) return null

  return (
    <div
      role="status"
      aria-live="polite"
      aria-atomic="true"
      className={cn(
        "pointer-events-none fixed inset-x-0 top-3 z-50 mx-auto flex w-fit max-w-[min(90vw,32rem)] items-center gap-2.5",
        "rounded-lg border border-amber/40 bg-surface-low/95 px-3.5 py-2 text-body-sm text-foreground shadow-l2 backdrop-blur",
      )}
    >
      <WifiOffIcon aria-hidden="true" className="size-4 shrink-0 text-amber" />
      <span>
        No internet connection. Downloads and source lookups will resume when you
        reconnect.
      </span>
    </div>
  )
}
