import type { InvokeArgs } from "@tauri-apps/api/core"

import { isTauri, trackedInvoke } from "@/lib/tauri"

export interface AppIpcError {
  code: string
  message: string
  detail?: string
}

export function toAppIpcError(error: unknown): AppIpcError {
  if (typeof error === "object" && error !== null) {
    const candidate = error as Record<string, unknown>
    const code = typeof candidate.code === "string" ? candidate.code : "unknown"
    const message =
      typeof candidate.message === "string"
        ? candidate.message
        : "Tsuki could not complete that request."
    const detail =
      typeof candidate.detail === "string"
        ? candidate.detail
        : typeof candidate.detail === "undefined"
          ? undefined
          : String(candidate.detail)
    return { code, message, detail }
  }

  return {
    code: "unknown",
    message: error instanceof Error ? error.message : "Tsuki could not complete that request.",
    detail: error instanceof Error ? error.stack : undefined,
  }
}

/**
 * Browser preview mode has no Rust backend. It returns an explicit fallback so
 * static `next dev` previews never throw; real Tauri sessions always call
 * `trackedInvoke` and surface typed AppIpcError failures to the caller.
 */
export async function ipcCall<T>(
  command: string,
  fallback: T,
  args?: InvokeArgs,
): Promise<T> {
  if (!isTauri()) {
    return fallback
  }

  try {
    return await trackedInvoke<T>(command, args)
  } catch (error) {
    throw toAppIpcError(error)
  }
}
