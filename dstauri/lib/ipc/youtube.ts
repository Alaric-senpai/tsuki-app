/**
 * Typed IPC wrappers for the YouTube Data API integration.
 *
 * The Rust side owns the HTTP client and the disk cache; nothing here talks to
 * Google directly. Importing this module never calls the API — the first call
 * happens only when the user submits a search or asks for trending videos.
 */

import type { YouTubeDiscovery } from "./bindings/YouTubeDiscovery"
import type { YouTubeVideo } from "./bindings/YouTubeVideo"
import { ipcCall } from "./client"

export type { YouTubeDiscovery, YouTubeVideo }

/** Search videos. One search.list + one videos.list per request on the Rust side. */
export function youtubeSearch(query: string, maxResults = 18): Promise<YouTubeVideo[]> {
  return ipcCall<YouTubeVideo[]>("youtube_search", [], { query, maxResults })
}

/** Most-popular videos. Only called from an explicit user action. */
export function youtubeTrending(maxResults = 18): Promise<YouTubeVideo[]> {
  return ipcCall<YouTubeVideo[]>("youtube_trending", [], { maxResults })
}

/** Both discovery rails at once, keeping partial results if one rail fails. */
export function youtubeDiscover(): Promise<YouTubeDiscovery> {
  return ipcCall<YouTubeDiscovery>(
    "youtube_discover",
    { rails: [], errors: [], fetchedAt: "" },
    {},
  )
}

/** Fetch one video by id (full details). */
export function youtubeVideo(id: string): Promise<YouTubeVideo | null> {
  return ipcCall<YouTubeVideo | null>("youtube_video", null, { id })
}

/** Drop cached YouTube responses so the next request hits the API. */
export function clearYoutubeCache(): Promise<boolean> {
  return ipcCall<boolean>("youtube_clear_cache", false, {})
}
