//! OpenAI adapter — STUB for the Core MVP.
//!
//! The trait surface compiles; cloud providers + the OS-keychain vault land in
//! a later pass (see spec §2 "Out of scope"). `stream_chat` returns
//! `ProviderError::Unimplemented` so the UI can mark it unavailable.

use async_trait::async_trait;

use super::{ChunkStream, LLMAdapter, ProviderError};
use crate::types::{ChatParams, Message, ModelInfo};

pub struct OpenAiAdapter;

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LLMAdapter for OpenAiAdapter {
    fn id(&self) -> &str {
        "openai"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Err(ProviderError::Unimplemented(
            "OpenAI adapter not enabled in MVP (no cloud key vault yet)".into(),
        ))
    }

    async fn stream_chat(
        &self,
        _messages: Vec<Message>,
        _params: ChatParams,
    ) -> Result<ChunkStream, ProviderError> {
        Err(ProviderError::Unimplemented(
            "OpenAI adapter not enabled in MVP".into(),
        ))
    }

    async fn health_check(&self) -> bool {
        false
    }
}
