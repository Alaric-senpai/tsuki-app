"use client"

import { useCallback, useMemo, useState, type ReactNode } from "react"
import { ListIcon } from "lucide-react"
import type { ColumnDef } from "@tanstack/table-core"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { EmptyState } from "@/components/common/empty-state"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table"
import { Pagination } from "@/components/common/pagination"

// Minimal data table that understands TanStack-style column defs
// (id, header, accessorFn, cell). Type safety for column accessors
// is the caller's responsibility.

type DataTableProps<T> = {
  /** Erased TanStack column defs; the caller owns row typing. */
  columns: readonly object[]
  data: readonly T[]
  getRowId?: (row: T, index: number) => string
  initialPageSize?: number
  pageSizes?: number[]
  onPageChange?: (page: number) => void
  onPageSizeChange?: (size: string) => void
  enableSelection?: boolean
  onSelectionChange?: (selectedIds: Set<string>) => void
  /** Bulk actions rendered in the selection banner. */
  selectionToolbar?: (selectedIds: Set<string>, clear: () => void) => ReactNode
  emptyTitle?: string
  emptyDescription?: string
  loading?: boolean
}

type RowModel = { id: string; index: number; original: unknown }

/** The subset of a TanStack column this table actually reads. */
type RuntimeColumn = {
  id?: string
  header?: ReactNode | ((context: { column: RuntimeColumn }) => ReactNode)
  accessorFn?: (row: unknown, index: number) => unknown
  cell?: (context: {
    column: RuntimeColumn
    row: { id: string; index: number; original: unknown }
    getValue: () => unknown
    renderValue: () => unknown
  }) => ReactNode
}

function renderHeader(column: RuntimeColumn) {
  if (typeof column.header === "function") {
    return column.header({ column })
  }
  return column.header ?? column.id
}

function renderCell(column: RuntimeColumn, row: RowModel) {
  const value = column.accessorFn
    ? column.accessorFn(row.original, row.index)
    : undefined

  if (typeof column.cell === "function") {
    // Minimal subset of TanStack's cell context
    return column.cell({
      column,
      row: { id: row.id, index: row.index, original: row.original },
      getValue: () => value,
      renderValue: () => value ?? null,
    })
  }

  return value === null || value === undefined || value === "" ? (
    <span className="text-muted-foreground">—</span>
  ) : (
    String(value)
  )
}

export function DataTable<T>({
  columns,
  data,
  getRowId,
  initialPageSize = 5,
  pageSizes = [5, 10, 25, 50, 100],
  onPageChange,
  onPageSizeChange,
  enableSelection = false,
  onSelectionChange,
  selectionToolbar,
  emptyTitle = "No items found",
  emptyDescription = "There are no items to display.",
  loading = false,
}: DataTableProps<T>) {
  const [pageSize, setPageSize] = useState<number>(initialPageSize)
  const [pageIndex, setPageIndex] = useState(0)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())

  const cols = columns as readonly RuntimeColumn[]

  const rows = useMemo<RowModel[]>(
    () =>
      data.map((original, index) => ({
        id: getRowId
          ? getRowId(original, index)
          : String((original as { id?: unknown } | null)?.id ?? index),
        index,
        original,
      })),
    [data, getRowId],
  )

  const pageCount = Math.max(1, Math.ceil(rows.length / pageSize))
  const currentPage = Math.min(pageIndex, pageCount - 1) // clamp when data shrinks
  const pageRows = useMemo(
    () => rows.slice(currentPage * pageSize, (currentPage + 1) * pageSize),
    [rows, currentPage, pageSize],
  )

  const handlePageChange = useCallback(
    (page: number) => {
      setPageIndex(page - 1)
      onPageChange?.(page)
    },
    [onPageChange],
  )

  const handlePageSizeChange = useCallback(
    (size: string) => {
      setPageSize(Number(size))
      setPageIndex(0)
      onPageSizeChange?.(size)
    },
    [onPageSizeChange],
  )

  // Rows that vanished (e.g. after a delete) drop out of the selection at
  // render time rather than through an effect, so committing a selection never
  // triggers a second render pass.
  const rowIds = useMemo(() => rows.map((row) => row.id), [rows])

  const commitSelection = useCallback(
    (next: Set<string>) => {
      const present = new Set(rowIds)
      const kept =
        next.size === 0
          ? next
          : new Set([...next].filter((id) => present.has(id)))
      setSelectedIds(kept)
      onSelectionChange?.(kept)
    },
    [onSelectionChange, rowIds],
  )

  const selection = useMemo(() => {
    if (selectedIds.size === 0) {
      return selectedIds
    }
    const present = new Set(rowIds)
    let pruned = false
    const next = new Set<string>()
    for (const id of selectedIds) {
      if (present.has(id)) {
        next.add(id)
      } else {
        pruned = true
      }
    }
    return pruned ? next : selectedIds
  }, [selectedIds, rowIds])

  const toggleRow = useCallback(
    (id: string) => {
      const next = new Set(selection)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      commitSelection(next)
    },
    [selection, commitSelection],
  )

  const togglePage = useCallback(
    (checked: boolean) => {
      const next = new Set(selection)
      for (const row of pageRows) {
        if (checked) next.add(row.id)
        else next.delete(row.id)
      }
      commitSelection(next)
    },
    [selection, pageRows, commitSelection],
  )

  const pageAllSelected =
    pageRows.length > 0 && pageRows.every((r) => selection.has(r.id))
  const pageSomeSelected =
    !pageAllSelected && pageRows.some((r) => selection.has(r.id))

  if (loading) {
    return (
      <div className="space-y-2" aria-busy="true" aria-label="Loading table rows">
        {Array.from({ length: 6 }, (_, index) => (
          <div
            key={index}
            className="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-low px-4 py-3"
          >
            <Skeleton className="size-8 rounded-lg" />
            <div className="flex-1 space-y-2">
              <Skeleton className="h-3 w-2/3" />
              <Skeleton className="h-2 w-1/3" />
              <Skeleton className="h-[22px] w-20 rounded-full" />
            </div>
          </div>
        ))}
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <EmptyState icon={ListIcon} title={emptyTitle} description={emptyDescription} />
    )
  }

  return (
    <div className="space-y-4">
      {enableSelection && selection.size > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/60 bg-surface-low p-3">
          <span className="text-sm text-muted-foreground">
            {selection.size} row(s) selected
          </span>
          <div className="flex flex-wrap items-center gap-2">
            {selectionToolbar?.(selection, () => commitSelection(new Set()))}
            <Button
              variant="outline"
              size="sm"
              onClick={() => commitSelection(new Set())}
            >
              Deselect all
            </Button>
          </div>
        </div>
      )}

      <div className="overflow-hidden rounded-xl border border-border/60 bg-surface-low">
        <Table className="min-w-full">
          <TableHeader>
            <TableRow>
              {enableSelection && (
                <TableHead className="w-10">
                  <Checkbox
                    aria-label="Select all rows on this page"
                    checked={pageAllSelected}
                    indeterminate={pageSomeSelected}
                    onCheckedChange={(checked) => togglePage(Boolean(checked))}
                  />
                </TableHead>
              )}
              {cols.map((column) => (
                <TableHead key={column.id}>
                  <span className="text-xs font-medium text-foreground">
                    {renderHeader(column)}
                  </span>
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {pageRows.map((row) => {
              const selected = selection.has(row.id)
              return (
                <TableRow key={row.id} data-selected={selected ? "true" : "false"}>
                  {enableSelection && (
                    <TableCell className="w-10">
                      <Checkbox
                        aria-label="Select row"
                        checked={selected}
                        onCheckedChange={() => toggleRow(row.id)}
                      />
                    </TableCell>
                  )}
                  {cols.map((column) => (
                    <TableCell key={column.id}>{renderCell(column, row)}</TableCell>
                  ))}
                </TableRow>
              )
            })}
          </TableBody>
        </Table>

        <div className="flex items-center justify-between border-t border-border/60 px-4 py-3">
          <Pagination
            pageCount={pageCount}
            page={currentPage + 1}
            pageSize={String(pageSize)}
            sizes={pageSizes.map((s) => ({ value: String(s), label: String(s) }))}
            onPageChange={handlePageChange}
            onPageSizeChange={handlePageSizeChange}
          />
        </div>
      </div>
    </div>
  )
}

export type { ColumnDef }