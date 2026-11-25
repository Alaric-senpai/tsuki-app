import { ipcCall } from "./client"

/** A playable media target resolved from a source episode page at play time. */
export interface PlayableSource {
  url: string
  title: string
  source: string
  platform: string
  isHls: boolean
  /** True when the URL was passed through without source resolution. */
  direct: boolean
}

export const EMPTY_PLAYABLE: PlayableSource = {
  url: "",
  title: "",
  source: "",
  platform: "",
  isHls: false,
  direct: false,
}

/** Resolve an episode page URL into a transient direct media target. */
export function resolvePlayable(pageUrl: string): Promise<PlayableSource> {
  return ipcCall<PlayableSource>(
    "resolve_playable",
    { ...EMPTY_PLAYABLE, url: pageUrl, direct: true },
    { pageUrl },
  )
}
