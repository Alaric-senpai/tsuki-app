"use client"

import Image from "next/image"
import { useCallback } from "react"
import { DownloadIcon, XIcon } from "lucide-react"

import { Drawer, DrawerClose, DrawerContent, DrawerFooter, DrawerHeader, DrawerTitle } from "@/components/ui/drawer"
import { Button } from "@/components/ui/button"
import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { useExploreSeries, useSourceImport } from "@/hooks/use-explore"

export function SeriesDrawer({
  source,
  seriesId,
  open,
  onOpenChange,
}: {
  source: string
  seriesId: string | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { data: series, loading, error, refresh } = useExploreSeries(source, seriesId)
  const importMut = useSourceImport()

  const doImport = useCallback(
    (download: boolean) => {
      if (!seriesId) return Promise.resolve(null)
      return importMut.importSeries(source, seriesId, [], download)
    },
    [seriesId, source, importMut],
  )

  const meta: string[] = []
  if (series?.status) meta.push(series.status)
  if (series?.kind) meta.push(series.kind)
  if (series?.season) meta.push(series.season)
  if (series?.studio) meta.push(series.studio)
  if (series?.episodeCount) meta.push(`${series.episodeCount} eps`)
  if (series?.released) meta.push(String(series.released))

  return (
    <Drawer open={open} onOpenChange={onOpenChange}>
      <DrawerContent>
        <div className="mx-auto w-full max-w-lg max-h-[90dvh] overflow-y-auto">
          {loading ? (
            <div className="p-6">
              <LoadingGrid count={3} />
            </div>
          ) : error ? (
            <div className="p-6">
              <ErrorPanel
                title="Series details could not be loaded"
                error={error}
                onRetry={refresh}
              />
            </div>
          ) : series ? (
            <>
              <DrawerHeader className="gap-2 pb-2">
                {/* Thumbnail + title + meta */}
                <div className="flex gap-3">
                  {series.thumbnail && (
                    <Image
                      src={series.thumbnail}
                      alt={series.title}
                      width={80}
                      height={120}
                      unoptimized
                      className="w-20 shrink-0 rounded-lg object-cover aspect-[2/3]"
                    />
                  )}
                  <div className="min-w-0">
                    <div className="flex items-start gap-2">
                      <DrawerTitle className="text-base leading-tight">
                        {series.title}
                      </DrawerTitle>
                      <DrawerClose
                        render={
                          <Button
                            variant="ghost"
                            size="icon-xs"
                            className="shrink-0"
                          />
                        }
                      >
                        <XIcon />
                        <span className="sr-only">Close</span>
                      </DrawerClose>
                    </div>
                    <p className="telemetry mt-1 text-[0.6rem] text-muted-foreground">
                      {meta.join(" · ")}
                    </p>
                    <div className="mt-1.5 flex flex-wrap gap-1">
                      {series.genres?.slice(0, 4).map((g) => (
                        <span
                          key={g}
                          className="rounded bg-surface-container px-1.5 py-0.5 text-[0.55rem] text-on-surface-variant capitalize"
                        >
                          {g}
                        </span>
                      ))}
                    </div>
                  </div>
                </div>

                {/* Description */}
                {series.description && (
                  <p className="text-xs text-muted-foreground leading-relaxed">
                    {series.description.slice(0, 240)}
                    {series.description.length > 240 ? "…" : ""}
                  </p>
                )}

                {/* Import result */}
                {importMut.last && (
                  <div className="rounded-lg border border-border/60 bg-surface-low p-3 text-xs space-y-1">
                    <p className="font-medium text-foreground">
                      {importMut.last.added} added, {importMut.last.updated} updated,{" "}
                      {importMut.last.total} total
                    </p>
                    {importMut.last.queued.length > 0 && (
                      <p className="text-muted-foreground">
                        {importMut.last.queued.length} episode(s) queued for download
                      </p>
                    )}
                    {importMut.last.warnings.map((w, i) => (
                      <p key={i} className="text-amber text-[0.6rem]">
                        ⚠ {w}
                      </p>
                    ))}
                  </div>
                )}

                {importMut.error && (
                  <ErrorPanel
                    title="Import failed"
                    error={importMut.error}
                    onRetry={() => doImport(false)}
                  />
                )}
              </DrawerHeader>

              {/* Episode list */}
              <div className="px-6 pb-4 space-y-1 max-h-64 overflow-y-auto">
                <p className="text-xs font-semibold text-foreground mb-2">
                  Episodes
                </p>
                {series.episodes.map((ep) => (
                  <div
                    key={ep.url}
                    className="flex items-center justify-between rounded-lg border border-border/40 bg-surface-low px-3 py-2 text-xs"
                  >
                    <div className="min-w-0">
                      <span className="font-medium text-foreground">
                        {ep.numberText ?? ep.episodeNumber?.toString() ?? "?"}
                      </span>
                      <span className="ml-2 truncate text-muted-foreground">
                        {ep.title}
                      </span>
                    </div>
                    {ep.date && (
                      <span className="telemetry shrink-0 text-[0.55rem] text-muted-foreground ml-2">
                        {ep.date}
                      </span>
                    )}
                  </div>
                ))}
              </div>

              <DrawerFooter className="flex-row gap-2 pt-2 border-t">
                <Button
                  variant="outline"
                  className="flex-1"
                  disabled={importMut.busy}
                  onClick={() => doImport(false)}
                >
                  Add to library
                </Button>
                <Button
                  className="flex-1"
                  disabled={importMut.busy}
                  onClick={() => doImport(true)}
                >
                  <DownloadIcon className="size-3.5 mr-1.5" />
                  Add &amp; download
                </Button>
              </DrawerFooter>
            </>
          ) : (
            <div className="p-6 text-center text-sm text-muted-foreground">
              No series selected.
            </div>
          )}
        </div>
      </DrawerContent>
    </Drawer>
  )
}
