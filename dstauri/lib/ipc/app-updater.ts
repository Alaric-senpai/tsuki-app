import { ipcCall } from "./client"

export type { AppRelease } from "./bindings/AppRelease"

import type { AppRelease } from "./bindings/AppRelease"

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
