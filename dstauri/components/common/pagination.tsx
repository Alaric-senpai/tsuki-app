"use client"

import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

export interface PageSizeOption {
  value: string
  label: string
}

export function Pagination({
  page,
  pageCount,
  pageSize,
  sizes,
  onPageChange,
  onPageSizeChange,
}: {
  page: number
  pageCount: number
  pageSize: string
  sizes: readonly PageSizeOption[]
  onPageChange: (page: number) => void
  onPageSizeChange: (size: string) => void
}) {
  return (
    <div className="flex items-center gap-2">
      <Select value={pageSize} onValueChange={(value) => value && onPageSizeChange(value)}>
        <SelectTrigger size="sm" aria-label="Items per page">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {sizes.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button
        variant="outline"
        size="icon-sm"
        onClick={() => onPageChange(page - 1)}
        disabled={page <= 1}
        aria-label="Previous page"
      >
        <ChevronLeftIcon />
      </Button>
      <span className="telemetry text-[0.55rem] text-muted-foreground">
        {page} / {pageCount}
      </span>
      <Button
        variant="outline"
        size="icon-sm"
        onClick={() => onPageChange(page + 1)}
        disabled={page >= pageCount}
        aria-label="Next page"
      >
        <ChevronRightIcon />
      </Button>
    </div>
  )
}
