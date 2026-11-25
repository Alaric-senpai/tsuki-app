import { cn } from "@/lib/utils"

export interface StatItem {
  label: string
  value: string | number
  hint?: string
}

export function StatGrid({
  items,
  className,
}: {
  items: StatItem[]
  className?: string
}) {
  return (
    <dl className={cn("grid grid-cols-2 gap-3 lg:grid-cols-4", className)}>
      {items.map((item) => (
        <div
          key={item.label}
          className="rounded-xl border border-border/60 bg-surface-low p-3"
        >
          <dt className="telemetry text-[0.55rem] text-muted-foreground">
            {item.label}
          </dt>
          <dd className="mt-1 font-mono text-lg font-semibold tracking-tight text-foreground">
            {item.value}
          </dd>
          {item.hint ? (
            <p className="mt-0.5 text-xs text-muted-foreground">{item.hint}</p>
          ) : null}
        </div>
      ))}
    </dl>
  )
}
