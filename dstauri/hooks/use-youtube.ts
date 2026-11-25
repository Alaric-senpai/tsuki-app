"use client"

import { useCallback, useState } from "react"

import { toAppIpcError, type AppIpcError } from "@/lib/ipc/client"
import {
  youtubeSearch,
  youtubeTrending,
  type YouTubeVideo,
} from "@/lib/ipc/youtube"

/**
 * YouTube Explore state. The API is never called on mount: the first request
 * happens when the user submits a search or clicks "Trending".
 */
export function useYouTubeExplore() {
  const [videos, setVideos] = useState<YouTubeVideo[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<AppIpcError | null>(null)
  const [searched, setSearched] = useState(false)
  const [query, setQuery] = useState("")

  const search = useCallback(async (term: string) => {
    const clean = term.trim()
    if (!clean) return
    setLoading(true)
    setError(null)
    setQuery(clean)
    try {
      setVideos(await youtubeSearch(clean))
      setSearched(true)
    } catch (cause) {
      setError(toAppIpcError(cause))
    } finally {
      setLoading(false)
    }
  }, [])

  const trending = useCallback(async () => {
    setLoading(true)
    setError(null)
    setQuery("")
    try {
      setVideos(await youtubeTrending())
      setSearched(true)
    } catch (cause) {
      setError(toAppIpcError(cause))
    } finally {
      setLoading(false)
    }
  }, [])

  return { videos, loading, error, searched, query, search, trending }
}
