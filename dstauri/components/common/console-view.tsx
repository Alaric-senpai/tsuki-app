"use client"

import { useState } from "react"
import { CheckIcon, CopyIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

function lineTone(line: string): string {
  const upper = line.toUpperCase()
  if (upper.includes("ERROR")) {
    return "text-rose"
  }
  if (upper.includes("WARNING")) {
    return "text-amber"
  }
  if (upper.includes("DESTINATION") || upper.includes("SAVED")) {
    return "text-emerald"
  }
  return "text-muted-foreground"
}

/**
 * Colorized, copyable console output. Used by the download queue for yt-dlp
 * logs so a failure can be pasted into a bug report.
 */
export function ConsoleView({
  lines,
  className,
  emptyLabel = "No console output yet.",
}: {
  lines: string[]
  className?: string
  emptyLabel?: string
}) {
  const [copied, setCopied] = useState(false)

  async function copyAll() {
    try {
      await navigator.clipboard.writeText(lines.join("\n"))
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      // Clipboard access can be denied by the webview; the text stays visible.
    }
  }

  return (
    <div className={cn("space-y-2", className)}>
      <div className="flex items-center justify-between">
        <span className="telemetry text-[0.5rem] uppercase text-muted-foreground">
          Console
        </span>
        <Button
          variant="ghost"
          size="xs"
          onClick={() => void copyAll()}
          disabled={lines.length === 0}
        >
          {copied ? <CheckIcon /> : <CopyIcon />}
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <pre className="max-h-44 overflow-auto rounded-lg border border-border/60 bg-background/60 p-3 font-mono text-[0.65rem] leading-relaxed">
        {lines.length === 0 ? (
          <span className="text-muted-foreground">{emptyLabel}</span>
        ) : (
          lines.map((line, index) => (
            <span key={`${index}-${line.slice(0, 24)}`} className={cn("block", lineTone(line))}>
              {line}
            </span>
          ))
        )}
      </pre>
    </div>
  )
}
