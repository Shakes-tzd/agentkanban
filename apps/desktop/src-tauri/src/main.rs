#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod graph_db;
mod plugin_manager;
mod server;
mod watcher;
mod workflow_service;

use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};
use tokio::sync::broadcast;
use tracing_subscriber;

/// State wrapper for the graph database connection
pub struct GraphDbState(pub Arc<graph_db::GraphDb>);

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Event channel for real-time updates
    let (event_tx, _) = broadcast::channel::<db::AgentEvent>(100);
    let event_tx = Arc::new(event_tx);

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            let _event_tx_clone = Arc::clone(&event_tx);

            // Migrate from legacy AgentKanban location if needed
            if let Err(e) = db::migrate_from_legacy() {
                tracing::warn!("Database migration warning: {}", e);
            }

            // Initialize database at ~/.ijoka/ijoka.db
            // This path is shared with Claude Code hooks for single source of truth
            let db_path = db::get_standard_db_path();

            // Ensure parent directory exists
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            tracing::info!("Using shared database at {:?}", db_path);
            let database = db::Database::new(&db_path)?;
            app.manage(db::DbState(Arc::new(database)));

            // Initialize graph database (Memgraph) connection
            // This is optional - the app works without it, using SQLite as fallback
            let graph_db = Arc::new(graph_db::GraphDb::new());
            app.manage(GraphDbState(Arc::clone(&graph_db)));

            // Try to connect to Memgraph in background (non-blocking)
            let graph_handle = app.handle().clone();
            let sync_graph_db = Arc::clone(&graph_db);
            let connect_graph_db = Arc::clone(&graph_db);
            tauri::async_runtime::spawn(async move {
                match connect_graph_db.connect().await {
                    Ok(_) => {
                        tracing::info!("Connected to Memgraph graph database");
                        let _ = graph_handle.emit("graph-db-connected", true);

                        // Sync graph data to SQLite cache immediately after connection
                        if let Some(db_state) = graph_handle.try_state::<db::DbState>() {
                            match sync_graph_to_sqlite(&sync_graph_db, &db_state.0).await {
                                Ok(count) => {
                                    tracing::info!("Initial graph->SQLite sync: {} features synced", count);
                                    let _ = graph_handle.emit("cache-synced", count);
                                }
                                Err(e) => {
                                    tracing::warn!("Initial sync failed: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Graph database not available: {}. Using SQLite-only mode.",
                            e
                        );
                        let _ = graph_handle.emit("graph-db-connected", false);
                    }
                }
            });

            // Setup Claude Code plugin
            let plugin_path = plugin_manager::PluginManager::default_plugin_path();
            let pm = plugin_manager::PluginManager::new(plugin_path);
            match pm.ensure_plugin_installed() {
                Ok(status) => {
                    tracing::info!("Plugin status: {:?}", status);
                }
                Err(e) => {
                    tracing::warn!("Plugin setup warning: {}. Plugin features may be limited.", e);
                }
            }

            // Start file watcher in background thread
            let watcher_handle = handle.clone();
            let watcher_tx = Arc::clone(&event_tx);
            std::thread::spawn(move || {
                if let Err(e) = watcher::start_watching(watcher_handle, watcher_tx) {
                    tracing::error!("File watcher error: {}", e);
                }
            });

            // Start HTTP server for hook events
            let http_handle = handle.clone();
            let http_tx = Arc::clone(&event_tx);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = server::start_server(http_handle, http_tx, 4000).await {
                    tracing::error!("HTTP server error: {}", e);
                }
            });

            // Forward events to frontend via Tauri events
            let event_handle = handle.clone();
            let mut event_rx = event_tx.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    let _ = event_handle.emit("agent-event", &event);
                }
            });

            // Cleanup stale sessions on startup and periodically (every 2 minutes)
            let cleanup_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                // Run cleanup immediately on startup
                if let Some(db_state) = cleanup_handle.try_state::<db::DbState>() {
                    if let Ok(count) = db_state.0.cleanup_stale_sessions(15) {
                        if count > 0 {
                            tracing::info!("Startup cleanup: marked {} stale sessions as ended", count);
                        }
                    }
                }

                // Then run periodically (every 2 minutes)
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
                loop {
                    interval.tick().await;
                    if let Some(db_state) = cleanup_handle.try_state::<db::DbState>() {
                        match db_state.0.cleanup_stale_sessions(15) {
                            Ok(count) if count > 0 => {
                                // Notify frontend to refresh sessions
                                let _ = cleanup_handle.emit("sessions-updated", ());
                            }
                            Err(e) => {
                                tracing::error!("Failed to cleanup stale sessions: {}", e);
                            }
                            _ => {}
                        }
                    }
                }
            });

            // Periodic Memgraph → SQLite sync (every 5 seconds)
            // This ensures UI stays updated when hooks write to Memgraph
            let sync_handle = handle.clone();
            let periodic_graph_db = Arc::clone(&graph_db);
            tauri::async_runtime::spawn(async move {
                // Wait a bit for initial connection
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                loop {
                    interval.tick().await;

                    // Only sync if graph DB is connected
                    if !periodic_graph_db.is_connected().await {
                        continue;
                    }

                    if let Some(db_state) = sync_handle.try_state::<db::DbState>() {
                        match sync_graph_to_sqlite(&periodic_graph_db, &db_state.0).await {
                            Ok(count) if count > 0 => {
                                tracing::debug!("Graph->SQLite sync: {} features", count);
                                // Notify frontend to refresh data
                                let _ = sync_handle.emit("features-updated", count);
                            }
                            Err(e) => {
                                tracing::debug!("Graph sync skipped: {}", e);
                            }
                            _ => {}
                        }
                    }
                }
            });

            // Setup system tray
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
            let scan = MenuItem::with_id(app, "scan", "Scan Projects", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &scan, &quit])?;

            let _tray = TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
                .menu(&menu)
                .tooltip("Ijoka")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "scan" => {
                        let _ = app.emit("scan-requested", ());
                    }
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
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_features,
            commands::get_feature,
            commands::get_events,
            commands::get_feature_events,
            commands::get_sessions,
            commands::get_stats,
            commands::get_projects,
            commands::scan_projects,
            commands::watch_project,
            commands::get_config,
            commands::save_config,
            commands::get_plugin_status,
            commands::install_plugin,
            commands::get_plugin_path,
            commands::install_integration,
            commands::update_feature,
            // Graph database commands
            commands::get_graph_db_status,
            commands::get_graph_projects,
            commands::get_graph_features,
            commands::get_graph_active_feature,
            commands::get_graph_project_stats,
            commands::sync_graph_to_cache,
        ])
        .on_window_event(|window, event| {
            // Minimize to tray instead of closing
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Sync features from Graph DB to SQLite cache
/// Returns the number of features synced
async fn sync_graph_to_sqlite(
    graph_db: &graph_db::GraphDb,
    sqlite_db: &db::Database,
) -> Result<usize, String> {
    if !graph_db.is_connected().await {
        return Err("Graph database not connected".to_string());
    }

    let projects = graph_db
        .get_projects()
        .await
        .map_err(|e| e.to_string())?;

    let mut synced_count = 0;

    for project in &projects {
        let features = graph_db
            .get_features_for_project(&project.path)
            .await
            .map_err(|e| e.to_string())?;

        for feature in features {
            if let Some(id) = &feature.id {
                let sync_feature = db::GraphFeatureSync {
                    id: id.clone(),
                    project_dir: project.path.clone(),
                    description: feature.description.clone(),
                    category: feature.category.clone(),
                    status: feature.status.clone(),
                    steps: feature.steps.clone().unwrap_or_default(),
                };

                if sqlite_db.sync_feature_from_graph(&sync_feature).is_ok() {
                    synced_count += 1;
                }
            }
        }
    }

    Ok(synced_count)
}
