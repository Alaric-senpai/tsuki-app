"use client"

import { useState } from "react"
import { RefreshCwIcon, SaveIcon } from "lucide-react"

import { ErrorPanel } from "@/components/common/data-state"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { useSeriesMutations } from "@/hooks/use-library"
import type { SeriesSummary } from "@/lib/ipc/library"

export function SeriesEditSheet({
  series,
  open,
  onOpenChange,
  onSaved,
}: {
  series: SeriesSummary | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved?: () => void
}) {
  const mutations = useSeriesMutations()
  // The sheet is keyed by series and open state in the library page, so its
  // draft fields are deliberately initialized once per editing session.
  const [name, setName] = useState(series?.name ?? "")
  const [description, setDescription] = useState(series?.description ?? "")
  const [saveDir, setSaveDir] = useState(series?.saveDir ?? "")
  const [bannerPath, setBannerPath] = useState("")

  const save = async () => {
    if (!series || !name.trim()) {
      return
    }
    const updated = await mutations.updateSeries({
      id: series.id,
      name: name.trim(),
      saveDir: saveDir.trim(),
      description: description.trim(),
    })
    if (!updated) {
      return
    }
    if (bannerPath.trim()) {
      const withBanner = await mutations.setBanner({
        id: series.id,
        sourcePath: bannerPath.trim(),
      })
      if (!withBanner) {
        return
      }
    }
    onSaved?.()
    onOpenChange(false)
  }

  const resync = async () => {
    const changed = await mutations.resyncBanners()
    if (changed !== null) {
      onSaved?.()
    }
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="gap-0 p-0 sm:max-w-md">
        <SheetHeader className="border-b border-border/60">
          <SheetTitle>Edit series</SheetTitle>
          <SheetDescription>
            Update catalog details and the artwork mirror for this series.
          </SheetDescription>
        </SheetHeader>

        <div className="flex-1 space-y-4 overflow-y-auto p-4">
          {mutations.error ? (
            <ErrorPanel error={mutations.error} title="Series update failed" />
          ) : null}
          <div className="space-y-1.5">
            <Label htmlFor="series-name">Name</Label>
            <Input
              id="series-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={mutations.busy}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="series-description">Description</Label>
            <textarea
              id="series-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={mutations.busy}
              rows={4}
              className="w-full min-w-0 rounded-lg border border-input bg-transparent px-2.5 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="series-save-dir">Save folder</Label>
            <Input
              id="series-save-dir"
              value={saveDir}
              onChange={(event) => setSaveDir(event.target.value)}
              disabled={mutations.busy}
              className="font-mono text-xs"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="series-banner-path">Banner source path</Label>
            <Input
              id="series-banner-path"
              value={bannerPath}
              onChange={(event) => setBannerPath(event.target.value)}
              disabled={mutations.busy}
              placeholder="/path/to/legacy/cache/artwork.jpg"
              className="font-mono text-xs"
            />
            <p className="text-xs text-muted-foreground">
              Paste a local path copied from the legacy artwork cache. File pickers are
              unavailable in this build. Leave blank to keep the current banner.
            </p>
          </div>
        </div>

        <SheetFooter className="border-t border-border/60">
          <Button
            variant="outline"
            onClick={() => void resync()}
            disabled={mutations.busy}
          >
            <RefreshCwIcon />
            Copy from legacy cache
          </Button>
          <div className="flex gap-2">
            <Button
              variant="ghost"
              onClick={() => onOpenChange(false)}
              disabled={mutations.busy}
            >
              Cancel
            </Button>
            <Button
              onClick={() => void save()}
              disabled={mutations.busy || !name.trim()}
            >
              <SaveIcon />
              {mutations.busy ? "Saving…" : "Save"}
            </Button>
          </div>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  )
}
