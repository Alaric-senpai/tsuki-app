"use client"

import { useId, useState } from "react"
import { SaveIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

export function SettingDialog({
  open,
  onOpenChange,
  title,
  description,
  label,
  value,
  onSave,
  type = "text",
  placeholder,
  min,
  step,
  saving = false,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  label: string
  value: string
  onSave: (next: string) => void
  type?: "text" | "number" | "password"
  placeholder?: string
  min?: number
  step?: number
  saving?: boolean
}) {
  const inputId = useId()
  const [draft, setDraft] = useState(value)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description ? <DialogDescription>{description}</DialogDescription> : null}
        </DialogHeader>
        <div className="p-4">
          <Label htmlFor={inputId}>{label}</Label>
          <Input
            id={inputId}
            type={type}
            value={draft}
            min={min}
            step={step}
            placeholder={placeholder}
            disabled={saving}
            onChange={(event) => setDraft(event.target.value)}
            className="mt-2 font-mono text-xs"
          />
        </div>
        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={saving}
          >
            Cancel
          </Button>
          <Button
            onClick={() => {
              onSave(draft)
              onOpenChange(false)
            }}
            disabled={saving}
          >
            <SaveIcon />
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
