"use client"

import { SearchIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { cn } from "@/lib/utils"

import {
  ALL_SOURCES,
  LIBRARY_SORT_LABELS,
  LIBRARY_SORTS,
  type LibrarySort,
} from "./library-view"

export function LibraryToolbar({
  filters,
  filter,
  query,
  onFilterChange,
  onQueryChange,
  sort,
  onSortChange,
  sources,
  source,
  onSourceChange,
}: {
  filters: readonly string[]
  filter: string
  query: string
  onFilterChange: (filter: string) => void
  onQueryChange: (query: string) => void
  sort: LibrarySort
  onSortChange: (sort: string) => void
  sources: readonly string[]
  source: string
  onSourceChange: (source: string) => void
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
        <Select value={sort} onValueChange={(value) => value && onSortChange(value)}>
          <SelectTrigger size="sm" aria-label="Sort library" className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {LIBRARY_SORTS.map((option) => (
              <SelectItem key={option} value={option}>
                {LIBRARY_SORT_LABELS[option]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {sources.length > 0 ? (
          <Select value={source} onValueChange={(value) => value && onSourceChange(value)}>
            <SelectTrigger size="sm" aria-label="Filter by source" className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL_SOURCES}>{ALL_SOURCES}</SelectItem>
              {sources.map((option) => (
                <SelectItem key={option} value={option}>
                  {option}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : null}
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
