"use client"

import { useState } from "react"
import { SaveIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import type { Episode } from "@/lib/ipc/library"

const STATUS_OPTIONS: readonly { value: string; label: string }[] = [
  { value: "ready", label: "Ready" },
  { value: "queued", label: "Queued" },
  { value: "downloaded", label: "Downloaded" },
  { value: "failed", label: "Failed" },
]

interface EpisodeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  episode: Episode | null
  onSave: (values: {
    videoId: string
    url: string
    episodeNumber: number | null
    title: string
    status: string
  }) => void
  saving?: boolean
}

export function EpisodeDialog({
  open,
  onOpenChange,
  episode,
  onSave,
  saving = false,
}: EpisodeDialogProps) {
  const isEdit = episode !== null

  const [videoId, setVideoId] = useState(episode?.videoId ?? "")
  const [url, setUrl] = useState(episode?.url ?? "")
  const [episodeNumber, setEpisodeNumber] = useState(
    episode?.episodeNumber ?? null,
  )
  const [title, setTitle] = useState(episode?.title ?? "")
  const [status, setStatus] = useState(episode?.status ?? "ready")

  const handleSave = () => {
    onSave({
      videoId,
      url,
      episodeNumber,
      title,
      status,
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{isEdit ? "Edit episode" : "Add episode"}</DialogTitle>
          <DialogDescription>
            {isEdit
              ? "Modify the episode's metadata and download state."
              : "Register a new episode for this series."}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3 p-4">
          <div className="space-y-1">
            <Label htmlFor="episode-title">Title</Label>
            <Input
              id="episode-title"
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              disabled={saving}
              placeholder="Episode title"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="episode-video-id">Video ID</Label>
            <Input
              id="episode-video-id"
              value={videoId}
              onChange={(event) => setVideoId(event.target.value)}
              disabled={saving}
              placeholder="e.g. dQw4w9WgXcQ"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="episode-url">Source URL</Label>
            <Input
              id="episode-url"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              disabled={saving}
              placeholder="https://example.com/episode"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="episode-number">Episode number</Label>
            <Input
              id="episode-number"
              type="number"
              value={episodeNumber === null ? "" : String(episodeNumber)}
              onChange={(event) => {
                const parsed = Number(event.target.value)
                setEpisodeNumber(Number.isFinite(parsed) ? parsed : null)
              }}
              disabled={saving}
              placeholder="Optional"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="episode-status">Status</Label>
            <Select
              value={status}
              onValueChange={(value) => {
                if (value) setStatus(value)
              }}
              disabled={saving}
            >
              <SelectTrigger id="episode-status" className="w-40">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {STATUS_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
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
            {isEdit ? "Save changes" : "Create episode"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
