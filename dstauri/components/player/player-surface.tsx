"use client"

import { useState } from "react"
import { LoaderCircleIcon } from "lucide-react"
import ReactPlayer from "react-player"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

/**
 * Thin wrapper around react-player. Both the online (/stream) and local
 * (/watch) routes use this so playback behaviour stays identical; only the
 * source resolution differs.
 *
 * react-player falls back to a plain <video> for URLs it does not recognise,
 * which renders as a silent black box. We surface a clear loading/error state
 * so a failed stream never looks like a broken player.
 */
export function PlayerSurface({
  src,
  className,
  onError,
  onReady,
}: {
  src: string
  className?: string
  onError?: (message: string) => void
  onReady?: () => void
}) {
  const [ready, setReady] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  // react-player's canPlay() is only a hint: its html fallback (a plain
  // <video>) can still handle URLs it does not recognise, e.g. Dailymotion.
  // Never block playback on that check — only surface real media errors.
  const message = error ?? ""
  const showError = error !== null

  const describe = (event: unknown): string => {
    if (event instanceof Error) return event.message
    if (typeof event === "string") return event
    return "Playback failed. This stream may need cookies or headers the player cannot supply."
  }

  const handleError = (event: unknown) => {
    const text = describe(event)
    setReady(false)
    setError(text)
    onError?.(text)
  }

  const handleReady = () => {
    setReady(true)
    setError(null)
    onReady?.()
  }

  const copyDetails = async () => {
    try {
      await navigator.clipboard.writeText(`${src}\n${message}`)
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  return (
    <div
      className={cn(
        "relative aspect-video w-full overflow-hidden rounded-xl bg-black",
        className,
      )}
    >
      {showError ? (
        <div
          role="alert"
          className="flex h-full w-full flex-col items-center justify-center gap-3 p-6 text-center"
        >
          <p className="text-sm font-medium text-foreground">Playback unavailable</p>
          <p className="max-w-md break-words text-xs text-muted-foreground">{message}</p>
          <p className="max-w-md break-all font-mono text-[10px] text-muted-foreground/70">
            {src}
          </p>
          <div className="flex items-center gap-2">
            <Button size="xs" variant="outline" onClick={() => void copyDetails()}>
              {copied ? "Copied" : "Copy details"}
            </Button>
            {/^https?:\/\//i.test(src) ? (
              <Button
                size="xs"
                variant="ghost"
                nativeButton={false}
                render={<a href={src} target="_blank" rel="noreferrer" />}
              >
                Open link
              </Button>
            ) : null}
          </div>
        </div>
      ) : (
        <>
          {!ready ? (
            <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center">
              <div className="flex items-center gap-2 rounded-full bg-black/60 px-3 py-1.5 text-xs text-white">
                <LoaderCircleIcon className="size-3.5 animate-spin" />
                Loading player…
              </div>
            </div>
          ) : null}
          <ReactPlayer
            src={src}
            controls
            playsInline
            width="100%"
            height="100%"
            fallback={
              <div className="flex h-full w-full items-center justify-center text-xs text-white/70">
                Loading player…
              </div>
            }
            onReady={handleReady}
            onError={handleError}
          />
        </>
      )}
    </div>
  )
}
