//! Thin `#[tauri::command]` handlers, one module per domain.

pub mod config;
pub mod downloads;
pub mod explore;
pub mod legacy;
pub mod library;
pub mod migrate;
pub mod player;
pub mod updates;
pub mod watch_progress;
pub mod youtube;
