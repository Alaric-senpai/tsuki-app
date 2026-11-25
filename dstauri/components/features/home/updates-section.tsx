"use client"

import Link from "next/link"
import { RefreshCwIcon, SparklesIcon } from "lucide-react"
import { toast } from "sonner"

import { EmptyState } from "@/components/common/empty-state"
import { StatusChip } from "@/components/common/status-chip"
import { Button } from "@/components/ui/button"
import {
  useAppConfig,
  useSeries,
  useSeriesUpdates,
  useUpdatePreference,
} from "@/hooks/use-library"

/** Local entries the user opted into update checks, plus the latest check
 * results. Nothing is fetched until the user presses "Check now". */
export function UpdatesSection() {
  const series = useSeries()
  const config = useAppConfig()
  const updates = useSeriesUpdates()
  const preference = useUpdatePreference()

  const watched = series.data.filter((entry) => entry.checkUpdates)
  const byId = new Map(updates.updates.map((update) => [update.seriesId, update]))

  const checkNow = async () => {
    const results = await updates.check()
    if (updates.error) {
      toast.error("Could not check for updates", {
        description: updates.error.detail ?? updates.error.message,
      })
      return
    }
    const found = results.reduce((total, item) => total + item.newEpisodes, 0)
    const failed = results.filter((item) => item.error).length
    if (found > 0) {
      toast.success(`${found} new episode${found === 1 ? "" : "s"} found`)
    } else {
      toast.success("Everything is up to date")
    }
    if (failed > 0) {
      toast.warning(`${failed} source${failed === 1 ? "" : "s"} could not be reached`)
    }
  }

  const toggle = async (id: number, enabled: boolean) => {
    const ok = await preference.set(id, enabled)
    if (ok) {
      toast.success(enabled ? "Watching for updates" : "Update checks disabled")
      await series.refresh()
    } else {
      toast.error("Could not change the update preference")
    }
  }

  const source = config.data.explore.update_source || "donghuastream"

  return (
    <section className="space-y-3">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="telemetry text-[0.55rem] text-muted-foreground">
            Local library · {source}
          </p>
          <h2 className="font-heading text-lg font-semibold tracking-tight text-foreground">
            Updates
          </h2>
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={checkNow}
          disabled={updates.checking || watched.length === 0}
        >
          <RefreshCwIcon className={updates.checking ? "size-4 animate-spin" : "size-4"} />
          {updates.checking ? "Checking…" : "Check now"}
        </Button>
      </div>

      {watched.length === 0 ? (
        <EmptyState
          icon={SparklesIcon}
          title="No series marked for updates"
          description="Opt a series into update checks from its detail page and new episodes will show up here."
          action={
            <Button variant="outline" size="sm" nativeButton={false} render={<Link href="/library" />}>
              Open library
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 md:grid-cols-2">
          {watched.map((entry) => {
            const result = byId.get(entry.id)
            const count = result?.newEpisodes ?? entry.updateCount
            return (
              <article
                key={entry.id}
                className="flex items-center gap-3 rounded-xl border border-border/60 bg-surface-low p-4"
              >
                <div className="min-w-0 flex-1">
                  <Link
                    href={`/series?id=${entry.id}`}
                    className="block truncate font-heading text-sm font-semibold text-foreground hover:underline"
                  >
                    {entry.name}
                  </Link>
                  <p className="mt-0.5 truncate text-xs text-muted-foreground">
                    {result?.error
                      ? result.error
                      : entry.lastCheckedAt
                        ? `Last checked ${new Date(entry.lastCheckedAt).toLocaleString()}`
                        : "Not checked yet"}
                  </p>
                </div>
                {count > 0 ? (
                  <StatusChip tone="active" label={`${count} new`} />
                ) : (
                  <StatusChip tone="idle" label="Up to date" />
                )}
                <Button
                  size="icon-xs"
                  variant="ghost"
                  title="Stop checking this series"
                  disabled={preference.busy}
                  onClick={() => toggle(entry.id, false)}
                >
                  <span aria-hidden="true">×</span>
                </Button>
              </article>
            )
          })}
        </div>
      )}
    </section>
  )
}
