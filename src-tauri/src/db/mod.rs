//! SQLite persistence for PromptOpt.
//!
//! Schema follows `design_docs/04_Database_Design.md` (prompts, context_profiles,
//! app_profiles, history) plus a `settings` key/value table for app config.
//! Location: `<app_data_dir>/PromptOpt/data.db`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde_json::json;

use crate::types::{
    ContextProfile, HistoryEntry, Prompt, Settings,
};

pub struct DbService {
    conn: Mutex<Connection>,
}

impl DbService {
    /// Open (or create) the database at the given path and run migrations.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path.as_ref())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Self::migrate(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Default path: <data_dir>/PromptOpt/data.db
    pub fn default_path() -> anyhow::Result<PathBuf> {
        let base = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("no data dir"))?;
        Ok(base.join("PromptOpt").join("data.db"))
    }

    fn migrate(conn: &Connection) -> anyhow::Result<()> {
        // DDL from design_docs/04_Database_Design.md §3 (+ settings table).
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS prompts (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                body        TEXT NOT NULL,
                framework   TEXT,
                model_used  TEXT,
                score       INTEGER DEFAULT 0,
                usage_count INTEGER DEFAULT 0,
                source_app  TEXT,
                context_id  TEXT REFERENCES context_profiles(id),
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                is_deleted  INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_prompts_framework ON prompts(framework);
            CREATE INDEX IF NOT EXISTS idx_prompts_score ON prompts(score DESC);

            CREATE TABLE IF NOT EXISTS context_profiles (
                id            TEXT PRIMARY KEY,
                name          TEXT NOT NULL,
                role          TEXT,
                audience      TEXT,
                tone          TEXT,
                style_snippet TEXT
            );

            CREATE TABLE IF NOT EXISTS app_profiles (
                app_name             TEXT PRIMARY KEY,
                default_framework    TEXT,
                default_model        TEXT,
                replacement_strategy TEXT CHECK(replacement_strategy IN ('Accessibility','Clipboard','SyntheticKeys'))
            );

            CREATE TABLE IF NOT EXISTS history (
                id                TEXT PRIMARY KEY,
                raw_prompt        TEXT NOT NULL,
                optimized_prompt  TEXT NOT NULL,
                model             TEXT NOT NULL,
                score             INTEGER,
                timestamp         TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC);

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS providers (
                id            TEXT PRIMARY KEY,
                kind          TEXT NOT NULL,
                label         TEXT NOT NULL,
                base_url      TEXT NOT NULL,
                api_key_slot  TEXT,
                default_model TEXT NOT NULL DEFAULT '',
                enabled       INTEGER NOT NULL DEFAULT 1,
                sort_order    INTEGER NOT NULL DEFAULT 0,
                created_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Self::seed_defaults(conn)?;
        Self::seed_providers(conn)?;
        // Heal existing installs: if user picked a provider in onboarding but
        // the old code never enabled it (bug: complete_onboarding missing
        // set_provider_enabled), enable it now.
        let pid: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = 'default_provider_id'", [], |r| r.get(0))
            .ok();
        if let Some(pid) = pid {
            let _ = conn.execute("UPDATE providers SET enabled = 1 WHERE id = ?1", params![pid]);
        }
        Ok(())
    }

    fn seed_defaults(conn: &Connection) -> anyhow::Result<()> {
        let defaults = Settings::default();
        let pairs: [(&str, String); 7] = [
            ("hotkey", defaults.hotkey.clone()),
            ("theme", defaults.theme.clone()),
            ("default_framework", defaults.default_framework.clone()),
            ("default_model", defaults.default_model.clone()),
            ("ollama_url", defaults.ollama_url.clone()),
            ("default_provider_id", defaults.default_provider_id.clone()),
            ("overlay_opacity", defaults.overlay_opacity.to_string()),
        ];
        for (k, v) in pairs {
            conn.execute(
                "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        Ok(())
    }

    // ---- settings ---------------------------------------------------------

    pub fn get_settings(&self) -> anyhow::Result<Settings> {
        let conn = self.conn.lock().unwrap();
        let read = |key: &str| -> String {
            conn.query_row(
                "SELECT value FROM settings WHERE key=?1",
                params![key],
                |r| r.get(0),
            )
            .unwrap_or_default()
        };
        Ok(Settings {
            hotkey: read("hotkey"),
            theme: read("theme"),
            default_framework: read("default_framework"),
            default_model: read("default_model"),
            ollama_url: read("ollama_url"),
            default_provider_id: read("default_provider_id"),
            overlay_opacity: read("overlay_opacity").parse().unwrap_or(90),
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- prompts ----------------------------------------------------------

    pub fn save_prompt(&self, p: &Prompt) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO prompts(id, title, body, framework, model_used, score, usage_count, source_app)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(id) DO UPDATE SET
                 title=excluded.title, body=excluded.body, framework=excluded.framework,
                 model_used=excluded.model_used, score=excluded.score,
                 usage_count=excluded.usage_count, source_app=excluded.source_app"#,
            params![
                p.id, p.title, p.body, p.framework, p.model_used,
                p.score, p.usage_count, p.source_app,
            ],
        )?;
        Ok(())
    }

    pub fn list_prompts(&self) -> anyhow::Result<Vec<Prompt>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, body, framework, model_used, score, usage_count, source_app, created_at
             FROM prompts WHERE is_deleted = 0
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Prompt {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                framework: r.get(3)?,
                model_used: r.get(4)?,
                score: r.get(5)?,
                usage_count: r.get(6)?,
                source_app: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn search_prompts(&self, query: &str) -> anyhow::Result<Vec<Prompt>> {
        let conn = self.conn.lock().unwrap();
        let like = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, title, body, framework, model_used, score, usage_count, source_app, created_at
             FROM prompts
             WHERE is_deleted = 0 AND (title LIKE ?1 OR body LIKE ?1)
             ORDER BY score DESC, created_at DESC",
        )?;
        let rows = stmt.query_map(params![like], |r| {
            Ok(Prompt {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                framework: r.get(3)?,
                model_used: r.get(4)?,
                score: r.get(5)?,
                usage_count: r.get(6)?,
                source_app: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn delete_prompt(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE prompts SET is_deleted=1 WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn increment_usage(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE prompts SET usage_count = usage_count + 1 WHERE id=?1",
            params![id],
        )?;
        Ok(())
    }

    // ---- context profiles -------------------------------------------------

    pub fn save_context(&self, c: &ContextProfile) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO context_profiles(id, name, role, audience, tone, style_snippet)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(id) DO UPDATE SET
                 name=excluded.name, role=excluded.role, audience=excluded.audience,
                 tone=excluded.tone, style_snippet=excluded.style_snippet"#,
            params![c.id, c.name, c.role, c.audience, c.tone, c.style_snippet],
        )?;
        Ok(())
    }

    pub fn list_contexts(&self) -> anyhow::Result<Vec<ContextProfile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, role, audience, tone, style_snippet FROM context_profiles",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ContextProfile {
                id: r.get(0)?,
                name: r.get(1)?,
                role: r.get(2)?,
                audience: r.get(3)?,
                tone: r.get(4)?,
                style_snippet: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_context(&self, id: &str) -> anyhow::Result<Option<ContextProfile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, role, audience, tone, style_snippet FROM context_profiles WHERE id=?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(ContextProfile {
                id: r.get(0)?,
                name: r.get(1)?,
                role: r.get(2)?,
                audience: r.get(3)?,
                tone: r.get(4)?,
                style_snippet: r.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_context(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM context_profiles WHERE id=?1", params![id])?;
        Ok(())
    }

    // ---- history ----------------------------------------------------------

    pub fn add_history(
        &self,
        raw: &str,
        optimized: &str,
        model: &str,
        score: Option<i64>,
    ) -> anyhow::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history(id, raw_prompt, optimized_prompt, model, score)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, raw, optimized, model, score],
        )?;
        Ok(id)
    }

    pub fn list_history(&self, limit: i64) -> anyhow::Result<Vec<HistoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, raw_prompt, optimized_prompt, model, score, timestamp
             FROM history ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                raw_prompt: r.get(1)?,
                optimized_prompt: r.get(2)?,
                model: r.get(3)?,
                score: r.get(4)?,
                timestamp: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    // ---- providers ---------------------------------------------------------

    fn seed_providers(conn: &Connection) -> anyhow::Result<()> {
        // Only seed if table is empty (preserves user edits across upgrades).
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM providers", [], |r| r.get(0))
            .unwrap_or(0);
        if count > 0 {
            return Ok(());
        }

        let defaults = [
            ("ollama", "ollama", "Ollama (local)", "http://localhost:11434", None, "llama3", true, 0),
            ("lmstudio", "openai_compat", "LM Studio (local)", "http://localhost:1234/v1", None, "", true, 1),
            ("openai", "openai_compat", "OpenAI", "https://api.openai.com/v1", Some("openai"), "gpt-4o", false, 10),
            ("anthropic", "anthropic", "Anthropic", "https://api.anthropic.com", Some("anthropic"), "claude-sonnet-4-20250514", false, 11),
            ("openrouter", "openai_compat", "OpenRouter", "https://openrouter.ai/api/v1", Some("openrouter"), "anthropic/claude-sonnet-4", false, 12),
            ("nvidia_nim", "openai_compat", "NVIDIA NIM", "https://integrate.api.nvidia.com/v1", Some("nvidia_nim"), "", false, 13),
            ("gemini", "gemini", "Google Gemini", "https://generativelanguage.googleapis.com", Some("gemini"), "gemini-2.0-flash", false, 14),
        ];
        for (id, kind, label, url, key_slot, model, enabled, order) in defaults {
            conn.execute(
                "INSERT OR IGNORE INTO providers(id, kind, label, base_url, api_key_slot, default_model, enabled, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, kind, label, url, key_slot, model, enabled as i64, order],
            )?;
        }
        Ok(())
    }

    pub fn list_providers(&self) -> anyhow::Result<Vec<crate::types::ProviderConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, label, base_url, api_key_slot, default_model, enabled, sort_order
             FROM providers ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(crate::types::ProviderConfig {
                id: r.get(0)?,
                kind: r.get(1)?,
                label: r.get(2)?,
                base_url: r.get(3)?,
                api_key_slot: r.get(4)?,
                default_model: r.get(5)?,
                enabled: r.get::<_, i64>(6)? != 0,
                sort_order: r.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn get_provider(&self, id: &str) -> anyhow::Result<Option<crate::types::ProviderConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, label, base_url, api_key_slot, default_model, enabled, sort_order
             FROM providers WHERE id=?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(r) => Ok(Some(crate::types::ProviderConfig {
                id: r.get(0)?,
                kind: r.get(1)?,
                label: r.get(2)?,
                base_url: r.get(3)?,
                api_key_slot: r.get(4)?,
                default_model: r.get(5)?,
                enabled: r.get::<_, i64>(6)? != 0,
                sort_order: r.get(7)?,
            })),
            None => Ok(None),
        }
    }

    pub fn save_provider(&self, p: &crate::types::ProviderConfig) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO providers(id, kind, label, base_url, api_key_slot, default_model, enabled, sort_order)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(id) DO UPDATE SET
                 kind=excluded.kind, label=excluded.label, base_url=excluded.base_url,
                 api_key_slot=excluded.api_key_slot, default_model=excluded.default_model,
                 enabled=excluded.enabled, sort_order=excluded.sort_order"#,
            params![p.id, p.kind, p.label, p.base_url, p.api_key_slot, p.default_model,
                    p.enabled as i64, p.sort_order],
        )?;
        Ok(())
    }

    pub fn delete_provider(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM providers WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn set_provider_enabled(&self, id: &str, enabled: bool) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE providers SET enabled=?2 WHERE id=?1", params![id, enabled as i64])?;
        Ok(())
    }

    // ---- meta -----------------------------------------------------------

    pub fn get_meta(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT value FROM meta WHERE key=?1", params![key], |r| r.get(0))
            .ok()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn clear_history(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    /// Diagnostic helper — return row counts per table as a JSON string.
    pub fn stats(&self) -> serde_json::Value {
        let conn = self.conn.lock().unwrap();
        let count = |table: &str| -> i64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(0)
        };
        json!({
            "prompts": count("prompts"),
            "context_profiles": count("context_profiles"),
            "app_profiles": count("app_profiles"),
            "history": count("history"),
            "providers": count("providers"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db() -> DbService {
        let path = std::env::temp_dir().join(format!(
            "promptopt_test_{}_{}.db",
            std::process::id(),
            std::thread::current().name().unwrap_or("unknown").replace("::", "_"),
        ));
        let _ = std::fs::remove_file(&path);
        DbService::open(&path).unwrap()
    }

    #[test]
    fn test_settings_roundtrip() {
        let db = tmp_db();
        let s = db.get_settings().unwrap();
        assert_eq!(s.hotkey, "Ctrl+Shift+E");
        assert_eq!(s.theme, "dark");
        assert_eq!(s.default_framework, "CREATE");
        assert_eq!(s.default_model, "ollama:llama3");
        assert_eq!(s.ollama_url, "http://localhost:11434");

        db.set_setting("hotkey", "Ctrl+Alt+P").unwrap();
        let s2 = db.get_settings().unwrap();
        assert_eq!(s2.hotkey, "Ctrl+Alt+P");
    }

    #[test]
    fn test_prompt_crud() {
        let db = tmp_db();
        assert_eq!(db.list_prompts().unwrap().len(), 0);

        let p = Prompt {
            id: "p1".into(),
            title: "Test Prompt".into(),
            body: "Write a poem about Rust".into(),
            framework: Some("CREATE".into()),
            model_used: Some("ollama:llama3".into()),
            score: 42,
            usage_count: 0,
            source_app: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        db.save_prompt(&p).unwrap();
        let list = db.list_prompts().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Test Prompt");

        // Search.
        let found = db.search_prompts("poem").unwrap();
        assert_eq!(found.len(), 1);

        // Soft delete.
        db.delete_prompt("p1").unwrap();
        assert_eq!(db.list_prompts().unwrap().len(), 0);
    }

    #[test]
    fn test_history_insert_and_list() {
        let db = tmp_db();
        assert_eq!(db.list_history(10).unwrap().len(), 0);

        db.add_history("raw", "optimized", "ollama:llama3", Some(75)).unwrap();
        db.add_history("raw2", "optimized2", "ollama:llama3", Some(80)).unwrap();

        let list = db.list_history(10).unwrap();
        assert_eq!(list.len(), 2);
        // Both entries present.
        let scores: Vec<Option<i64>> = list.iter().map(|h| h.score).collect();
        assert!(scores.contains(&Some(75)));
        assert!(scores.contains(&Some(80)));
    }

    #[test]
    fn test_context_crud() {
        let db = tmp_db();
        let c = ContextProfile {
            id: "ctx1".into(),
            name: "Dev Role".into(),
            role: Some("Senior Engineer".into()),
            audience: Some("Junior Devs".into()),
            tone: Some("concise".into()),
            style_snippet: None,
        };
        db.save_context(&c).unwrap();
        let list = db.list_contexts().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Dev Role");

        let found = db.get_context("ctx1").unwrap();
        assert!(found.is_some());
        assert!(db.get_context("nope").unwrap().is_none());

        // Delete.
        db.delete_context("ctx1").unwrap();
        assert!(db.get_context("ctx1").unwrap().is_none());
        assert_eq!(db.list_contexts().unwrap().len(), 0);
    }

    #[test]
    fn test_stats() {
        let db = tmp_db();
        let stats = db.stats();
        assert_eq!(stats["prompts"], 0);
        assert_eq!(stats["history"], 0);
    }

    #[test]
    fn test_providers_seeded() {
        let db = tmp_db();
        let providers = db.list_providers().unwrap();
        // Seed should insert 7 defaults.
        assert_eq!(providers.len(), 7, "expected 7 seeded providers, got {}", providers.len());
        let ollama = providers.iter().find(|p| p.id == "ollama").unwrap();
        assert_eq!(ollama.kind, "ollama");
        assert!(ollama.enabled);
        // Cloud providers disabled until key set.
        let openai = providers.iter().find(|p| p.id == "openai").unwrap();
        assert!(!openai.enabled);
    }

    #[test]
    fn test_provider_crud() {
        let db = tmp_db();
        // Save new custom provider.
        let custom = crate::types::ProviderConfig {
            id: "my_local".into(),
            kind: "openai_compat".into(),
            label: "My Local".into(),
            base_url: "http://localhost:9999".into(),
            api_key_slot: None,
            default_model: "my-model".into(),
            enabled: true,
            sort_order: 50,
        };
        db.save_provider(&custom).unwrap();
        let found = db.get_provider("my_local").unwrap().unwrap();
        assert_eq!(found.label, "My Local");

        // Update.
        let updated = crate::types::ProviderConfig { label: "Updated".into(), ..custom.clone() };
        db.save_provider(&updated).unwrap();
        let loaded = db.list_providers().unwrap();
        assert!(loaded.iter().any(|p| p.id == "my_local" && p.label == "Updated"));

        // Delete.
        db.delete_provider("my_local").unwrap();
        assert!(db.get_provider("my_local").unwrap().is_none());

        // Built-ins still there.
        assert!(db.get_provider("ollama").unwrap().is_some());
    }

    #[test]
    fn test_set_provider_enabled() {
        let db = tmp_db();
        let p = db.get_provider("openai").unwrap().unwrap();
        assert!(!p.enabled);
        db.set_provider_enabled("openai", true).unwrap();
        let p2 = db.get_provider("openai").unwrap().unwrap();
        assert!(p2.enabled);
    }

    #[test]
    fn test_meta_crud() {
        let db = tmp_db();
        assert_eq!(db.get_meta("nope"), None);
        db.set_meta("onboarding_completed", "1").unwrap();
        assert_eq!(db.get_meta("onboarding_completed"), Some("1".into()));
        db.set_meta("onboarding_completed", "0").unwrap();
        assert_eq!(db.get_meta("onboarding_completed"), Some("0".into()));
    }

    #[test]
    fn test_clear_history() {
        let db = tmp_db();
        db.add_history("raw", "opt", "m", Some(50)).unwrap();
        assert_eq!(db.list_history(10).unwrap().len(), 1);
        db.clear_history().unwrap();
        assert_eq!(db.list_history(10).unwrap().len(), 0);
    }
}
