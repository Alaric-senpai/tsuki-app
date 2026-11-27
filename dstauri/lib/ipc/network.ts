import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { ipcCall } from "./client"

/** Payload carried by every `network://…` event from Rust. */
export interface NetworkStatus {
  online: boolean
}

/** Cached connectivity state from Rust — cheap to call repeatedly. */
export function networkStatus(): Promise<boolean> {
  return ipcCall<boolean>("network_status", true)
}

/** Rust emits this whenever connectivity changes, plus once at startup. */
export function listenNetworkStatus(
  handler: (status: NetworkStatus) => void,
): Promise<UnlistenFn> {
  return listen<NetworkStatus>("network://status", (event) => handler(event.payload))
}

/** One-shot startup / recovery signals. Both carry the same `{ online }` payload. */
export function listenNetworkSignals(
  handler: (status: NetworkStatus) => void,
): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<NetworkStatus>("network://offline", (event) => handler(event.payload)),
    listen<NetworkStatus>("network://online", (event) => handler(event.payload)),
  ])
}
