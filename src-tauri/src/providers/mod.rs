//! Pluggable LLM provider layer (spec §3.2, use-case §13).
//!
//! Every provider implements `LLMAdapter`. The engine talks only to the trait,
//! so adding a provider is one module + one DB row (FR-L1..L3).
//!
//! Kinds (use-case §13 matrix):
//! - `ollama`        — native `/api/chat` + `/api/tags` (default local)
//! - `openai_compat` — `/v1/chat/completions` + `/v1/models` SSE streaming.
//!                     Covers OpenAI, LM Studio, llama.cpp, OpenRouter,
//!                     NVIDIA NIM, Mistral, Groq, Together, Custom.
//! - `anthropic`     — native `/v1/messages` + `x-api-key` header.
//! - `gemini`        — native Generate Content + `?key=` query auth.

pub mod anthropic;
pub mod gemini;
pub mod keys;
pub mod ollama;
pub mod openai_compat;

use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;
use thiserror::Error;

use crate::types::{ChatParams, Message, ModelInfo, ProviderConfig};

/// A boxed async stream of text chunks (or errors). Spec §3.1 names this
/// `ChatStream`; `ChunkStream` kept as alias for back-compat.
pub type ChatStream =
    Pin<Box<dyn Stream<Item = Result<String, ProviderError>> + Send>>;
pub type ChunkStream = ChatStream;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider unreachable: {0}")]
    Unreachable(String),
    #[error("provider returned an error status {0}: {1}")]
    Status(u16, String),
    #[error("stream parse error: {0}")]
    Parse(String),
    #[error("not implemented for this provider: {0}")]
    Unimplemented(String),
    #[error("io/transport error: {0}")]
    Transport(String),
    #[error("missing API key for provider '{0}'")]
    MissingKey(String),
}

#[async_trait]
pub trait LLMAdapter: Send + Sync {
    fn id(&self) -> &str;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        params: ChatParams,
    ) -> Result<ChunkStream, ProviderError>;
    /// Quick liveness probe (does not block the UI if false).
    async fn health_check(&self) -> bool;
}

/// Construct the concrete adapter for a `ProviderConfig`. Reads the API key
/// from the OS keychain when `api_key_slot` is set.
pub fn build_adapter(cfg: &ProviderConfig) -> Result<Box<dyn LLMAdapter>, ProviderError> {
    let api_key = cfg
        .api_key_slot
        .as_deref()
        .and_then(|_| keys::get(&cfg.id));
    match cfg.kind.as_str() {
        "ollama" => Ok(Box::new(ollama::OllamaAdapter::new(&cfg.base_url))),
        "openai_compat" => Ok(Box::new(openai_compat::OpenAiCompatAdapter::new(
            &cfg.base_url,
            api_key,
        ))),
        "anthropic" => {
            let key = api_key.ok_or_else(|| ProviderError::MissingKey(cfg.id.clone()))?;
            Ok(Box::new(anthropic::AnthropicAdapter::new(&cfg.base_url, key)))
        }
        "gemini" => {
            let key = api_key.ok_or_else(|| ProviderError::MissingKey(cfg.id.clone()))?;
            Ok(Box::new(gemini::GeminiAdapter::new(&cfg.base_url, key)))
        }
        other => Err(ProviderError::Unimplemented(format!(
            "unknown provider kind: {other}"
        ))),
    }
}
