import type { LucideIcon } from "lucide-react"
import type { ReactNode } from "react"

export function EmptyState({
  title,
  description,
  action,
}: {
  icon: LucideIcon
  title: string
  description?: string
  action?: ReactNode
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border/70 bg-surface-low/40 px-6 py-16 text-center">
      <img
        src="/assets/empty-state-mascot.jpeg"
        alt=""
        className="size-24 rounded-full opacity-50"
        loading="lazy"
        decoding="async"
        aria-hidden
      />
      <div className="space-y-1">
        <p className="font-heading text-base font-semibold text-foreground">{title}</p>
        {description ? (
          <p className="mx-auto max-w-sm text-sm text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {action}
    </div>
  )
}
