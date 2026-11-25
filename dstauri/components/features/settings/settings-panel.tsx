"use client"

import { useEffect, useState } from "react"

import { toast } from "sonner"

import { ErrorPanel } from "@/components/common/data-state"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import {
  useAppConfig,
  useLegacyDetection,
  useLegacyImport,
  useLegacyPull,
} from "@/hooks/use-library"
import type {
  SetExploreConfigArgs,
  SetPlayerConfigArgs,
  SetYouTubeConfigArgs,
} from "@/lib/ipc/config"
import { setDownloadConcurrency } from "@/lib/ipc/downloads"

import { SelectSettingRow, type SelectOption } from "./select-setting-row"
import { SettingDialog } from "./setting-dialog"
import { SettingRow } from "./setting-row"

const QUALITY_OPTIONS: readonly SelectOption[] = [
  { value: "360p", label: "360p" },
  { value: "480p", label: "480p" },
  { value: "720p", label: "720p" },
  { value: "1080p", label: "1080p" },
  { value: "best", label: "Best available" },
]
const CONTAINER_OPTIONS: readonly SelectOption[] = [
  { value: "mp4", label: "MP4" },
  { value: "mkv", label: "MKV" },
  { value: "webm", label: "WebM" },
]
const COOKIE_OPTIONS: readonly SelectOption[] = [
  { value: "none", label: "None" },
  { value: "chrome", label: "Chrome" },
  { value: "firefox", label: "Firefox" },
  { value: "chromium", label: "Chromium" },
]
const FOLDER_OPTIONS: readonly SelectOption[] = [
  { value: "1", label: "On" },
  { value: "0", label: "Off" },
]
const ENGINE_OPTIONS: readonly SelectOption[] = [
  { value: "auto", label: "Auto" },
  { value: "mpv", label: "mpv" },
  { value: "html", label: "HTML video" },
  { value: "external", label: "External player" },
]
const SAFE_SEARCH_OPTIONS: readonly SelectOption[] = [
  { value: "none", label: "None" },
  { value: "moderate", label: "Moderate" },
  { value: "strict", label: "Strict" },
]
// YouTube has no episode list to diff against, so it is never an update source.
const CONCURRENCY_OPTIONS: readonly SelectOption[] = [
  { value: "1", label: "1 at a time" },
  { value: "2", label: "2 at once" },
  { value: "3", label: "3 at once (default)" },
  { value: "4", label: "4 at once" },
  { value: "6", label: "6 at once" },
  { value: "8", label: "8 at once" },
]
const UPDATE_SOURCE_OPTIONS: readonly SelectOption[] = [
  { value: "donghuastream", label: "DonghuaStream" },
  { value: "animecube", label: "AnimeCube" },
]

type DialogKey =
  | "global_save_dir"
  | "sub_langs"
  | "max_retries"
  | "rate_limit_mb"
  | "concurrent_downloads"
  | "player_volume"
  | "external_player"
  | "explore_result_limit"
  | "explore_cache_minutes"
  | "youtube_api_key"
  | "youtube_region"
  | "youtube_language"

interface DialogConfig {
  key: DialogKey
  title: string
  description: string
  label: string
  value: string
  type?: "text" | "number" | "password"
  placeholder?: string
  min?: number
  step?: number
  onSave: (value: string) => void
}

function numeric(value: string): number | null {
  const parsed = Number(value)
  return value.trim() && Number.isFinite(parsed) && parsed >= 0 ? parsed : null
}

export function SettingsPanel() {
  const config = useAppConfig()
  const migration = useLegacyImport()
  const { loadPath } = migration
  const legacy = useLegacyDetection()
  const legacyPull = useLegacyPull()
  const [dialogKey, setDialogKey] = useState<DialogKey | null>(null)

  useEffect(() => {
    void loadPath()
  }, [loadPath])

  const flat = (key: string) => config.data.settings[key] ?? ""
  const saveFlat = (key: string) => (value: string) => {
    void config.saveSetting(key, value)
  }

  const playerArgs: SetPlayerConfigArgs = {
    engine: config.data.player.engine,
    volume: config.data.player.volume,
    externalPlayer: config.data.player.external_player,
    rememberPosition: config.data.player.remember_position,
    autoPlayNext: config.data.player.auto_play_next,
  }
  const exploreArgs: SetExploreConfigArgs = {
    resultLimit: config.data.explore.result_limit,
    cacheMinutes: config.data.explore.cache_minutes,
    updateSource: config.data.explore.update_source,
  }

  const pullLegacy = async () => {
    const result = await legacyPull.pull()
    if (!result) {
      const detail = legacyPull.error
      toast.error("Could not pull legacy data", {
        description: detail ? (detail.detail ?? detail.message) : undefined,
      })
      return
    }
    toast.success("Legacy data imported", {
      description: `${result.summary.seriesCreated} series created · ${result.summary.episodesAdded} episodes added`,
    })
    void legacy.refresh()
    void config.refresh()
  }
  const youtubeArgs: SetYouTubeConfigArgs = {
    apiKey: config.data.youtube.api_key,
    regionCode: config.data.youtube.region_code,
    language: config.data.youtube.language,
    safeSearch: config.data.youtube.safe_search,
  }

  const dialogs: DialogConfig[] = [
    {
      key: "global_save_dir",
      title: "Download folder",
      description: "Where completed episodes are stored on this device.",
      label: "Folder path",
      value: flat("global_save_dir"),
      onSave: saveFlat("global_save_dir"),
    },
    {
      key: "sub_langs",
      title: "Subtitle languages",
      description: "Comma-separated language codes requested from yt-dlp.",
      label: "Language codes",
      value: flat("sub_langs"),
      onSave: saveFlat("sub_langs"),
    },
    {
      key: "max_retries",
      title: "Retry budget",
      description: "Per-episode attempts before a failure is reported.",
      label: "Attempts",
      value: flat("max_retries"),
      type: "number",
      min: 0,
      step: 1,
      onSave: (value) => {
        if (numeric(value) !== null) {
          saveFlat("max_retries")(value)
        }
      },
    },
    {
      key: "rate_limit_mb",
      title: "Bandwidth limit",
      description: "Zero means no bandwidth cap.",
      label: "MB per second",
      value: flat("rate_limit_mb"),
      type: "number",
      min: 0,
      step: 1,
      onSave: (value) => {
        if (numeric(value) !== null) {
          saveFlat("rate_limit_mb")(value)
        }
      },
    },
    {
      key: "concurrent_downloads",
      title: "Concurrent downloads",
      description: "Sequential queue size used for preparation work.",
      label: "Tasks",
      value: flat("concurrent_downloads"),
      type: "number",
      min: 1,
      step: 1,
      onSave: (value) => {
        if (numeric(value) !== null) {
          saveFlat("concurrent_downloads")(value)
        }
      },
    },
    {
      key: "player_volume",
      title: "Player volume",
      description: "Initial volume from 0 (muted) to 1.",
      label: "Volume",
      value: String(config.data.player.volume),
      type: "number",
      min: 0,
      step: 0.05,
      onSave: (value) => {
        const parsed = numeric(value)
        if (parsed !== null) {
          void config.savePlayerConfig({ ...playerArgs, volume: Math.min(parsed, 1) })
        }
      },
    },
    {
      key: "external_player",
      title: "External player command",
      description: "Command used when the external engine is selected.",
      label: "Command or path",
      value: config.data.player.external_player,
      placeholder: "mpv",
      onSave: (value) => {
        void config.savePlayerConfig({ ...playerArgs, externalPlayer: value })
      },
    },
    {
      key: "explore_result_limit",
      title: "Explore results",
      description: "Maximum number of results shown per source search.",
      label: "Results",
      value: String(config.data.explore.result_limit),
      type: "number",
      min: 1,
      step: 1,
      onSave: (value) => {
        const parsed = numeric(value)
        if (parsed !== null) {
          void config.saveExploreConfig({ ...exploreArgs, resultLimit: Math.round(parsed) })
        }
      },
    },
    {
      key: "explore_cache_minutes",
      title: "Search cache",
      description: "How long YouTube search results stay cached on disk.",
      label: "Minutes",
      value: String(config.data.explore.cache_minutes),
      type: "number",
      min: 0,
      step: 1,
      onSave: (value) => {
        const parsed = numeric(value)
        if (parsed !== null) {
          void config.saveExploreConfig({ ...exploreArgs, cacheMinutes: Math.round(parsed) })
        }
      },
    },
    {
      key: "youtube_api_key",
      title: "YouTube API key",
      description: "Stored in your local config file; no key is bundled with the app.",
      label: "API key",
      value: config.data.youtube.api_key,
      type: "password",
      onSave: (value) => {
        void config.saveYouTubeConfig({ ...youtubeArgs, apiKey: value })
      },
    },
    {
      key: "youtube_region",
      title: "YouTube region",
      description: "Two-letter region code passed to the Data API.",
      label: "Region code",
      value: config.data.youtube.region_code,
      onSave: (value) => {
        void config.saveYouTubeConfig({ ...youtubeArgs, regionCode: value })
      },
    },
    {
      key: "youtube_language",
      title: "YouTube language",
      description: "Language code used for search results and subtitles.",
      label: "Language code",
      value: config.data.youtube.language,
      onSave: (value) => {
        void config.saveYouTubeConfig({ ...youtubeArgs, language: value })
      },
    },
  ]

  const activeDialog = dialogs.find((item) => item.key === dialogKey)
  const busy = Boolean(config.savingKey)

  return (
    <div className="space-y-4 px-6 py-6">
      {config.error ? (
        <ErrorPanel error={config.error} onRetry={config.refresh} title="Settings could not load" />
      ) : null}
      {config.saveError ? (
        <ErrorPanel error={config.saveError} title="Setting could not be saved" />
      ) : null}

      <div className="grid gap-4 xl:grid-cols-2">
        <Card className="rounded-xl border border-border/60 bg-surface-low">
          <CardHeader className="border-b border-border/60 px-4 py-3">
            <p className="telemetry text-[0.5rem] text-muted-foreground">yt-dlp & queue</p>
            <CardTitle className="font-heading text-base font-semibold">Downloads</CardTitle>
          </CardHeader>
          <CardContent className="divide-y divide-border/40 p-0">
            <SettingRow
              label="Download folder"
              value={flat("global_save_dir")}
              onEdit={() => setDialogKey("global_save_dir")}
              busy={busy}
            />
            <SelectSettingRow
              label="Concurrent downloads"
              hint="How many episodes can download at the same time"
              value={flat("concurrent_downloads")}
              options={CONCURRENCY_OPTIONS}
              onChange={(value) => {
                void setDownloadConcurrency(Number(value))
                  .then(() => {
                    toast.success("Download concurrency updated", {
                      description: `${value} at once`,
                    })
                    config.refresh()
                  })
                  .catch((error: unknown) => {
                    toast.error("Could not change concurrency", {
                      description: String(error),
                    })
                  })
              }}
              busy={busy}
            />
            <SelectSettingRow
              label="Default quality"
              value={flat("default_quality")}
              options={QUALITY_OPTIONS}
              onChange={(value) => void config.saveSetting("default_quality", value)}
              busy={busy}
            />
            <SelectSettingRow
              label="Container"
              value={flat("default_container")}
              options={CONTAINER_OPTIONS}
              onChange={(value) => void config.saveSetting("default_container", value)}
              busy={busy}
            />
            <SettingRow
              label="Subtitle languages"
              value={flat("sub_langs")}
              onEdit={() => setDialogKey("sub_langs")}
              busy={busy}
            />
            <SettingRow
              label="Retry budget"
              value={flat("max_retries")}
              onEdit={() => setDialogKey("max_retries")}
              busy={busy}
            />
            <SettingRow
              label="Bandwidth limit (MB/s)"
              value={flat("rate_limit_mb")}
              onEdit={() => setDialogKey("rate_limit_mb")}
              busy={busy}
            />
            <SelectSettingRow
              label="Series subfolder"
              value={flat("per_series_subfolder")}
              options={FOLDER_OPTIONS}
              onChange={(value) => void config.saveSetting("per_series_subfolder", value)}
              busy={busy}
            />
            <SettingRow
              label="Concurrent downloads"
              value={flat("concurrent_downloads")}
              onEdit={() => setDialogKey("concurrent_downloads")}
              busy={busy}
            />
            <SelectSettingRow
              label="Browser cookies"
              value={flat("browser_cookies")}
              options={COOKIE_OPTIONS}
              onChange={(value) => void config.saveSetting("browser_cookies", value)}
              busy={busy}
            />
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card className="rounded-xl border border-border/60 bg-surface-low">
            <CardHeader className="border-b border-border/60 px-4 py-3">
              <p className="telemetry text-[0.5rem] text-muted-foreground">Playback & catalogs</p>
              <CardTitle className="font-heading text-base font-semibold">
                Engine status
              </CardTitle>
            </CardHeader>
            <CardContent className="divide-y divide-border/40 p-0">
              <SelectSettingRow
                label="Player engine"
                value={config.data.player.engine}
                options={ENGINE_OPTIONS}
                onChange={(value) => void config.savePlayerConfig({ ...playerArgs, engine: value })}
                busy={busy}
              />
              <SettingRow
                label="Volume"
                value={config.data.player.volume.toFixed(2)}
                onEdit={() => setDialogKey("player_volume")}
                busy={busy}
              />
              <SettingRow
                label="External player"
                value={config.data.player.external_player || "Default"}
                onEdit={() => setDialogKey("external_player")}
                busy={busy}
              />
              <SelectSettingRow
                label="Remember position"
                value={config.data.player.remember_position ? "1" : "0"}
                options={FOLDER_OPTIONS}
                onChange={(value) =>
                  void config.savePlayerConfig({
                    ...playerArgs,
                    rememberPosition: value === "1",
                  })
                }
                busy={busy}
              />
              <SelectSettingRow
                label="Auto-play next episode"
                value={config.data.player.auto_play_next ? "1" : "0"}
                options={FOLDER_OPTIONS}
                onChange={(value) =>
                  void config.savePlayerConfig({
                    ...playerArgs,
                    autoPlayNext: value === "1",
                  })
                }
                busy={busy}
              />
              <SettingRow
                label="Explore results"
                value={String(config.data.explore.result_limit)}
                onEdit={() => setDialogKey("explore_result_limit")}
                busy={busy}
              />
              <SettingRow
                label="Search cache (minutes)"
                value={String(config.data.explore.cache_minutes)}
                onEdit={() => setDialogKey("explore_cache_minutes")}
                busy={busy}
              />
              <SelectSettingRow
                label="Updates source"
                hint="Used by Home → Updates to check for new episodes"
                value={config.data.explore.update_source}
                options={UPDATE_SOURCE_OPTIONS}
                onChange={(value) => void config.saveExploreConfig({ ...exploreArgs, updateSource: value })}
                busy={busy}
              />
            </CardContent>
          </Card>

          <Card className="rounded-xl border border-border/60 bg-surface-low">
            <CardHeader className="border-b border-border/60 px-4 py-3">
              <p className="telemetry text-[0.5rem] text-muted-foreground">Data API</p>
              <CardTitle className="font-heading text-base font-semibold">YouTube</CardTitle>
            </CardHeader>
            <CardContent className="divide-y divide-border/40 p-0">
              <SettingRow
                label="API key"
                value={config.data.youtube.api_key ? "Configured" : "Not configured"}
                onEdit={() => setDialogKey("youtube_api_key")}
                busy={busy}
              />
              <SettingRow
                label="Region code"
                value={config.data.youtube.region_code}
                onEdit={() => setDialogKey("youtube_region")}
                busy={busy}
              />
              <SettingRow
                label="Language"
                value={config.data.youtube.language}
                onEdit={() => setDialogKey("youtube_language")}
                busy={busy}
              />
              <SelectSettingRow
                label="Safe search"
                value={config.data.youtube.safe_search}
                options={SAFE_SEARCH_OPTIONS}
                onChange={(value) => void config.saveYouTubeConfig({ ...youtubeArgs, safeSearch: value })}
                busy={busy}
              />
            </CardContent>
          </Card>
        </div>
      </div>

      {legacy.data.detected ? (
        <Card className="rounded-xl border border-border/60 bg-surface-low">
          <CardHeader className="border-b border-border/60 px-4 py-3">
            <p className="telemetry text-[0.5rem] text-muted-foreground">Migration</p>
            <CardTitle className="font-heading text-base font-semibold">Legacy</CardTitle>
            <CardDescription className="text-xs text-muted-foreground">
              {legacy.data.note}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3 p-4">
            <p className="text-sm text-muted-foreground">
              {legacy.data.exportReady
                ? `Staged export: ${legacy.data.seriesCount} series · ${legacy.data.episodeCount} episodes.`
                : "No staged export yet. Pulling will create one from the legacy database."}
            </p>
            {legacyPull.error ? (
              <ErrorPanel error={legacyPull.error} title="Legacy pull failed" />
            ) : null}
            {legacyPull.summary ? (
              <p className="text-sm text-foreground">
                Last pull: {legacyPull.summary.summary.seriesCreated} series created · {" "}
                {legacyPull.summary.summary.episodesAdded} episodes added
                {legacyPull.summary.summary.episodesUpdated
                  ? ` · ${legacyPull.summary.summary.episodesUpdated} updated`
                  : ""}
              </p>
            ) : null}
            {config.data.meta.legacy_data_imported ? (
              <p className="text-xs text-muted-foreground">Legacy data was already imported.</p>
            ) : null}
            <Button
              size="sm"
              variant="outline"
              disabled={legacyPull.pulling || busy}
              onClick={() => void pullLegacy()}
            >
              {legacyPull.pulling ? "Pulling…" : "Pull legacy data"}
            </Button>
          </CardContent>
        </Card>
      ) : null}

      <p className="telemetry text-[0.55rem] text-muted-foreground">
        Config version {config.data.version}
        {config.savingKey ? ` · Saving ${config.savingKey}` : ""}
      </p>

      {activeDialog ? (
        <SettingDialog
          key={activeDialog.key}
          open={dialogKey !== null}
          onOpenChange={(open) => {
            if (!open) {
              setDialogKey(null)
            }
          }}
          title={activeDialog.title}
          description={activeDialog.description}
          label={activeDialog.label}
          value={activeDialog.value}
          type={activeDialog.type}
          placeholder={activeDialog.placeholder}
          min={activeDialog.min}
          step={activeDialog.step}
          saving={busy}
          onSave={activeDialog.onSave}
        />
      ) : null}
    </div>
  )
}
