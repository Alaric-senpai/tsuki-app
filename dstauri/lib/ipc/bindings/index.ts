/**
 * Types generated from Rust by `ts-rs`. Do not hand-edit these files:
 * `cargo test` in `src-tauri/` regenerates them into this directory.
 *
 * The IPC wrappers in `lib/ipc/*.ts` re-export the ones they use so callers do
 * not need to know where a type came from. A few hand-written types remain
 * where the generated one is intentionally wider (e.g. `DownloadSnapshot.state`)
 * or where the Rust type has a custom `Serialize` impl (`AppError`).
 */
export type { ArchiveRepairResult } from "./ArchiveRepairResult"
export type { BulkAddResult } from "./BulkAddResult"
export type { ConsoleLine } from "./ConsoleLine"
export type { DiskUsage } from "./DiskUsage"
export type { DownloadSnapshot } from "./DownloadSnapshot"
export type { EntriesAddResult } from "./EntriesAddResult"
export type { EpisodeEntry } from "./EpisodeEntry"
export type { Episode } from "./Episode"
export type { EpisodeWithSeries } from "./EpisodeWithSeries"
export type { LibraryStats } from "./LibraryStats"
export type { SeriesPoster } from "./SeriesPoster"
export type { SeriesSummary } from "./SeriesSummary"
export type { SeriesSummaryView } from "./SeriesSummaryView"
export type { Series } from "./Series"
export type { SeriesUsage } from "./SeriesUsage"
export type { WatchProgress } from "./WatchProgress"
