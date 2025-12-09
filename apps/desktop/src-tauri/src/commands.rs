use crate::db::{Config, DbState, Feature, FeatureUpdate, GraphFeatureSync, UpdateSource, AgentEvent, Session, Stats};
use crate::graph_db;
use crate::plugin_manager::PluginManager;
use crate::GraphDbState;
use serde::Serialize;
use tauri::State;

#[tauri::command]
pub async fn get_features(
    db: State<'_, DbState>,
    project_dir: Option<String>,
) -> Result<Vec<Feature>, String> {
    db.0.get_features(project_dir.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_events(
    db: State<'_, DbState>,
    limit: Option<i64>,
) -> Result<Vec<AgentEvent>, String> {
    db.0.get_events(limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_feature_events(
    db: State<'_, DbState>,
    feature_id: String,
    limit: Option<i64>,
) -> Result<Vec<AgentEvent>, String> {
    db.0.get_events_by_feature(&feature_id, limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_sessions(db: State<'_, DbState>) -> Result<Vec<Session>, String> {
    db.0.get_sessions().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stats(db: State<'_, DbState>) -> Result<Stats, String> {
    db.0.get_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_projects(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    db.0.get_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_projects() -> Result<Vec<String>, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let mut projects = vec![];

    // Common project locations
    let search_dirs = vec![
        home.join("projects"),
        home.join("code"),
        home.join("dev"),
        home.join("workspace"),
        home.join("Documents/projects"),
    ];

    for search_dir in search_dirs {
        if !search_dir.exists() {
            continue;
        }

        // Look for feature_list.json files
        let pattern = format!("{}/**/feature_list.json", search_dir.display());
        if let Ok(paths) = glob::glob(&pattern) {
            for entry in paths.flatten() {
                if let Some(parent) = entry.parent() {
                    projects.push(parent.to_string_lossy().to_string());
                }
            }
        }
    }

    // Also check Claude projects directory for recent projects
    let claude_projects = home.join(".claude/projects");
    if claude_projects.exists() {
        if let Ok(entries) = std::fs::read_dir(&claude_projects) {
            for entry in entries.flatten() {
                // Claude encodes project paths - we'd need to decode them
                // For now, just note that there are Claude projects
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    // Decode the project path (it's typically URL-encoded or similar)
                    // This is a simplified version
                    if let Ok(decoded) = urlencoding::decode(&name) {
                        let path = decoded.to_string();
                        if std::path::Path::new(&path).exists() && !projects.contains(&path) {
                            projects.push(path);
                        }
                    }
                }
            }
        }
    }

    projects.sort();
    projects.dedup();

    Ok(projects)
}

#[tauri::command]
pub async fn watch_project(
    db: State<'_, DbState>,
    project_dir: String,
) -> Result<(), String> {
    let mut config = db.0.get_config().map_err(|e| e.to_string())?;

    if !config.watched_projects.contains(&project_dir) {
        config.watched_projects.push(project_dir);
        db.0.save_config(&config).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_config(db: State<'_, DbState>) -> Result<Config, String> {
    db.0.get_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_config(db: State<'_, DbState>, config: Config) -> Result<(), String> {
    db.0.save_config(&config).map_err(|e| e.to_string())
}

// Plugin management commands

#[tauri::command]
pub async fn get_plugin_status() -> Result<String, String> {
    let plugin_path = PluginManager::default_plugin_path();
    let pm = PluginManager::new(plugin_path);
    Ok(pm.get_plugin_status().to_string())
}

#[tauri::command]
pub async fn install_plugin() -> Result<String, String> {
    let plugin_path = PluginManager::default_plugin_path();
    let pm = PluginManager::new(plugin_path);

    match pm.ensure_plugin_installed() {
        Ok(status) => Ok(format!("Plugin installed successfully: {:?}", status)),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_plugin_path() -> Result<String, String> {
    let path = PluginManager::default_plugin_path();
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn install_integration(project_dir: String) -> Result<String, String> {
    crate::workflow_service::WorkflowService::install_antigravity_integration(&project_dir)
        .map_err(|e| e.to_string())?;
    Ok("Integration installed successfully".to_string())
}

/// Update a feature with source-aware override logic
/// source: "human" for UI interactions, "agent" for programmatic updates
#[tauri::command]
pub async fn update_feature(
    db: State<'_, DbState>,
    feature_id: String,
    update: FeatureUpdate,
    source: String,
) -> Result<bool, String> {
    let update_source = match source.as_str() {
        "human" => UpdateSource::Human,
        "agent" => UpdateSource::Agent,
        _ => return Err("Invalid source: must be 'human' or 'agent'".to_string()),
    };

    db.0.update_feature(&feature_id, update, update_source)
        .map_err(|e| e.to_string())
}

/// Get a single feature by ID
#[tauri::command]
pub async fn get_feature(
    db: State<'_, DbState>,
    feature_id: String,
) -> Result<Option<Feature>, String> {
    db.0.get_feature(&feature_id)
        .map_err(|e| e.to_string())
}

// =============================================================================
// GRAPH DATABASE COMMANDS
// =============================================================================

#[derive(Serialize)]
pub struct GraphDbStatus {
    pub connected: bool,
    pub uri: String,
}

/// Check if graph database is connected
#[tauri::command]
pub async fn get_graph_db_status(
    graph_db: State<'_, GraphDbState>,
) -> Result<GraphDbStatus, String> {
    let connected = graph_db.0.is_connected().await;
    Ok(GraphDbStatus {
        connected,
        uri: std::env::var("IJOKA_GRAPH_URI")
            .unwrap_or_else(|_| "bolt://localhost:7687".to_string()),
    })
}

/// Get projects from graph database
#[tauri::command]
pub async fn get_graph_projects(
    graph_db: State<'_, GraphDbState>,
) -> Result<Vec<graph_db::Project>, String> {
    graph_db
        .0
        .get_projects()
        .await
        .map_err(|e| e.to_string())
}

/// Get features for a project from graph database
#[tauri::command]
pub async fn get_graph_features(
    graph_db: State<'_, GraphDbState>,
    project_path: String,
) -> Result<Vec<graph_db::Feature>, String> {
    graph_db
        .0
        .get_features_for_project(&project_path)
        .await
        .map_err(|e| e.to_string())
}

/// Get active feature for a project from graph database
#[tauri::command]
pub async fn get_graph_active_feature(
    graph_db: State<'_, GraphDbState>,
    project_path: String,
) -> Result<Option<graph_db::Feature>, String> {
    graph_db
        .0
        .get_active_feature(&project_path)
        .await
        .map_err(|e| e.to_string())
}

/// Get project statistics from graph database
#[tauri::command]
pub async fn get_graph_project_stats(
    graph_db: State<'_, GraphDbState>,
    project_path: String,
) -> Result<graph_db::ProjectStats, String> {
    graph_db
        .0
        .get_project_stats(&project_path)
        .await
        .map_err(|e| e.to_string())
}

/// Sync projects from graph to SQLite cache
#[tauri::command]
pub async fn sync_graph_to_cache(
    graph_db: State<'_, GraphDbState>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    if !graph_db.0.is_connected().await {
        return Err("Graph database not connected".to_string());
    }

    // Get all projects from graph
    let projects = graph_db
        .0
        .get_projects()
        .await
        .map_err(|e| e.to_string())?;

    let mut synced_features = 0;

    for project in &projects {
        // Get features for each project
        let features = graph_db
            .0
            .get_features_for_project(&project.path)
            .await
            .map_err(|e| e.to_string())?;

        // Sync each feature to SQLite cache
        for feature in features {
            // Convert graph_db::Feature to GraphFeatureSync for SQLite upsert
            if let Some(id) = &feature.id {
                let sync_feature = GraphFeatureSync {
                    id: id.clone(),
                    project_dir: project.path.clone(),
                    description: feature.description.clone(),
                    category: feature.category.clone(),
                    status: feature.status.clone(),
                    steps: feature.steps.clone().unwrap_or_default(),
                };

                // Attempt to insert/update the feature
                if let Err(e) = db.0.sync_feature_from_graph(&sync_feature) {
                    tracing::warn!("Failed to sync feature {}: {}", id, e);
                } else {
                    synced_features += 1;
                }
            }
        }
    }

    Ok(format!(
        "Synced {} features from {} projects",
        synced_features,
        projects.len()
    ))
}
