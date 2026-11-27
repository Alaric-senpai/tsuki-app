//! App-wide events the frontend listens to.
//!
//! Progress-style streams have their own namespaces (`download://…`). This
//! module holds the coarse "something changed, re-read it" signals so the UI
//! can stay in sync without polling.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Emitted whenever the catalog changed in a way the UI should re-read.
pub const LIBRARY_CHANGED: &str = "library://changed";

/// Emitted on startup (when automatic checks are enabled) and after a manual
/// check finds a newer public release. Carries an `AppRelease` payload.
pub const UPDATE_AVAILABLE: &str = "update://available";

/// Fired on every connectivity change, and once at startup with the seed value.
pub const NETWORK_STATUS: &str = "network://status";
/// Fired at startup / on a watcher transition when connectivity was lost.
pub const NETWORK_OFFLINE: &str = "network://offline";
/// Fired at startup / on a watcher transition when connectivity returned.
pub const NETWORK_ONLINE: &str = "network://online";

/// Payload shared by all three `network://…` events.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub online: bool,
}

pub fn library_changed(app: &AppHandle) {
    if let Err(error) = app.emit(LIBRARY_CHANGED, ()) {
        log::warn!("could not emit {LIBRARY_CHANGED}: {error}");
    }
}

fn network_event(app: &AppHandle, name: &str, online: bool) {
    if let Err(error) = app.emit(name, NetworkStatus { online }) {
        log::warn!("could not emit {name}: {error}");
    }
}

pub fn network_status(app: &AppHandle, online: bool) {
    network_event(app, NETWORK_STATUS, online);
}

pub fn network_offline(app: &AppHandle) {
    network_event(app, NETWORK_OFFLINE, false);
}

pub fn network_online(app: &AppHandle) {
    network_event(app, NETWORK_ONLINE, true);
}
