import type { ReactNode } from "react"
import { DownloadIcon } from "lucide-react"

/** Consistent heading for the stacked sections on data-heavy pages. */
export function SectionTitle({
  children,
  actions,
}: {
  children: string
  actions?: ReactNode
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <div className="flex items-center gap-2">
        <DownloadIcon className="size-4 text-primary" />
        <h2 className="font-heading text-lg font-semibold tracking-tight">
          {children}
        </h2>
      </div>
      {actions}
    </div>
  )
}
