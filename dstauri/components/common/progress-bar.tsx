import { cn } from "@/lib/utils"

export function ProgressBar({
  value,
  label,
  className,
}: {
  value: number
  label?: string
  className?: string
}) {
  const percent = Math.max(0, Math.min(100, Math.round(value)))

  return (
    <div className={cn("space-y-1.5", className)}>
      <div
        role="progressbar"
        aria-label={label ?? "Completion"}
        aria-valuenow={percent}
        aria-valuemin={0}
        aria-valuemax={100}
        className="h-1.5 overflow-hidden rounded-full bg-surface-high"
      >
        <div
          className="h-full rounded-full bg-primary transition-[width]"
          style={{ width: `${percent}%` }}
        />
      </div>
      {label ? (
        <p className="telemetry text-[0.5rem] text-muted-foreground">{label}</p>
      ) : null}
    </div>
  )
}
