"use client"

import { useState } from "react"
import { FolderIcon, ImageIcon, SaveIcon } from "lucide-react"

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
import type { SeriesSummary } from "@/lib/ipc/library"

interface SeriesEditDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  series: Pick<SeriesSummary, "name" | "description" | "saveDir" | "bannerSource">
  onPickBanner: () => void
  onSave: (values: {
    name: string
    description: string
    saveDir: string
    bannerSource: string
  }) => void
  saving?: boolean
}

export function SeriesEditDialog({
  open,
  onOpenChange,
  series,
  onPickBanner,
  onSave,
  saving = false,
}: SeriesEditDialogProps) {
  const [name, setName] = useState(series.name)
  const [description, setDescription] = useState(series.description)
  const [saveDir, setSaveDir] = useState(series.saveDir)
  const [bannerSource, setBannerSource] = useState(series.bannerSource)

  const handleSave = () => {
    onSave({ name, description, saveDir, bannerSource })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit series</DialogTitle>
          <DialogDescription>
            Update the series metadata and save location.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3 p-4">
          <div className="space-y-1">
            <Label htmlFor="series-name">Name</Label>
            <Input
              id="series-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={saving}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="series-description">Description</Label>
            <textarea
              id="series-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={saving}
              rows={3}
              className="w-full rounded-lg border border-input bg-transparent px-2.5 py-1 text-base transition-colors outline-none disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-input/50 disabled:opacity-50 md:text-sm dark:bg-input/30 dark:disabled:bg-input/80"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="series-save-dir">Save folder</Label>
            <div className="flex items-center gap-2">
              <Input
                id="series-save-dir"
                value={saveDir}
                onChange={(event) => setSaveDir(event.target.value)}
                disabled={saving}
                className="flex-1"
              />
              <Button
                variant="outline"
                size="sm"
                disabled={saving}
                aria-label="Pick folder"
              >
                <FolderIcon className="size-3.5" />
              </Button>
            </div>
          </div>
          <div className="space-y-1">
            <Label htmlFor="series-banner-source">Banner source</Label>
            <div className="flex items-center gap-2">
              <Input
                id="series-banner-source"
                value={bannerSource}
                onChange={(event) => setBannerSource(event.target.value)}
                disabled={saving}
                className="flex-1"
              />
              <Button
                variant="outline"
                size="sm"
                onClick={onPickBanner}
                disabled={saving}
                aria-label="Pick banner file"
              >
                <ImageIcon className="size-3.5" />
              </Button>
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={saving}
          >
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            <SaveIcon />
            Save changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
