//! Optimization engine: template rendering → LLM call → score → diff.
//!
//! Framework templates are stored as JSON files under `framework_packs/` (see
//! spec §8). The engine renders the selected template with `minijinja`,
//! routes to the chosen provider, streams chunks back to the UI via Tauri
//! events, then produces a heuristic quality score and a unified diff.

use std::collections::HashMap;
use std::path::PathBuf;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::info;

use crate::db::DbService;
use crate::providers::{build, ProviderError};
use crate::types::{ChatParams, ContextProfile, Message, OptimizeRequest, OptimizeResult};

/// A framework pack loaded from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub variables: Vec<String>,
    pub template: String,
}

pub struct OptimizationEngine {
    frameworks: HashMap<String, FrameworkPack>,
}

impl OptimizationEngine {
    /// Load all `*.json` files from `framework_packs/` relative to the binary
    /// (or fall back to a compile-time embedded path).
    pub fn new(app_data_dir: &std::path::Path) -> anyhow::Result<Self> {
        let packs_dir = app_data_dir.join("framework_packs");
        // Also look next to the binary (for dev).
        let mut frameworks = HashMap::new();

        let mut load_from = |dir: &PathBuf| -> anyhow::Result<()> {
            if !dir.exists() {
                return Ok(());
            }
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    let content = std::fs::read_to_string(&path)?;
                    let pack: FrameworkPack = serde_json::from_str(&content)?;
                    info!(pack_id = %pack.id, "loaded framework pack");
                    frameworks.insert(pack.id.clone(), pack);
                }
            }
            Ok(())
        };

        // Try next-to-binary first (dev / `cargo run`).
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        if let Some(ref dir) = exe_dir {
            let _ = load_from(&dir.join("framework_packs"));
        }
        // Then app data dir.
        load_from(&packs_dir)?;

        // Hardcoded fallbacks in case no files found on disk.
        if frameworks.is_empty() {
            let create = FrameworkPack {
                id: "CREATE".into(),
                name: "CREATE".into(),
                variables: vec!["context".into(), "role".into(), "task".into(), "explanation".into(), "constraints".into(), "tone".into(), "extras".into()],
                template: r#"You are a prompt engineer. Rewrite the user's raw prompt using the CREATE framework.

CREATE = Context, Request, Explanation, Action, Tone, Extras.
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                    .into(),
            };
            let ape = FrameworkPack {
                id: "APE".into(),
                name: "APE (Action, Purpose, Expectation)".into(),
                variables: vec!["action".into(), "purpose".into(), "expectation".into()],
                template: r#"You are a prompt engineer. Rewrite the user's raw prompt using the APE framework (Action, Purpose, Expectation).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}"#
                    .into(),
            };
            frameworks.insert("CREATE".into(), create);
            frameworks.insert("APE".into(), ape);
        }

        Ok(Self { frameworks })
    }

    pub fn list_frameworks(&self) -> Vec<&FrameworkPack> {
        let mut v: Vec<_> = self.frameworks.values().collect();
        v.sort_by_key(|f| f.id.clone());
        v
    }

    pub fn get_framework(&self, id: &str) -> Option<&FrameworkPack> {
        self.frameworks.get(id)
    }

    /// Render the selected framework template with the raw prompt and
    /// optional context profile, returning the system message for the LLM.
    fn render_template(
        &self,
        req: &OptimizeRequest,
        ctx: Option<&ContextProfile>,
    ) -> anyhow::Result<String> {
        let pack = self.frameworks.get(&req.framework)
            .ok_or_else(|| anyhow::anyhow!("framework '{}' not found", req.framework))?;

        let mut env = minijinja::Environment::new();
        env.add_template("framework", &pack.template)?;
        let tmpl = env.get_template("framework")?;

        let mut vars = std::collections::HashMap::new();
        vars.insert("raw_prompt", req.raw.as_str());
        if let Some(c) = ctx {
            vars.insert("context", c.style_snippet.as_deref().unwrap_or(""));
            vars.insert("role", c.role.as_deref().unwrap_or(""));
            vars.insert("tone", c.tone.as_deref().unwrap_or(""));
            vars.insert("audience", c.audience.as_deref().unwrap_or(""));
        } else {
            vars.insert("context", "");
            vars.insert("role", "");
            vars.insert("tone", "");
            vars.insert("audience", "");
        }

        tmpl.render(vars).map_err(Into::into)
    }

    /// Run the full optimization: render → stream → emit events → emit done.
    ///
    /// Returns the accumulated optimized text (also logged to history).
    pub async fn optimize(
        &self,
        app: AppHandle,
        db: &DbService,
        req: OptimizeRequest,
        ollama_url: &str,
    ) -> Result<OptimizeResult, ProviderError> {
        let session_id = uuid::Uuid::new_v4().to_string();

        // Load context profile if provided.
        let ctx = if let Some(ref cid) = req.context_id {
            db.get_context(cid).ok().flatten()
        } else {
            None
        };

        // Render framework template → system message.
        let system_prompt = self.render_template(&req, ctx.as_ref())
            .map_err(|e| ProviderError::Parse(format!("template render: {e}")))?;

        // Extract model name from selector ("ollama:llama3" → "llama3").
        let model_name = req.model.split(':').nth(1)
            .unwrap_or(&req.model)
            .to_string();

        // Build messages.
        let messages = vec![
            Message::system(&system_prompt),
            Message::user(&req.raw),
        ];
        let params = ChatParams {
            model: model_name.clone(),
            ..Default::default()
        };

        // Resolve provider adapter.
        let adapter = build(&req.model, ollama_url)
            .ok_or_else(|| ProviderError::Unreachable(format!("unknown provider in '{}'", req.model)))?;

        // Stream chunks to the UI via Tauri events.
        let mut stream = adapter.stream_chat(messages, params).await?;

        let mut optimized = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => {
                    optimized.push_str(&text);
                    let _ = app.emit("opt_chunk", serde_json::json!({
                        "text": &text,
                        "session_id": &session_id,
                    }));
                }
                Err(e) => {
                    let _ = app.emit("opt_error", serde_json::json!({
                        "code": "STREAM_ERROR",
                        "message": e.to_string(),
                        "session_id": &session_id,
                    }));
                    return Err(e);
                }
            }
        }

        // Compute score (heuristic).
        let score = Self::score_prompt(&optimized);

        // Compute diff.
        let diff = Self::compute_diff(&req.raw, &optimized);

        // Approximate token count (chars / 4).
        let tokens = optimized.len() / 4;

        // Log to history.
        let _ = db.add_history(&req.raw, &optimized, &req.model, Some(score as i64));

        let result = OptimizeResult {
            optimized: optimized.clone(),
            score,
            diff,
            tokens,
            session_id: session_id.clone(),
        };

        let _ = app.emit("opt_done", &result);

        Ok(result)
    }

    /// Heuristic quality score 0–100.
    ///
    /// Rewards: length (100–500 words sweet spot), structure markers
    /// (headings, bullet dashes, numbered lists), specificity signals
    /// (quantified phrases, explicit constraints).
    fn score_prompt(text: &str) -> u32 {
        if text.is_empty() { return 0; }
        let mut score: u32 = 30; // baseline

        let words: Vec<&str> = text.split_whitespace().collect();
        let word_count = words.len() as u32;

        // Length reward (optimal 50–300 words for a prompt).
        match word_count {
            0..=10 => score += 0,
            11..=50 => score += 10,
            51..=300 => score += 20,
            301..=600 => score += 10,
            _ => score += 5,
        }

        // Structure markers.
        let lines: Vec<&str> = text.lines().collect();
        let has_headings = lines.iter().any(|l| l.starts_with('#'));
        let has_bullets = lines.iter().any(|l| l.trim_start().starts_with("- ") || l.trim_start().starts_with("* "));
        let has_numbers = lines.iter().any(|l| {
            l.trim_start().chars().next().map_or(false, |c| c.is_ascii_digit())
        });

        if has_headings { score += 10; }
        if has_bullets { score += 8; }
        if has_numbers { score += 7; }

        // Specificity signals.
        let specificity_markers = ["example", "for instance", "such as", "e.g.", "i.e.",
            "step", "format", "output", "constraint", "criteria", "define",
            "at least", "no more than", "exactly"];
        let lower = text.to_lowercase();
        let hits = specificity_markers.iter()
            .filter(|m| lower.contains(*m))
            .count();
        score += (hits as u32) * 3;

        // Penalty for excessive repetition.
        let unique_ratio = if words.len() > 5 {
            let unique: std::collections::HashSet<&str> = words.iter().cloned().collect();
            unique.len() as f64 / words.len() as f64
        } else {
            1.0
        };
        if unique_ratio < 0.5 {
            score = score.saturating_sub(15);
        }

        score.min(100)
    }

    /// Unified diff using the `similar` crate.
    fn compute_diff(raw: &str, optimized: &str) -> String {
        let patch = similar::TextDiff::from_lines(raw, optimized)
            .unified_diff()
            .header("Original", "Optimized")
            .to_string();
        // Truncate very long diffs for the overlay (keep first 2 KB).
        if patch.len() > 2048 {
            format!("{}…\n(truncated)", &patch[..2048])
        } else {
            patch
        }
    }
}
