"use client"

import { DownloadIcon } from "lucide-react"

import { ErrorPanel } from "@/components/common/data-state"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import { useAppConfig } from "@/hooks/use-library"
import { useAppUpdate } from "@/hooks/use-app-update"
import { formatBytes } from "@/lib/utils"

/** Global changelog dialog shown when a startup check finds a new release. */
export function AppUpdateDialog() {
  const update = useAppUpdate()
  const config = useAppConfig()
  const settings = config.data.settings
  const automatic = settings.auto_check_app_updates !== "0" && settings.auto_download_app_updates !== "0"
  const release = update.release

  return (
    <Dialog
      open={release !== null}
      onOpenChange={(open) => {
        if (!open) update.skip()
      }}
    >
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Version {release?.version ?? ""} is available</DialogTitle>
          <DialogDescription>
            {release?.assetName
              ? `${release.assetName} · ${formatBytes(Number(release.size))}`
              : "A newer release of Tsuki is ready."}
          </DialogDescription>
        </DialogHeader>

        <div className="max-h-64 space-y-3 overflow-y-auto text-sm text-muted-foreground">
          <p className="telemetry text-[0.5rem] text-muted-foreground">Release notes</p>
          {release?.notes ? (
            <p className="whitespace-pre-wrap break-words">{release.notes}</p>
          ) : (
            <p>No release notes were published for this version.</p>
          )}
          {update.error ? <ErrorPanel error={{ code: "update", message: update.error }} /> : null}
        </div>

        <div className="flex items-center gap-2">
          <Checkbox
            id="automatic-app-updates"
            checked={automatic}
            onCheckedChange={(checked) => {
              void update.setAutomatic(checked === true, checked === true).then(() => config.refresh())
            }}
          />
          <Label htmlFor="automatic-app-updates" className="text-xs text-muted-foreground">
            Automatically check for and download future updates
          </Label>
        </div>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={update.skip}>
            Skip for now
          </Button>
          <Button size="sm" disabled={update.busy} onClick={() => void update.start()}>
            <DownloadIcon />
            {update.busy ? "Starting…" : "Start update"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
