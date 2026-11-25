"use client"

import { useState } from "react"
import { SearchIcon, TrendingUpIcon } from "lucide-react"

import { ErrorPanel, LoadingGrid } from "@/components/common/data-state"
import { EmptyState } from "@/components/common/empty-state"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { AppIpcError } from "@/lib/ipc/client"
import type { YouTubeVideo } from "@/lib/ipc/youtube"

export interface YouTubePanelProps {
  videos: YouTubeVideo[]
  loading: boolean
  error: AppIpcError | null
  searched: boolean
  query: string
  onSearch: (term: string) => void
  onTrending: () => void
}

export function YouTubePanel({
  videos,
  loading,
  error,
  searched,
  query,
  onSearch,
  onTrending,
}: YouTubePanelProps) {
  const [term, setTerm] = useState("")

  const submit = (event: React.FormEvent) => {
    event.preventDefault()
    onSearch(term)
  }

  return (
    <section className="space-y-5" aria-label="YouTube videos">
      <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
        <label htmlFor="youtube-search" className="sr-only">
          Search YouTube videos
        </label>
        <Input
          id="youtube-search"
          type="search"
          value={term}
          onChange={(event) => setTerm(event.target.value)}
          placeholder="Search videos…"
          className="min-w-52 flex-1"
        />
        <Button type="submit" disabled={loading || !term.trim()}>
          <SearchIcon />
          Search
        </Button>
        <Button type="button" variant="outline" onClick={onTrending} disabled={loading}>
          <TrendingUpIcon />
          Trending
        </Button>
      </form>

      {error ? (
        <ErrorPanel
          error={error}
          title="YouTube could not be reached"
          onRetry={() => (query ? onSearch(query) : onTrending())}
        />
      ) : null}

      {loading ? <LoadingGrid count={6} /> : null}

      {!loading && !error && !searched ? (
        <EmptyState
          icon={SearchIcon}
          title="Search YouTube"
          description="Add your API key in Settings, then search or browse trending videos. Nothing is requested until you choose."
        />
      ) : null}

      {!loading && searched && videos.length === 0 ? (
        <EmptyState
          icon={SearchIcon}
          title="No videos found"
          description="Try a different search term or check your YouTube API key in Settings."
        />
      ) : null}

      {videos.length > 0 ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {videos.map((video) => (
            <a
              key={video.id}
              href={video.url}
              target="_blank"
              rel="noreferrer"
              className="group space-y-2 rounded-xl border border-border/50 bg-surface-low p-2 transition-colors hover:border-primary/50"
            >
              <div className="relative overflow-hidden rounded-lg aspect-video bg-muted">
                <img
                  src={video.thumbnail}
                  alt=""
                  loading="lazy"
                  decoding="async"
                  className="size-full object-cover transition-transform group-hover:scale-105"
                />
                {video.duration ? (
                  <span className="telemetry absolute bottom-1.5 right-1.5 rounded bg-black/75 px-1.5 py-0.5 text-[0.6rem] text-white">
                    {video.duration}
                  </span>
                ) : null}
              </div>
              <div className="space-y-0.5 px-1 pb-1">
                <p className="line-clamp-2 text-sm font-medium text-foreground group-hover:text-primary">
                  {video.title}
                </p>
                <p className="telemetry truncate text-xs text-muted-foreground">
                  {video.channel} · {video.viewsText} · {video.ageText}
                </p>
              </div>
            </a>
          ))}
        </div>
      ) : null}
    </section>
  )
}
