import { ipcCall } from "./client"

import type { ImportSummary } from "./migrate"

/** What the Settings "Legacy" section needs to decide whether to show itself. */
export interface LegacyStatus {
  /** A legacy config file or executable was found on this machine. */
  detected: boolean
  configPath: string | null
  /** Where a staged export is read from / written to. */
  exportPath: string
  /** A staged export file already exists at `exportPath`. */
  exportReady: boolean
  seriesCount: number
  episodeCount: number
  /** Legacy executables found on `PATH`. */
  binaries: string[]
  /** The `export_legacy.py` helper, if one was found. */
  exportScript: string | null
  imported: boolean
  importedAt: string | null
  /** Human-readable explanation of what can be done next. */
  note: string
}

export interface PullSummary {
  /** False when no export script was available, so nothing was run. */
  exported: boolean
  path: string
  /** Captured stdout/stderr from the export script. */
  log: string
  summary: ImportSummary
}

const EMPTY_STATUS: LegacyStatus = {
  detected: false,
  configPath: null,
  exportPath: "",
  exportReady: false,
  seriesCount: 0,
  episodeCount: 0,
  binaries: [],
  exportScript: null,
  imported: false,
  importedAt: null,
  note: "Browser preview: legacy import is unavailable.",
}

const EMPTY_PULL: PullSummary = {
  exported: false,
  path: "",
  log: "",
  summary: {
    seriesSeen: 0,
    seriesCreated: 0,
    episodesSeen: 0,
    episodesAdded: 0,
    episodesUpdated: 0,
  },
}

/** Detect an installed donghua-stream GUI/CLI and its staged export. */
export function legacyStatus(): Promise<LegacyStatus> {
  return ipcCall<LegacyStatus>("legacy_status", EMPTY_STATUS)
}

/** Run the legacy export helper (when available) and import the result. */
export function pullLegacyData(): Promise<PullSummary> {
  return ipcCall<PullSummary>("pull_legacy_data", EMPTY_PULL)
}
