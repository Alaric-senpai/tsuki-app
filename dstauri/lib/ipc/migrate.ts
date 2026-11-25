import { ipcCall } from "./client"

export interface ImportSummary {
  seriesSeen: number
  seriesCreated: number
  episodesSeen: number
  episodesAdded: number
  episodesUpdated: number
}

export function legacyExportPath(): Promise<string> {
  return ipcCall<string>("legacy_export_path", "Browser preview")
}

export function importLegacyJson(path: string): Promise<ImportSummary> {
  return ipcCall<ImportSummary>("import_legacy_json", {
    seriesSeen: 0,
    seriesCreated: 0,
    episodesSeen: 0,
    episodesAdded: 0,
    episodesUpdated: 0,
  }, { path })
}
