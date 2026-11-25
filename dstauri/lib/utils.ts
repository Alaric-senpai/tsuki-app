export { cn } from "cn"

/** Builds a safe CSS `url()` value for cached artwork paths or data URLs. */
export function backgroundImageUrl(value: string | null | undefined): string | undefined {
  if (!value) {
    return undefined
  }
  return `url("${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}")`
}

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const

/** Human-readable byte count (1024-based). */
export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B"
  }
  const exponent = Math.min(
    Math.floor(Math.log(value) / Math.log(1024)),
    BYTE_UNITS.length - 1,
  )
  const scaled = value / 1024 ** exponent
  const digits = exponent === 0 || scaled >= 10 ? 0 : 1
  return `${scaled.toFixed(digits)} ${BYTE_UNITS[exponent]}`
}

/** Transfer rate, e.g. `1.4 MB/s`. */
export function formatSpeed(bytesPerSecond: number): string {
  return bytesPerSecond > 0 ? `${formatBytes(bytesPerSecond)}/s` : "—"
}

/** Playback clock, e.g. `4:12` or `1:04:12`. */
export function formatClock(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return "0:00"
  }
  const total = Math.floor(seconds)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const rest = total % 60
  const paddedRest = String(rest).padStart(2, "0")
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${paddedRest}`
  }
  return `${minutes}:${paddedRest}`
}

/** Remaining time, e.g. `4m 12s`. */
export function formatEta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return "—"
  }
  const total = Math.round(seconds)
  const minutes = Math.floor(total / 60)
  const rest = total % 60
  return minutes > 0 ? `${minutes}m ${rest}s` : `${rest}s`
}
