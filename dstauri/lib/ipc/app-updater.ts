import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { ipcCall } from "./client"

export type { AppRelease } from "./bindings/AppRelease"

import type { AppRelease } from "./bindings/AppRelease"

/** The startup event also carries a cached installer path when auto-download ran. */
export type AvailableUpdate = AppRelease & { downloadedPath?: string | null }

/** Ask Rust whether a newer public release exists for this platform. */
export function checkAppUpdate(): Promise<AppRelease | null> {
  return ipcCall<AppRelease | null>("check_app_update", null)
}

/**
 * Download the matching installer (`.rpm` on Fedora, `.deb` on Debian/Ubuntu,
 * AppImage otherwise) and open it for installation.
 */
export function installAppUpdate(): Promise<string> {
  return ipcCall<string>("install_app_update", "")
}

/** Save the opt-in/opt-out flags shown in the update dialog and Settings. */
export function setAppUpdateSettings(
  autoCheck: boolean,
  autoDownload: boolean,
): Promise<void> {
  return ipcCall<void>("set_app_update_settings", undefined, {
    autoCheck,
    autoDownload,
  })
}

/** Download (unless already cached) and open the installer for this release. */
export function startAppUpdate(release: AvailableUpdate): Promise<string> {
  return ipcCall<string>("start_app_update", "", { release })
}

/** Rust emits this on startup when automatic checks find a newer release. */
export function listenUpdateAvailable(
  handler: (release: AvailableUpdate) => void,
): Promise<UnlistenFn> {
  return listen<AvailableUpdate>("update://available", (event) => handler(event.payload))
}
