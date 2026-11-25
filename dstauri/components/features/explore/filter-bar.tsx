"use client"

import { useCallback, useRef } from "react"
import { RefreshCwIcon, SearchIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import type { CatalogFilters } from "@/lib/ipc/explore"
import { cn } from "@/lib/utils"

export interface ExploreFiltersState {
  genre: string
  order: string
  status: string
  kind: string
  search: string
}

export function defaultFilters(): ExploreFiltersState {
  return { genre: "", order: "", status: "", kind: "", search: "" }
}

function SelectRow({
  label,
  value,
  options,
  onChange,
  className,
}: {
  label: string
  value: string
  options: ReadonlyArray<{ value: string; label: string }>
  onChange: (v: string) => void
  className?: string
}) {
  return (
    <Select value={value} onValueChange={(next) => { if (next) onChange(next) }}>
      <SelectTrigger size="sm" className={cn("w-36", className)} aria-label={label}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

export function FilterBar({
  filters,
  catalogFilters,
  onFiltersChange,
  onSearch,
  onRefresh,
  refreshing = false,
  searching = false,
}: {
  filters: ExploreFiltersState
  catalogFilters: CatalogFilters | null
  onFiltersChange: (f: ExploreFiltersState) => void
  onSearch: (search: string) => void
  onRefresh: () => void
  refreshing?: boolean
  searching?: boolean
}) {
  const searchRef = useRef<HTMLInputElement>(null)

  const set = useCallback(
    (patch: Partial<ExploreFiltersState>) =>
      onFiltersChange({ ...filters, ...patch }),
    [filters, onFiltersChange],
  )

  // Genre chips: "All Genres" chip + one per remote genre
  const allGenres = catalogFilters?.genres ?? []
  const hasGenres = allGenres.length > 0

  return (
    <div className="space-y-3">
      {/* Genre chips */}
      <div className="flex items-center gap-2 overflow-x-auto pb-1 scrollbar-hide">
        <button
          type="button"
          onClick={() => set({ genre: "" })}
          className={cn(
            "shrink-0 rounded-lg px-3.5 py-1.5 text-xs font-medium transition-colors",
            filters.genre === ""
              ? "bg-secondary text-on-secondary font-semibold"
              : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high",
          )}
        >
          All Genres
        </button>
        {!hasGenres && !refreshing && (
          <span className="shrink-0 text-xs text-muted-foreground">No genres available</span>
        )}
        {allGenres.map((g) => (
          <button
            key={g.slug}
            type="button"
            onClick={() => set({ genre: g.slug })}
            className={cn(
              "shrink-0 rounded-lg px-3.5 py-1.5 text-xs font-medium transition-colors capitalize",
              filters.genre === g.slug
                ? "bg-secondary text-on-secondary font-semibold"
                : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high",
            )}
          >
            {g.label}
          </button>
        ))}
      </div>

      {/* Order / Status / Kind selects + search + refresh */}
      <div className="flex flex-wrap items-center gap-3">
        <SelectRow
          label="Order by"
          value={filters.order}
          options={catalogFilters?.orders.map((o) => ({ value: o.value, label: o.label })) ?? [
            { value: "", label: "Loading…" },
          ]}
          onChange={(v) => set({ order: v })}
        />
        <SelectRow
          label="Status"
          value={filters.status}
          options={catalogFilters?.statuses.map((s) => ({ value: s.value, label: s.label })) ?? [
            { value: "", label: "Loading…" },
          ]}
          onChange={(v) => set({ status: v })}
        />
        <SelectRow
          label="Kind"
          value={filters.kind}
          options={catalogFilters?.kinds.map((k) => ({ value: k.value, label: k.label })) ?? [
            { value: "", label: "Loading…" },
          ]}
          onChange={(v) => set({ kind: v })}
        />

        {/* Search */}
        <form
          className="relative flex-1 min-w-32"
          onSubmit={(e) => {
            e.preventDefault()
            onSearch(filters.search)
          }}
        >
          <SearchIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
          <Input
            ref={searchRef}
            value={filters.search}
            onChange={(e) => set({ search: e.target.value })}
            placeholder="Search series…"
            className="h-8 pl-8 pr-16 text-xs"
            aria-label="Search series"
          />
          {filters.search ? (
            <button
              type="submit"
              className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-primary hover:underline"
            >
              {searching ? "…" : "Go"}
            </button>
          ) : null}
        </form>

        {/* Refresh */}
        <Button
          variant="outline"
          size="sm"
          onClick={onRefresh}
          disabled={refreshing}
          className="h-8 gap-1.5 text-xs"
        >
          <RefreshCwIcon className={cn("size-3", refreshing && "animate-spin")} />
          {refreshing ? "Syncing" : "Sync"}
        </Button>
      </div>
    </div>
  )
}
