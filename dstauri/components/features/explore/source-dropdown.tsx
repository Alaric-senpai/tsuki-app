"use client"

import { ChevronDownIcon } from "lucide-react"

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import type { SourceInfo } from "@/lib/ipc/explore"

export function SourceDropdown({
  sources,
  active,
  onSelect,
}: {
  sources: SourceInfo[]
  active: string
  onSelect: (key: string) => void
}) {
  const current = sources.find((s) => s.key === active)

  return (
    <DropdownMenu>
      <DropdownMenuTrigger 
          type="button"
          className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-surface-low px-3 py-1.5 text-sm text-foreground hover:bg-surface-container-high transition-colors"
      >
          <span className="font-medium">{current?.label ?? active}</span>
          <ChevronDownIcon className="size-3.5 text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={6} className="min-w-48">
        {sources.map((src) => (
          <DropdownMenuItem
            key={src.key}
            disabled={!src.available}
            onSelect={() => onSelect(src.key)}
            className="flex flex-col items-start gap-0.5 py-2"
          >
            <span className={!src.available ? "text-muted-foreground" : ""}>
              {src.label}
            </span>
            {!src.available && src.note && (
              <span className="text-[0.65rem] text-muted-foreground">
                {src.note}
              </span>
            )}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
