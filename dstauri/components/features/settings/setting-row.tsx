import { PencilIcon } from "lucide-react"

import { Button } from "@/components/ui/button"

export function SettingRow({
  label,
  value,
  hint,
  onEdit,
  busy = false,
}: {
  label: string
  value: string
  hint?: string
  onEdit?: () => void
  busy?: boolean
}) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-3">
      <div className="min-w-0 space-y-0.5">
        <p className="text-sm text-foreground">{label}</p>
        {hint ? <p className="text-xs text-muted-foreground">{hint}</p> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <span
          title={value}
          className="max-w-[12rem] truncate font-mono text-xs text-muted-foreground"
        >
          {value || "—"}
        </span>
        {onEdit ? (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={onEdit}
            disabled={busy}
            aria-label={`Edit ${label}`}
          >
            <PencilIcon />
          </Button>
        ) : null}
      </div>
    </div>
  )
}
