mod commands;
mod config;
mod db;
mod downloader;
mod error;
mod events;
mod media_ids;
mod migrate;
mod sources;
mod state;

use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_opener::OpenerExt;

use crate::config::ConfigStore;
use crate::db::Db;
use crate::downloader::DownloadManager;
use crate::error::AppError;
use crate::state::AppState;

const DATABASE_FILE: &str = "tsuki.db";

fn external_navigation_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("external-navigation")
        .on_navigation(|webview, url| {
            let is_internal_host = matches!(
                url.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("tauri.localhost") | Some("::1")
            );

            let is_internal = url.scheme() == "tauri" || is_internal_host;

            if is_internal {
                return true;
            }

            let is_external_link = matches!(url.scheme(), "http" | "https" | "mailto" | "tel");

            if is_external_link {
                log::info!("opening external link in system browser: {}", url);
                let _ = webview.opener().open_url(url.as_str(), None::<&str>);
                return false;
            }

            true
        })
        .build()
}

/// Tray icon so downloads stay reachable while the window is hidden.
/// Linux needs an appindicator implementation at runtime; when the platform
/// refuses a tray we log it and keep the app fully usable.
fn install_tray(app: &tauri::App) -> Result<(), AppError> {
    let show = MenuItem::with_id(app, "show", "Open Tsuki", true, None::<&str>)
        .map_err(|err| AppError::io_with("Could not build the tray menu.", err.to_string()))?;
    let downloads = MenuItem::with_id(app, "downloads", "Download centre", true, None::<&str>)
        .map_err(|err| AppError::io_with("Could not build the tray menu.", err.to_string()))?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|err| AppError::io_with("Could not build the tray menu.", err.to_string()))?;
    let menu = Menu::with_items(app, &[&show, &downloads, &quit])
        .map_err(|err| AppError::io_with("Could not build the tray menu.", err.to_string()))?;

    let builder = TrayIconBuilder::with_id(downloader::TRAY_ID)
        .tooltip("Tsuki — idle")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "downloads" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                    let _ = window.eval("window.location.assign('/downloads')");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        });

    let builder = match app.default_window_icon() {
        Some(icon) => builder.icon(icon.clone()),
        None => builder,
    };

    match builder.build(app) {
        Ok(_) => log::info!("tray icon ready"),
        Err(err) => log::warn!("tray icon unavailable on this platform: {err}"),
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch hands its argv to the
        // running instance instead of opening a duplicate window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("sqlx", log::LevelFilter::Error)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(external_navigation_plugin())
        .setup(|app| {
            let config_dir = app.path().app_config_dir().map_err(|err| {
                AppError::io_with(
                    "Could not resolve the configuration directory.",
                    err.to_string(),
                )
            })?;
            let store = ConfigStore::new(config_dir);
            let mut current = store.load_or_create()?;
            log::info!("configuration ready at {}", store.path().display());

            let data_dir = app.path().app_data_dir().map_err(|err| {
                AppError::io_with(
                    "Could not resolve the application data directory.",
                    err.to_string(),
                )
            })?;
            let db_path = data_dir.join(DATABASE_FILE);
            let db = tauri::async_runtime::block_on(Db::connect(&db_path))?;

            tauri::async_runtime::block_on(async {
                let repair = db::queries::repair_archives(db.pool()).await?;
                log::info!(
                    "archive repair: {} folder(s) scanned, {} archive(s) created",
                    repair.folders_scanned,
                    repair.archives_created
                );
                let changed = db::queries::sync_all_archives(db.pool()).await?;
                log::info!("archive sync updated {changed} episode row(s)");
                Ok::<_, AppError>(())
            })?;

            tauri::async_runtime::block_on(migrate::run_pending_migration(
                db.pool(),
                &store,
                &mut current,
                &data_dir,
            ))?;

            // Explore caches live beside the other caches, never in the DB.
            let cache_root = app
                .path()
                .app_cache_dir()
                .unwrap_or_else(|_| data_dir.join("cache"));
            sources::cache::init(cache_root.join("explore"));

            install_tray(app)?;

            let concurrency = current
                .settings
                .get("concurrent_downloads")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(3)
                .clamp(1, 16);
            let downloads = DownloadManager::new(
                db.pool().clone(),
                app.handle().clone(),
                data_dir.clone(),
                concurrency,
            );

            app.manage(AppState::new(
                store,
                current,
                db,
                data_dir.clone(),
                downloads,
            ));

            // The queue lives in memory, so reinstate anything the catalog
            // still marks as queued from the previous session.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let Some(state) = handle.try_state::<AppState>() else {
                    return;
                };
                if let Err(error) = commands::downloads::resume_queued(&state).await {
                    log::warn!("could not resume the download queue: {}", error.message());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::set_setting,
            commands::config::get_config_path,
            commands::config::set_player_config,
            commands::config::set_explore_config,
            commands::config::set_youtube_config,
            commands::player::resolve_playable,
            commands::config::open_in_external_player,
            commands::library::list_series,
            commands::library::get_series,
            commands::library::update_series_details,
            commands::library::set_series_banner,
            commands::library::resync_remote_banners,
            commands::library::set_series_banner_from_url,
            commands::library::resync_series_banners,
            commands::library::list_episodes,
            commands::library::recent_episodes,
            commands::library::series_posters,
            commands::library::library_stats,
            commands::library::sync_archives,
            commands::library::pick_banner_file,
            commands::library::create_episode,
            commands::library::update_episode,
            commands::library::delete_episode,
            commands::migrate::import_legacy_json,
            commands::legacy::legacy_status,
            commands::legacy::pull_legacy_data,
            commands::migrate::legacy_export_path,
            commands::downloads::enqueue_episode_download,
            commands::downloads::cancel_episode_download,
            commands::downloads::pause_episode_download,
            commands::downloads::resume_episode_download,
            commands::downloads::move_download_in_queue,
            commands::downloads::pause_downloads,
            commands::downloads::downloads_paused,
            commands::downloads::download_queue,
            commands::downloads::clear_download_queue,
            commands::downloads::resume_downloads,
            commands::downloads::remove_from_queue,
            commands::downloads::download_state,
            commands::downloads::set_download_concurrency,
            commands::downloads::downloaded_episode_path,
            commands::explore::explore_sources,
            commands::explore::explore_filters,
            commands::explore::explore_catalog,
            commands::explore::explore_series,
            commands::explore::import_source_series,
            commands::updates::check_series_updates,
            commands::updates::set_series_check_updates,
            commands::watch_progress::save_watch_progress,
            commands::watch_progress::get_watch_progress,
            commands::watch_progress::clear_watch_progress,
            commands::watch_progress::all_watch_progress,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            log::error!("failed to run the Tsuki application: {err}");
            std::process::exit(1);
        });
}
