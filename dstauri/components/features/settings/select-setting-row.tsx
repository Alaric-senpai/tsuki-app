import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

export interface SelectOption {
  value: string
  label: string
}

export function SelectSettingRow({
  label,
  value,
  options,
  onChange,
  busy = false,
  hint,
}: {
  label: string
  value: string
  options: readonly SelectOption[]
  onChange: (value: string) => void
  busy?: boolean
  hint?: string
}) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-3">
      <div className="min-w-0 space-y-0.5">
        <p className="text-sm text-foreground">{label}</p>
        {hint ? <p className="text-xs text-muted-foreground">{hint}</p> : null}
      </div>
      <Select
        value={value}
        disabled={busy}
        onValueChange={(next) => {
          if (next) {
            onChange(next)
          }
        }}
      >
        <SelectTrigger size="sm" className="w-40" aria-label={label}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}
