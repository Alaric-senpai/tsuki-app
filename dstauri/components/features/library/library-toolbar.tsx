"use client"

import { SearchIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

export function LibraryToolbar({
  filters,
  filter,
  query,
  onFilterChange,
  onQueryChange,
}: {
  filters: readonly string[]
  filter: string
  query: string
  onFilterChange: (filter: string) => void
  onQueryChange: (query: string) => void
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex flex-wrap items-center gap-2">
        {filters.map((option) => (
          <Button
            key={option}
            variant="ghost"
            onClick={() => onFilterChange(option)}
            className={cn(
              "h-7 rounded-full border px-3 text-xs font-medium transition-colors",
              filter === option
                ? "border-primary/40 bg-primary/10 text-primary"
                : "border-border/60 bg-surface-low text-muted-foreground hover:text-foreground",
            )}
          >
            {option}
          </Button>
        ))}
      </div>
      <div className="relative w-full sm:w-72">
        <SearchIcon className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Search your library"
          aria-label="Search your library"
          className="h-9 bg-surface-low pl-9"
        />
      </div>
    </div>
  )
}
