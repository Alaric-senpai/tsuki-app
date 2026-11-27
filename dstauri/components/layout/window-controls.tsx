"use client"

import { useEffect, useState } from "react"
import { CopyIcon, MinusIcon, SquareIcon, XIcon } from "lucide-react"
import { getCurrentWindow } from "@tauri-apps/api/window"

import { isTauri } from "@/lib/tauri"
import { Separator } from "@/components/ui/separator"

/**
 * Native decorations are not available on every Linux compositor. These
 * controls keep the window usable everywhere: hiding the window leaves the
 * tray icon (installed in Rust) as the way back into the app.
 */
export function WindowControls() {
  const [available, setAvailable] = useState(false)
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    if (!isTauri()) return
    const window = getCurrentWindow()
    // Defer the state update so React does not render synchronously inside
    // the effect body (SSR renders the same null output on both sides).
    queueMicrotask(() => setAvailable(true))
    let active = true
    void window.isMaximized().then((value) => {
      if (active) setMaximized(value)
    })
    const unlisten = window.onResized(() => {
      void window.isMaximized().then((value) => {
        if (active) setMaximized(value)
      })
    })
    return () => {
      active = false
      void unlisten.then((off) => off())
    }
  }, [])

  if (!available) return null

  const window = getCurrentWindow()
  const action = (className: string, label: string, run: () => Promise<void>, icon: React.ReactNode) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={() => void run()}
      className={className}
    >
      {icon}
    </button>
  )

  const base =
    "flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-high hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"

  return (
    <div className="flex items-center gap-1" role="group" aria-label="Window controls">
      <Separator orientation="vertical" className="mx-1 h-4" />
      {action(
        `${base} hover:text-primary`,
        "Minimize to tray",
        () => window.hide(),
        <MinusIcon className="size-3.5" />,
      )}
      {action(
        base,
        maximized ? "Restore window" : "Maximize window",
        async () => {
          await window.toggleMaximize()
          setMaximized(await window.isMaximized())
        },
        maximized ? <CopyIcon className="size-3.5" /> : <SquareIcon className="size-3.5" />,
      )}
      {action(
        `${base} hover:text-destructive`,
        "Close window",
        () => window.close(),
        <XIcon className="size-3.5" />,
      )}
    </div>
  )
}
