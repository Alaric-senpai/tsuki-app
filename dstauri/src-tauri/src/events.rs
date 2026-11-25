//! App-wide events the frontend listens to.
//!
//! Progress-style streams have their own namespaces (`download://…`). This
//! module holds the coarse "something changed, re-read it" signals so the UI
//! can stay in sync without polling.

use tauri::{AppHandle, Emitter};

/// Emitted whenever the catalog changed in a way the UI should re-read.
pub const LIBRARY_CHANGED: &str = "library://changed";

pub fn library_changed(app: &AppHandle) {
    if let Err(error) = app.emit(LIBRARY_CHANGED, ()) {
        log::warn!("could not emit {LIBRARY_CHANGED}: {error}");
    }
}
