"use client"

import { useState } from "react"
import { DownloadIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { useAppConfig } from "@/hooks/use-library"
import { checkAppUpdate, installAppUpdate, type AppRelease } from "@/lib/ipc/app-updater"
import { formatBytes } from "@/lib/utils"

import { SelectSettingRow, type SelectOption } from "./select-setting-row"

const TOGGLE_OPTIONS: readonly SelectOption[] = [
  { value: "1", label: "On" },
  { value: "0", label: "Off" },
]

/** Startup checking is the default; both flags can be changed here or in the dialog. */
export function AppUpdateCard() {
  const [release, setRelease] = useState<AppRelease | null>(null)
  const [checked, setChecked] = useState(false)
  const [busy, setBusy] = useState(false)
  const config = useAppConfig()
  const checkOnStartup = config.data.settings.auto_check_app_updates !== "0"
  const downloadAutomatically = config.data.settings.auto_download_app_updates !== "0"

  const check = async () => {
    setBusy(true)
    try {
      const found = await checkAppUpdate()
      setRelease(found)
      setChecked(true)
      if (!found) toast.success("You are running the latest release")
      else toast.info(`Version ${found.version} is available`)
    } catch (error) {
      toast.error("Could not check for updates", {
        description: error instanceof Error ? error.message : String(error),
      })
    } finally {
      setBusy(false)
    }
  }

  const install = async () => {
    if (!release) return
    setBusy(true)
    try {
      const path = await installAppUpdate()
      toast.success("Installer downloaded", {
        description: path
          ? `${release.assetName} (${formatBytes(Number(release.size))}) — follow your system's install prompt.`
          : `Opened ${release.assetName}.`,
      })
    } catch (error) {
      toast.error("Could not start the update", {
        description: error instanceof Error ? error.message : String(error),
      })
    } finally {
      setBusy(false)
    }
  }

  return (
    <Card className="rounded-xl border border-border/60 bg-surface-low">
      <CardHeader className="border-b border-border/60 px-4 py-3">
        <p className="telemetry text-[0.5rem] text-muted-foreground">Application</p>
        <CardTitle className="font-heading text-base font-semibold">Updates</CardTitle>
        <CardDescription className="text-xs text-muted-foreground">
          Publishes AppImage, .deb and .rpm packages for Linux (Fedora uses .rpm).
        </CardDescription>
      </CardHeader>
      <CardContent className="divide-y divide-border/40 px-4 py-2">
        <div className="space-y-1 py-2">
          <SelectSettingRow
            label="Check on startup"
            hint="Shows the changelog dialog when a newer release exists"
            value={checkOnStartup ? "1" : "0"}
            options={TOGGLE_OPTIONS}
            onChange={(value) => void config.saveSetting("auto_check_app_updates", value)}
          />
          <SelectSettingRow
            label="Automatic downloads"
            hint="Downloads the installer in the background before the dialog opens"
            value={downloadAutomatically ? "1" : "0"}
            options={TOGGLE_OPTIONS}
            onChange={(value) => void config.saveSetting("auto_download_app_updates", value)}
          />
        </div>
        <div className="flex flex-wrap gap-2 py-3">
          <Button variant="outline" size="sm" disabled={busy} onClick={() => void check()}>
            <RefreshCwIcon />
            Check for updates
          </Button>
          <Button size="sm" disabled={busy || !release} onClick={() => void install()}>
            <DownloadIcon />
            Download &amp; install
          </Button>
        </div>
        {release ? (
          <div className="space-y-1 text-xs text-muted-foreground">
            <p className="font-telemetry-micro text-telemetry-micro text-foreground">
              {release.version} · {release.assetName}
            </p>
            {release.notes ? (
              <p className="max-h-32 overflow-auto whitespace-pre-wrap">{release.notes}</p>
            ) : null}
          </div>
        ) : checked ? (
          <p className="text-xs text-muted-foreground">No newer release found.</p>
        ) : null}
      </CardContent>
    </Card>
  )
}
