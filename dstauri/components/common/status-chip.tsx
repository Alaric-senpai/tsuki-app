import { cn } from "@/lib/utils"

const TONES = {
  active: "bg-emerald",
  buffering: "bg-sky",
  stalled: "bg-amber",
  error: "bg-rose",
  idle: "bg-muted-foreground",
} as const

export type StatusTone = keyof typeof TONES

export function StatusChip({
  tone = "idle",
  label,
  className,
}: {
  tone?: StatusTone
  label: string
  className?: string
}) {
  return (
    <span
      className={cn(
        "inline-flex h-[22px] items-center gap-2 rounded-full border border-border/70 bg-surface-low px-2.5",
        className,
      )}
    >
      <span className={cn("size-1.5 rounded-full", TONES[tone])} />
      <span className="telemetry text-[0.55rem] text-muted-foreground">{label}</span>
    </span>
  )
}
