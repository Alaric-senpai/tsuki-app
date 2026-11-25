"use client"

import { useState } from "react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useAddToLibrary } from "@/hooks/use-explore"
import { useSeries } from "@/hooks/use-library"

/** Add the selected episodes of a remote item to the local library. */
export function AddToLibraryDialog({
  open,
  onOpenChange,
  source,
  seriesId,
  episodeUrls,
  defaultName,
  onDone,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  source: string
  seriesId: string
  episodeUrls: string[]
  defaultName: string
  onDone?: () => void
}) {
  const library = useSeries()
  const importer = useAddToLibrary()
  const [mode, setMode] = useState<"existing" | "new">("existing")
  const [targetId, setTargetId] = useState<string | null>(null)
  const [customName, setCustomName] = useState<string | null>(null)
  const [download, setDownload] = useState(false)

  const name = customName ?? defaultName
  const canSubmit = mode === "existing" ? Boolean(targetId) : name.trim().length > 0

  const submit = async () => {
    if (!canSubmit) return
    const { result, error } = await importer.run({
      source,
      id: seriesId,
      episodeUrls,
      download,
      targetSeriesId: mode === "existing" ? Number(targetId) : null,
      seriesName: mode === "new" ? name.trim() : null,
    })
    if (error) {
      toast.error("Could not add to the library", {
        description: error.detail ? `${error.message}\n${error.detail}` : error.message,
      })
      return
    }
    if (!result) return

    toast.success(
      mode === "existing"
        ? `Added to ${result.seriesName}`
        : `Created ${result.seriesName}`,
      {
        description: `${result.added} added · ${result.updated} updated · ${result.total} total${
          result.queued.length > 0
            ? ` · ${result.queued.length} queued for download`
            : ""
        }`,
      },
    )
    if (result.warnings.length > 0) {
      toast.warning("Some episodes need attention", {
        description: result.warnings.slice(0, 3).join("\n"),
      })
    }
    setCustomName(null)
    onOpenChange(false)
    onDone?.()
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add to library</DialogTitle>
          <DialogDescription>
            {episodeUrls.length > 0
              ? `Add ${episodeUrls.length} selected episode(s) to an existing entry or create a new one.`
              : "Add every episode to an existing entry or create a new one."}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-2">
            <Button
              type="button"
              variant={mode === "existing" ? "default" : "outline"}
              disabled={library.data.length === 0}
              onClick={() => setMode("existing")}
            >
              Existing entry
            </Button>
            <Button
              type="button"
              variant={mode === "new" ? "default" : "outline"}
              onClick={() => setMode("new")}
            >
              New entry
            </Button>
          </div>

          {mode === "existing" ? (
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-muted-foreground">Library entry</p>
              <Select value={targetId} onValueChange={setTargetId}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Choose an entry…" />
                </SelectTrigger>
                <SelectContent>
                  {library.data.map((series) => (
                    <SelectItem key={series.id} value={String(series.id)}>
                      {series.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {library.data.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No library entries yet — create a new one instead.
                </p>
              ) : null}
            </div>
          ) : (
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-muted-foreground" htmlFor="new-series-name">
                Name (also used for the download folder)
              </label>
              <Input
                id="new-series-name"
                value={name}
                onChange={(event) => setCustomName(event.target.value)}
                placeholder={defaultName}
              />
            </div>
          )}

          <label className="flex items-center gap-2 text-xs text-foreground">
            <input
              type="checkbox"
              checked={download}
              onChange={(event) => setDownload(event.target.checked)}
              className="size-3.5 accent-primary"
            />
            Download the selected episodes now
          </label>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button disabled={importer.busy || !canSubmit} onClick={() => void submit()}>
            {download ? "Add & download" : "Add to library"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
