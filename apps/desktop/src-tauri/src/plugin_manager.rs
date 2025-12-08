//! Plugin Manager for Claude Code Integration
//!
//! Manages the AgentKanban plugin installation using the official Claude CLI.
//! This is the production-ready approach that properly integrates with Claude Code.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Manages Claude Code plugin configuration
pub struct PluginManager {
    claude_dir: PathBuf,
    plugin_source_path: PathBuf,
}

impl PluginManager {
    /// Create a new PluginManager
    ///
    /// # Arguments
    /// * `plugin_source_path` - Path to the local plugin source (packages/claude-plugin)
    pub fn new(plugin_source_path: PathBuf) -> Self {
        let claude_dir = dirs::home_dir()
            .expect("Could not find home directory")
            .join(".claude");

        PluginManager {
            claude_dir,
            plugin_source_path,
        }
    }

    /// Get the default plugin source path relative to the app
    pub fn default_plugin_path() -> PathBuf {
        // In development, the plugin is at ../../../packages/claude-plugin relative to src-tauri
        // Path: apps/desktop/src-tauri -> apps/desktop -> apps -> repo root -> packages/claude-plugin
        // In production, it would be bundled with the app
        if cfg!(debug_assertions) {
            // Development: use the monorepo path
            // CARGO_MANIFEST_DIR = /path/to/agentkanban/apps/desktop/src-tauri
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent() // apps/desktop
                .unwrap()
                .parent() // apps
                .unwrap()
                .parent() // repo root
                .unwrap()
                .join("packages")
                .join("claude-plugin")
        } else {
            // Production: plugin bundled in app data directory
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("agentkanban")
                .join("claude-plugin")
        }
    }

    /// Ensure the plugin is properly installed and enabled using the Claude CLI
    pub fn ensure_plugin_installed(&self) -> Result<PluginStatus, String> {
        tracing::info!("Ensuring AgentKanban plugin is installed...");
        tracing::info!("Plugin source path: {:?}", self.plugin_source_path);

        // Verify plugin source exists
        if !self.plugin_source_path.exists() {
            return Err(format!(
                "Plugin source not found at: {:?}",
                self.plugin_source_path
            ));
        }

        // Check current status
        let current_status = self.get_plugin_status();

        match current_status {
            PluginStatus::Installed => {
                tracing::info!("Plugin already installed and enabled");
                return Ok(PluginStatus::Installed);
            }
            PluginStatus::Disabled => {
                // Plugin is installed but disabled - enable it
                tracing::info!("Plugin installed but disabled, enabling...");
                self.enable_plugin_in_settings()?;
                return Ok(PluginStatus::Installed);
            }
            _ => {
                // Need to install
                tracing::info!("Plugin not installed, installing via CLI...");
            }
        }

        // Try CLI installation first (preferred method)
        match self.install_via_cli() {
            Ok(_) => {
                tracing::info!("Plugin installed successfully via CLI");
                // Enable the plugin after installation
                self.enable_plugin_in_settings()?;
                Ok(PluginStatus::Installed)
            }
            Err(cli_err) => {
                tracing::warn!("CLI installation failed: {}. Falling back to manual registration.", cli_err);
                // Fall back to manual JSON manipulation for development
                self.install_manually()?;
                Ok(PluginStatus::Installed)
            }
        }
    }

    /// Install the plugin using the Claude CLI
    fn install_via_cli(&self) -> Result<(), String> {
        let plugin_path = self.plugin_source_path.to_string_lossy().to_string();

        tracing::info!("Running: claude /plugin install {}", plugin_path);

        let output = Command::new("claude")
            .args(["/plugin", "install", &plugin_path])
            .output()
            .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::info!("CLI output: {}", stdout);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(format!("CLI returned error: {} {}", stderr, stdout))
        }
    }

    /// Manual installation fallback - directly manipulates config files
    /// Used when CLI is not available or fails
    fn install_manually(&self) -> Result<(), String> {
        tracing::info!("Installing plugin manually...");

        // Ensure directories exist
        let plugins_dir = self.claude_dir.join("plugins");
        fs::create_dir_all(&plugins_dir)
            .map_err(|e| format!("Failed to create plugins directory: {}", e))?;

        let plugin_path_str = self.plugin_source_path.to_string_lossy().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Update known_marketplaces.json
        let marketplaces_path = plugins_dir.join("known_marketplaces.json");
        let mut marketplaces: Value = if marketplaces_path.exists() {
            let content = fs::read_to_string(&marketplaces_path).unwrap_or_else(|_| "{}".to_string());
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        marketplaces["AgentKanban"] = serde_json::json!({
            "source": {
                "source": "directory",
                "path": plugin_path_str
            },
            "installLocation": plugin_path_str,
            "lastUpdated": now
        });

        fs::write(&marketplaces_path, serde_json::to_string_pretty(&marketplaces).unwrap())
            .map_err(|e| format!("Failed to write marketplaces: {}", e))?;

        // Update installed_plugins.json
        let plugins_path = plugins_dir.join("installed_plugins.json");
        let mut plugins: Value = if plugins_path.exists() {
            let content = fs::read_to_string(&plugins_path).unwrap_or_else(|_| r#"{"version":1,"plugins":{}}"#.to_string());
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({"version": 1, "plugins": {}}))
        } else {
            serde_json::json!({"version": 1, "plugins": {}})
        };

        let version = self.get_plugin_version().unwrap_or_else(|| "0.1.0".to_string());

        plugins["plugins"]["agentkanban@AgentKanban"] = serde_json::json!({
            "version": version,
            "installedAt": now,
            "lastUpdated": now,
            "installPath": plugin_path_str,
            "isLocal": true
        });

        fs::write(&plugins_path, serde_json::to_string_pretty(&plugins).unwrap())
            .map_err(|e| format!("Failed to write plugins: {}", e))?;

        // Enable the plugin
        self.enable_plugin_in_settings()?;

        tracing::info!("Manual plugin installation complete");
        Ok(())
    }

    /// Enable the plugin in settings.json
    fn enable_plugin_in_settings(&self) -> Result<(), String> {
        let settings_path = self.claude_dir.join("settings.json");

        let mut settings: Value = if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)
                .map_err(|e| format!("Failed to read settings: {}", e))?;
            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse settings: {}", e))?
        } else {
            serde_json::json!({})
        };

        // Ensure enabledPlugins object exists
        if settings.get("enabledPlugins").is_none() {
            settings["enabledPlugins"] = serde_json::json!({});
        }

        // Enable our plugin
        settings["enabledPlugins"]["agentkanban@AgentKanban"] = serde_json::json!(true);

        // Disable any old plugin identifiers
        let old_identifiers = ["agentkanban@agentkanban", "agentkanban@agentkanban-local"];
        for old_id in old_identifiers {
            if settings["enabledPlugins"].get(old_id).is_some() {
                settings["enabledPlugins"][old_id] = serde_json::json!(false);
            }
        }

        let json_str = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        fs::write(&settings_path, json_str)
            .map_err(|e| format!("Failed to write settings: {}", e))?;

        tracing::info!("Enabled plugin in settings.json");
        Ok(())
    }

    /// Read the plugin version from plugin.json
    fn get_plugin_version(&self) -> Option<String> {
        let plugin_json_path = self.plugin_source_path.join(".claude-plugin").join("plugin.json");

        let content = fs::read_to_string(&plugin_json_path).ok()?;
        let plugin: Value = serde_json::from_str(&content).ok()?;

        plugin["version"].as_str().map(String::from)
    }

    /// Check if plugin is currently installed and enabled
    pub fn get_plugin_status(&self) -> PluginStatus {
        let settings_path = self.claude_dir.join("settings.json");

        if !settings_path.exists() {
            return PluginStatus::NotInstalled;
        }

        let content = match fs::read_to_string(&settings_path) {
            Ok(c) => c,
            Err(_) => return PluginStatus::NotInstalled,
        };

        let settings: Value = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(_) => return PluginStatus::NotInstalled,
        };

        // Check for any known plugin identifier
        let plugin_identifiers = [
            "agentkanban@AgentKanban",
            "agentkanban@agentkanban-local",
            "agentkanban@agentkanban",
        ];

        for identifier in plugin_identifiers {
            if let Some(enabled) = settings["enabledPlugins"].get(identifier) {
                if enabled.as_bool() == Some(true) {
                    return PluginStatus::Installed;
                } else if enabled.as_bool() == Some(false) {
                    return PluginStatus::Disabled;
                }
            }
        }

        // Check if plugin exists in installed_plugins.json but not enabled
        let plugins_path = self.claude_dir.join("plugins").join("installed_plugins.json");
        if plugins_path.exists() {
            if let Ok(content) = fs::read_to_string(&plugins_path) {
                if let Ok(plugins) = serde_json::from_str::<Value>(&content) {
                    for identifier in plugin_identifiers {
                        if plugins["plugins"].get(identifier).is_some() {
                            return PluginStatus::Disabled;
                        }
                    }
                }
            }
        }

        PluginStatus::NotInstalled
    }
}

/// Status of the plugin installation
#[derive(Debug, Clone, PartialEq)]
pub enum PluginStatus {
    /// Plugin is installed from local source and enabled
    Installed,
    /// Plugin is installed but from GitHub (out of sync)
    OutOfSync,
    /// Plugin is installed but disabled
    Disabled,
    /// Plugin is not installed
    NotInstalled,
}

impl std::fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginStatus::Installed => write!(f, "installed"),
            PluginStatus::OutOfSync => write!(f, "out_of_sync"),
            PluginStatus::Disabled => write!(f, "disabled"),
            PluginStatus::NotInstalled => write!(f, "not_installed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_plugin_path() {
        let path = PluginManager::default_plugin_path();
        assert!(path.to_string_lossy().contains("claude-plugin"));
    }
}
