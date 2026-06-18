//! Shared types passed across the Tauri IPC boundary.

use serde::{Deserialize, Serialize};

/// A single chat message in the unified provider format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
}

/// Per-call sampling/limits sent to an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatParams {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_model() -> String {
    "llama3".to_string()
}
fn default_temperature() -> f32 {
    0.7
}
fn default_max_tokens() -> u32 {
    1024
}

impl Default for ChatParams {
    fn default() -> Self {
        Self {
            model: default_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

/// A saved prompt in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub model_used: Option<String>,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub usage_count: i64,
    #[serde(default)]
    pub source_app: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub style_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub raw_prompt: String,
    pub optimized_prompt: String,
    pub model: String,
    #[serde(default)]
    pub score: Option<i64>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub hotkey: String,
    pub theme: String,
    pub default_framework: String,
    pub default_model: String,
    pub ollama_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+E".to_string(),
            theme: "dark".to_string(),
            default_framework: "CREATE".to_string(),
            default_model: "ollama:llama3".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
        }
    }
}

/// Screen/caret position reported by the accessibility layer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Payload for `optimize_prompt` IPC command.
#[derive(Debug, Clone, Deserialize)]
pub struct OptimizeRequest {
    pub raw: String,
    pub framework: String,
    pub model: String,
    pub context_id: Option<String>,
}

/// Final result emitted on `opt_done`.
#[derive(Debug, Clone, Serialize)]
pub struct OptimizeResult {
    pub optimized: String,
    pub score: u32,
    pub diff: String,
    pub tokens: usize,
    pub session_id: String,
}

/// Result of in-place replacement.
#[derive(Debug, Clone, Serialize)]
pub struct ReplaceResult {
    pub success: bool,
    pub fallback: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureResult {
    pub text: String,
    pub position: Position,
}

/// Normalized error code surfaced to the UI (see API Design §4.1).
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<String>,
}

impl ApiError {
    pub fn provider_unreachable(msg: impl Into<String>) -> Self {
        Self { code: "PROVIDER_UNREACHABLE".into(), message: msg.into(), details: None }
    }
    pub fn replacement_failed(msg: impl Into<String>) -> Self {
        Self { code: "REPLACEMENT_FAILED".into(), message: msg.into(), details: None }
    }
    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self { code: "PERMISSION_DENIED".into(), message: msg.into(), details: None }
    }
    pub fn pii_blocked() -> Self {
        Self {
            code: "PII_BLOCKED".into(),
            message: "Sensitive data detected; cloud routing blocked.".into(),
            details: None,
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { code: "INTERNAL".into(), message: msg.into(), details: None }
    }
}
