//! Pluggable LLM provider layer.
//!
//! All providers implement `LLMAdapter`. The Optimization Engine talks only to
//! the trait, so adding a provider is a self-contained module. See
//! `design_docs/05_API_Design.md` §3.

pub mod ollama;
pub mod openai;

use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;
use thiserror::Error;

use crate::types::{ChatParams, Message, ModelInfo};

/// A boxed async stream of text chunks (or errors).
/// (Spec §3.1 names this type `ChatStream`; `ChunkStream` kept as alias.)
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

/// Build an adapter for a `provider:model` selector string.
/// Returns `None` for unknown providers (UI shows them as unavailable).
pub fn build(selector: &str, ollama_url: &str) -> Option<Box<dyn LLMAdapter>> {
    let (provider, _model) = match selector.split_once(':') {
        Some((p, m)) => (p, Some(m)),
        None => (selector, None),
    };
    match provider {
        "ollama" => Some(Box::new(ollama::OllamaAdapter::new(ollama_url))),
        "openai" => Some(Box::new(openai::OpenAiAdapter::new())),
        _ => None,
    }
}
