use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct Database {
    conn: Mutex<Connection>,
}

pub struct DbState(pub Arc<Database>);

/// Get the standard database path: ~/.ijoka/ijoka.db
/// This is shared between Tauri app and Claude Code hooks
pub fn get_standard_db_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".ijoka")
        .join("ijoka.db")
}

/// Migrate database from legacy AgentKanban location if needed
pub fn migrate_from_legacy() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let legacy_dir = home.join(".agentkanban");
    let legacy_db = legacy_dir.join("agentkanban.db");
    let new_dir = home.join(".ijoka");
    let new_db = new_dir.join("ijoka.db");

    // Only migrate if legacy exists and new doesn't
    if legacy_db.exists() && !new_db.exists() {
        tracing::info!("Migrating database from AgentKanban to Ijoka...");

        // Create new directory
        std::fs::create_dir_all(&new_dir)?;

        // Copy database file
        std::fs::copy(&legacy_db, &new_db)?;

        // Copy WAL and SHM files if they exist
        let legacy_wal = legacy_dir.join("agentkanban.db-wal");
        let legacy_shm = legacy_dir.join("agentkanban.db-shm");
        if legacy_wal.exists() {
            std::fs::copy(&legacy_wal, new_dir.join("ijoka.db-wal"))?;
        }
        if legacy_shm.exists() {
            std::fs::copy(&legacy_shm, new_dir.join("ijoka.db-shm"))?;
        }

        tracing::info!("Migration complete. Legacy data preserved at {:?}", legacy_dir);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub id: Option<i64>,
    pub event_type: String,
    pub source_agent: String,
    pub session_id: String,
    pub project_dir: String,
    pub tool_name: Option<String>,
    pub payload: Option<String>,
    pub feature_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub id: String,
    pub project_dir: String,
    pub description: String,
    pub category: String,
    pub passes: bool,
    pub in_progress: bool,
    pub agent: Option<String>,
    pub steps: Option<Vec<String>>,
    pub work_count: i32,
    pub completion_criteria: Option<String>,
    pub updated_at: String,
    // Agent-managed state
    pub confidence: Option<i32>,         // 0-100, agent's estimate of completion
    pub model: Option<String>,           // Which model is working on it (e.g., "claude-3.5-sonnet")
    pub is_streaming: bool,              // Currently generating output
    pub retry_count: i32,                // Loop detection - increments on repeated failures
    pub token_cost: Option<i64>,         // Running token cost
    pub has_error: bool,                 // Error state for visual indicator
    pub last_agent_update: Option<String>, // Timestamp of last agent update
    // Human override state
    pub manual_priority: Option<String>, // "high" | "normal" - human override for priority
    pub human_override_until: Option<String>, // Timestamp to lock human state
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub session_id: String,
    pub source_agent: String,
    pub project_dir: String,
    pub started_at: String,
    pub last_activity: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub total: i64,
    pub completed: i64,
    pub in_progress: i64,
    pub percentage: f64,
    pub active_sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub watched_projects: Vec<String>,
    pub sync_server_port: u16,
    pub notifications_enabled: bool,
    pub selected_project: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            watched_projects: vec![],
            sync_server_port: 4000,
            notifications_enabled: true,
            selected_project: None,
        }
    }
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Configure SQLite for concurrent access (WAL mode)
        // This allows hooks and Tauri app to safely share the database
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 10000;
            PRAGMA cache_size = -2000;
            "#,
        )?;

        tracing::info!("Database opened with WAL mode at {:?}", path);

        // Create base tables
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                session_id TEXT NOT NULL,
                project_dir TEXT NOT NULL,
                tool_name TEXT,
                payload TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS features (
                id TEXT PRIMARY KEY,
                project_dir TEXT NOT NULL,
                description TEXT NOT NULL,
                category TEXT DEFAULT 'functional',
                passes INTEGER DEFAULT 0,
                in_progress INTEGER DEFAULT 0,
                agent TEXT,
                steps TEXT,
                work_count INTEGER DEFAULT 0,
                completion_criteria TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            );
            
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                source_agent TEXT NOT NULL,
                project_dir TEXT NOT NULL,
                started_at TEXT DEFAULT (datetime('now')),
                last_activity TEXT DEFAULT (datetime('now')),
                status TEXT DEFAULT 'active'
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id);
            CREATE INDEX IF NOT EXISTS idx_events_project ON events(project_dir);
            CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_features_project ON features(project_dir);
            "#,
        )?;

        // Migration: Add feature_id column if it doesn't exist
        // SQLite doesn't support IF NOT EXISTS for columns, so we try and ignore errors
        let _ = conn.execute("ALTER TABLE events ADD COLUMN feature_id TEXT", []);

        // Create index on feature_id (will only create if doesn't exist)
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_events_feature_id ON events(feature_id);")?;

        // Migration: Add steps column to features table
        let _ = conn.execute("ALTER TABLE features ADD COLUMN steps TEXT", []);

        // Migration: Add work_count and completion_criteria columns for auto-completion
        let _ = conn.execute("ALTER TABLE features ADD COLUMN work_count INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN completion_criteria TEXT", []);

        // Migration: Add agent-managed state columns
        let _ = conn.execute("ALTER TABLE features ADD COLUMN confidence INTEGER", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN model TEXT", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN is_streaming INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN retry_count INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN token_cost INTEGER", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN has_error INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN last_agent_update TEXT", []);

        // Migration: Add human override state columns
        let _ = conn.execute("ALTER TABLE features ADD COLUMN manual_priority TEXT", []);
        let _ = conn.execute("ALTER TABLE features ADD COLUMN human_override_until TEXT", []);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_event(&self, event: &AgentEvent) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO events (event_type, source_agent, session_id, project_dir, tool_name, payload, feature_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.event_type,
                event.source_agent,
                event.session_id,
                event.project_dir,
                event.tool_name,
                event.payload,
                event.feature_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_events(&self, limit: i64) -> Result<Vec<AgentEvent>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, event_type, source_agent, session_id, project_dir, tool_name, payload, feature_id, created_at
             FROM events ORDER BY created_at DESC LIMIT ?1",
        )?;

        let events = stmt
            .query_map([limit], |row| {
                Ok(AgentEvent {
                    id: Some(row.get(0)?),
                    event_type: row.get(1)?,
                    source_agent: row.get(2)?,
                    session_id: row.get(3)?,
                    project_dir: row.get(4)?,
                    tool_name: row.get(5)?,
                    payload: row.get(6)?,
                    feature_id: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    pub fn get_events_by_feature(&self, feature_id: &str, limit: i64) -> Result<Vec<AgentEvent>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, event_type, source_agent, session_id, project_dir, tool_name, payload, feature_id, created_at
             FROM events WHERE feature_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![feature_id, limit], |row| {
                Ok(AgentEvent {
                    id: Some(row.get(0)?),
                    event_type: row.get(1)?,
                    source_agent: row.get(2)?,
                    session_id: row.get(3)?,
                    project_dir: row.get(4)?,
                    tool_name: row.get(5)?,
                    payload: row.get(6)?,
                    feature_id: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get events without a feature_id (unlinked)
    pub fn get_unlinked_events(&self, project_dir: Option<&str>, limit: i64) -> Result<Vec<AgentEvent>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(dir) = project_dir {
            (
                "SELECT id, event_type, source_agent, session_id, project_dir, tool_name, payload, feature_id, created_at
                 FROM events WHERE (feature_id IS NULL OR feature_id = '') AND project_dir = ?1
                 ORDER BY created_at DESC LIMIT ?2",
                vec![Box::new(dir.to_string()), Box::new(limit)]
            )
        } else {
            (
                "SELECT id, event_type, source_agent, session_id, project_dir, tool_name, payload, feature_id, created_at
                 FROM events WHERE (feature_id IS NULL OR feature_id = '')
                 ORDER BY created_at DESC LIMIT ?1",
                vec![Box::new(limit)]
            )
        };

        let mut stmt = conn.prepare(sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let events = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(AgentEvent {
                    id: Some(row.get(0)?),
                    event_type: row.get(1)?,
                    source_agent: row.get(2)?,
                    session_id: row.get(3)?,
                    project_dir: row.get(4)?,
                    tool_name: row.get(5)?,
                    payload: row.get(6)?,
                    feature_id: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Update an event's feature_id
    pub fn link_event_to_feature(&self, event_id: i64, feature_id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE events SET feature_id = ?1 WHERE id = ?2",
            params![feature_id, event_id],
        )?;
        Ok(rows > 0)
    }

    pub fn sync_features(
        &self,
        project_dir: &str,
        features: Vec<Feature>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        for feature in features {
            let steps_json = feature
                .steps
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_default());

            conn.execute(
                "INSERT OR REPLACE INTO features (
                    id, project_dir, description, category, passes, in_progress, agent, steps,
                    work_count, completion_criteria, updated_at,
                    confidence, model, is_streaming, retry_count, token_cost, has_error, last_agent_update,
                    manual_priority, human_override_until
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'),
                         ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    feature.id,
                    project_dir,
                    feature.description,
                    feature.category,
                    feature.passes,
                    feature.in_progress,
                    feature.agent,
                    steps_json,
                    feature.work_count,
                    feature.completion_criteria,
                    feature.confidence,
                    feature.model,
                    feature.is_streaming,
                    feature.retry_count,
                    feature.token_cost,
                    feature.has_error,
                    feature.last_agent_update,
                    feature.manual_priority,
                    feature.human_override_until,
                ],
            )?;
        }

        Ok(())
    }

    pub fn get_features(&self, project_dir: Option<&str>) -> Result<Vec<Feature>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        fn parse_steps(steps_json: Option<String>) -> Option<Vec<String>> {
            steps_json.and_then(|s| serde_json::from_str(&s).ok())
        }

        fn map_feature_row(row: &rusqlite::Row) -> rusqlite::Result<Feature> {
            Ok(Feature {
                id: row.get(0)?,
                project_dir: row.get(1)?,
                description: row.get(2)?,
                category: row.get(3)?,
                passes: row.get(4)?,
                in_progress: row.get(5)?,
                agent: row.get(6)?,
                steps: parse_steps(row.get(7)?),
                work_count: row.get::<_, Option<i32>>(8)?.unwrap_or(0),
                completion_criteria: row.get(9)?,
                updated_at: row.get(10)?,
                confidence: row.get(11)?,
                model: row.get(12)?,
                is_streaming: row.get::<_, Option<bool>>(13)?.unwrap_or(false),
                retry_count: row.get::<_, Option<i32>>(14)?.unwrap_or(0),
                token_cost: row.get(15)?,
                has_error: row.get::<_, Option<bool>>(16)?.unwrap_or(false),
                last_agent_update: row.get(17)?,
                manual_priority: row.get(18)?,
                human_override_until: row.get(19)?,
            })
        }

        let select_cols = "SELECT id, project_dir, description, category, passes, in_progress, agent, steps,
                          work_count, completion_criteria, updated_at,
                          confidence, model, is_streaming, retry_count, token_cost, has_error, last_agent_update,
                          manual_priority, human_override_until
                          FROM features";

        if let Some(dir) = project_dir {
            let mut stmt = conn.prepare(&format!("{} WHERE project_dir = ?1 ORDER BY id", select_cols))?;
            let features = stmt
                .query_map([dir], map_feature_row)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(features)
        } else {
            let mut stmt = conn.prepare(&format!("{} ORDER BY project_dir, id", select_cols))?;
            let features = stmt
                .query_map([], map_feature_row)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(features)
        }
    }

    pub fn get_sessions(&self) -> Result<Vec<Session>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, source_agent, project_dir, started_at, last_activity, status
             FROM sessions WHERE status = 'active' ORDER BY last_activity DESC",
        )?;

        let sessions = stmt
            .query_map([], |row| {
                Ok(Session {
                    session_id: row.get(0)?,
                    source_agent: row.get(1)?,
                    project_dir: row.get(2)?,
                    started_at: row.get(3)?,
                    last_activity: row.get(4)?,
                    status: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    pub fn upsert_session(&self, session: &Session) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO sessions (session_id, source_agent, project_dir, started_at, last_activity, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.session_id,
                session.source_agent,
                session.project_dir,
                session.started_at,
                session.last_activity,
                session.status,
            ],
        )?;
        Ok(())
    }

    pub fn update_session_status(
        &self,
        session_id: &str,
        status: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET status = ?1, last_activity = datetime('now') WHERE session_id = ?2",
            params![status, session_id],
        )?;
        Ok(())
    }

    /// Clean up stale sessions that have been inactive for more than the specified minutes.
    /// Returns the number of sessions marked as ended.
    pub fn cleanup_stale_sessions(&self, inactive_minutes: i64) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE sessions SET status = 'ended'
             WHERE status = 'active'
             AND datetime(last_activity) < datetime('now', ?1)",
            params![format!("-{} minutes", inactive_minutes)],
        )?;

        if rows > 0 {
            tracing::info!("Cleaned up {} stale sessions (inactive > {} minutes)", rows, inactive_minutes);
        }

        Ok(rows)
    }

    pub fn get_stats(&self) -> Result<Stats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM features", [], |r| r.get(0))?;

        let completed: i64 =
            conn.query_row("SELECT COUNT(*) FROM features WHERE passes = 1", [], |r| {
                r.get(0)
            })?;

        let in_progress: i64 = conn.query_row(
            "SELECT COUNT(*) FROM features WHERE in_progress = 1 AND passes = 0",
            [],
            |r| r.get(0),
        )?;

        let active_sessions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE status = 'active'",
            [],
            |r| r.get(0),
        )?;

        let percentage = if total > 0 {
            (completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(Stats {
            total,
            completed,
            in_progress,
            percentage,
            active_sessions,
        })
    }

    pub fn get_config(&self) -> Result<Config, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let config_json: Option<String> = conn
            .query_row("SELECT value FROM config WHERE key = 'main'", [], |r| {
                r.get(0)
            })
            .ok();

        match config_json {
            Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
            None => Ok(Config::default()),
        }
    }

    pub fn save_config(&self, config: &Config) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(config).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES ('main', ?1)",
            [json],
        )?;
        Ok(())
    }

    pub fn get_projects(&self) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT project_dir FROM features ORDER BY project_dir",
        )?;

        let projects = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(projects)
    }

    /// Add a project to watched_projects if not already present.
    /// Returns true if the project was added, false if already exists.
    pub fn add_watched_project(&self, project_dir: &str) -> Result<bool, rusqlite::Error> {
        let mut config = self.get_config()?;

        if config.watched_projects.contains(&project_dir.to_string()) {
            return Ok(false);
        }

        config.watched_projects.push(project_dir.to_string());
        self.save_config(&config)?;
        Ok(true)
    }

    /// Update a feature with source-aware override logic.
    /// - Human updates always apply and set a 5-minute lock
    /// - Agent updates only apply if no human override is active
    /// Returns true if the update was applied, false if blocked by human override
    pub fn update_feature(
        &self,
        feature_id: &str,
        update: FeatureUpdate,
        source: UpdateSource,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // First, check if there's an active human override
        let current_override: Option<String> = conn
            .query_row(
                "SELECT human_override_until FROM features WHERE id = ?1",
                [feature_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        let now = chrono::Utc::now();

        // If source is agent, check if human override is active
        if matches!(source, UpdateSource::Agent) {
            if let Some(override_until) = current_override {
                if let Ok(override_time) = chrono::DateTime::parse_from_rfc3339(&override_until) {
                    if override_time > now {
                        tracing::info!(
                            "Agent update blocked for feature {} - human override active until {}",
                            feature_id,
                            override_until
                        );
                        return Ok(false);
                    }
                }
            }
        }

        // Build dynamic UPDATE statement based on provided fields
        let mut updates = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(v) = update.passes { updates.push("passes = ?"); params.push(Box::new(v)); }
        if let Some(v) = update.in_progress { updates.push("in_progress = ?"); params.push(Box::new(v)); }
        if let Some(v) = &update.agent { updates.push("agent = ?"); params.push(Box::new(v.clone())); }
        if let Some(v) = update.confidence { updates.push("confidence = ?"); params.push(Box::new(v)); }
        if let Some(v) = &update.model { updates.push("model = ?"); params.push(Box::new(v.clone())); }
        if let Some(v) = update.is_streaming { updates.push("is_streaming = ?"); params.push(Box::new(v)); }
        if let Some(v) = update.retry_count { updates.push("retry_count = ?"); params.push(Box::new(v)); }
        if let Some(v) = update.token_cost { updates.push("token_cost = ?"); params.push(Box::new(v)); }
        if let Some(v) = update.has_error { updates.push("has_error = ?"); params.push(Box::new(v)); }
        if let Some(v) = &update.manual_priority { updates.push("manual_priority = ?"); params.push(Box::new(v.clone())); }

        if updates.is_empty() {
            return Ok(true); // Nothing to update
        }

        // Add source-specific fields
        match source {
            UpdateSource::Human => {
                // Set 5-minute human override lock
                let override_until = (now + chrono::Duration::minutes(5)).to_rfc3339();
                updates.push("human_override_until = ?");
                params.push(Box::new(override_until));
            }
            UpdateSource::Agent => {
                updates.push("last_agent_update = ?");
                params.push(Box::new(now.to_rfc3339()));
            }
        }

        // Always update updated_at
        updates.push("updated_at = datetime('now')");

        // Build and execute query
        let sql = format!(
            "UPDATE features SET {} WHERE id = ?",
            updates.join(", ")
        );
        params.push(Box::new(feature_id.to_string()));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = conn.execute(&sql, params_refs.as_slice())?;

        Ok(rows > 0)
    }

    /// Sync a feature from graph database to SQLite cache.
    /// This upserts the feature, converting graph status to SQLite boolean flags.
    pub fn sync_feature_from_graph(&self, feature: &GraphFeatureSync) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let passes = feature.status == "complete";
        let in_progress = feature.status == "in_progress";
        let steps_json = serde_json::to_string(&feature.steps).ok();

        conn.execute(
            "INSERT INTO features (id, project_dir, description, category, passes, in_progress, steps, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
                project_dir = excluded.project_dir,
                description = excluded.description,
                category = excluded.category,
                passes = excluded.passes,
                in_progress = excluded.in_progress,
                steps = excluded.steps,
                updated_at = datetime('now')",
            params![
                feature.id,
                feature.project_dir,
                feature.description,
                feature.category,
                passes,
                in_progress,
                steps_json,
            ],
        )?;

        Ok(())
    }

    /// Get a single feature by ID
    pub fn get_feature(&self, feature_id: &str) -> Result<Option<Feature>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        fn parse_steps(steps_json: Option<String>) -> Option<Vec<String>> {
            steps_json.and_then(|s| serde_json::from_str(&s).ok())
        }

        let result = conn.query_row(
            "SELECT id, project_dir, description, category, passes, in_progress, agent, steps,
                    work_count, completion_criteria, updated_at,
                    confidence, model, is_streaming, retry_count, token_cost, has_error, last_agent_update,
                    manual_priority, human_override_until
             FROM features WHERE id = ?1",
            [feature_id],
            |row| {
                Ok(Feature {
                    id: row.get(0)?,
                    project_dir: row.get(1)?,
                    description: row.get(2)?,
                    category: row.get(3)?,
                    passes: row.get(4)?,
                    in_progress: row.get(5)?,
                    agent: row.get(6)?,
                    steps: parse_steps(row.get(7)?),
                    work_count: row.get::<_, Option<i32>>(8)?.unwrap_or(0),
                    completion_criteria: row.get(9)?,
                    updated_at: row.get(10)?,
                    confidence: row.get(11)?,
                    model: row.get(12)?,
                    is_streaming: row.get::<_, Option<bool>>(13)?.unwrap_or(false),
                    retry_count: row.get::<_, Option<i32>>(14)?.unwrap_or(0),
                    token_cost: row.get(15)?,
                    has_error: row.get::<_, Option<bool>>(16)?.unwrap_or(false),
                    last_agent_update: row.get(17)?,
                    manual_priority: row.get(18)?,
                    human_override_until: row.get(19)?,
                })
            },
        );

        match result {
            Ok(feature) => Ok(Some(feature)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Source of a feature update - determines override behavior
#[derive(Debug, Clone, Copy)]
pub enum UpdateSource {
    Human, // User interaction (drag-drop, click, etc.) - always wins, sets 5-min lock
    Agent, // Agent/hook update - blocked if human override active
}

/// Feature data from graph database for syncing to SQLite cache
#[derive(Debug, Clone)]
pub struct GraphFeatureSync {
    pub id: String,
    pub project_dir: String,
    pub description: String,
    pub category: String,
    pub status: String,
    pub steps: Vec<String>,
}

/// Partial update struct for features - only set fields you want to update
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureUpdate {
    pub passes: Option<bool>,
    pub in_progress: Option<bool>,
    pub agent: Option<String>,
    pub confidence: Option<i32>,
    pub model: Option<String>,
    pub is_streaming: Option<bool>,
    pub retry_count: Option<i32>,
    pub token_cost: Option<i64>,
    pub has_error: Option<bool>,
    pub manual_priority: Option<String>,
}
