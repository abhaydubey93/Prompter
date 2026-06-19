//! OpenAI-compatible adapter (use-case §13).
//!
//! Drives OpenAI, LM Studio, llama.cpp, OpenRouter, NVIDIA NIM, Mistral,
//! Groq, Together, and any custom OpenAI-compatible endpoint. All speak
//! `POST /v1/chat/completions` (SSE when `stream:true`) and `GET /v1/models`.
//!
//! Auth: optional `Bearer <key>` header (local servers like LM Studio/Ollama's
//! OpenAI shim usually need none; cloud providers always do).

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use super::{ChunkStream, LLMAdapter, ProviderError};
use crate::types::{ChatParams, Message, ModelInfo};

pub struct OpenAiCompatAdapter {
    base: String,
    api_key: Option<String>,
    client: Client,
}

impl OpenAiCompatAdapter {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        let base = base_url.trim_end_matches('/').to_string();
        Self {
            base,
            api_key,
            client: Client::new(),
        }
    }

    fn auth(&self) -> Option<String> {
        self.api_key
            .as_ref()
            .map(|k| format!("Bearer {k}"))
    }

    fn endpoint(&self, path: &str) -> String {
        // If base already ends with /v1, don't double it.
        if self.base.ends_with("/v1") {
            format!("{}{path}", self.base)
        } else {
            format!("{}/v1{path}", self.base)
        }
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}
#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

#[async_trait]
impl LLMAdapter for OpenAiCompatAdapter {
    fn id(&self) -> &str {
        "openai_compat"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let mut req = self.client.get(self.endpoint("/models"));
        if let Some(h) = self.auth() {
            req = req.header("Authorization", h);
        }
        let resp = req.send().await.map_err(|e| ProviderError::Unreachable(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Status(resp.status().as_u16(), "list models".into()));
        }
        let body: ModelsResponse =
            resp.json().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
        Ok(body
            .data
            .into_iter()
            .map(|m| ModelInfo { id: m.id.clone(), name: m.id })
            .collect())
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        params: ChatParams,
    ) -> Result<ChunkStream, ProviderError> {
        let body = json!({
            "model": params.model,
            "messages": messages.iter().map(|m| json!({
                "role": m.role,
                "content": m.content,
            })).collect::<Vec<_>>(),
            "stream": true,
            "temperature": params.temperature,
            "max_tokens": params.max_tokens,
        });

        let mut req = self
            .client
            .post(self.endpoint("/chat/completions"))
            .json(&body);
        if let Some(h) = self.auth() {
            req = req.header("Authorization", h);
        }

        let resp = req.send().await.map_err(|e| ProviderError::Unreachable(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, text));
        }

        let mut byte_stream = resp.bytes_stream();
        let stream = async_stream::stream! {
            let mut buf: Vec<u8> = Vec::new();
            while let Some(chunk_res) = byte_stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buf.extend_from_slice(&bytes);
                        // SSE: events separated by \n\n; lines start with "data: ".
                        loop {
                            let Some(pos) = buf.iter().position(|b| *b == b'\n') else {
                                break;
                            };
                            let line: Vec<u8> = buf.drain(..=pos).collect();
                            let line_str = String::from_utf8_lossy(&line).trim().to_string();
                            if line_str.is_empty() { continue; }
                            let Some(payload) = line_str.strip_prefix("data:") else { continue; };
                            let payload = payload.trim();
                            if payload == "[DONE]" { return; }
                            match serde_json::from_str::<StreamChunk>(payload) {
                                Ok(c) => {
                                    if let Some(delta) = c.choices.first().and_then(|ch| ch.delta.as_ref()) {
                                        if let Some(content) = &delta.content {
                                            if !content.is_empty() {
                                                yield Ok(content.clone());
                                            }
                                        }
                                    }
                                    if c.choices.first().map_or(false, |ch| ch.finish_reason.is_some()) {
                                        return;
                                    }
                                }
                                Err(e) => {
                                    yield Err(ProviderError::Parse(format!(
                                        "bad SSE line: {e}: {payload}"
                                    )));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(ProviderError::Transport(e.to_string()));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> bool {
        let mut req = self.client.get(self.endpoint("/models"));
        if let Some(h) = self.auth() {
            req = req.header("Authorization", h);
        }
        req.send().await.map(|r| r.status().is_success()).unwrap_or(false)
    }
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    #[serde(default)]
    delta: Option<Delta>,
    #[serde(default)]
    finish_reason: Option<String>,
}
#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_no_double_v1() {
        let a = OpenAiCompatAdapter::new("https://api.openai.com", None);
        assert_eq!(a.endpoint("/models"), "https://api.openai.com/v1/models");
        let b = OpenAiCompatAdapter::new("https://api.openai.com/v1", None);
        assert_eq!(b.endpoint("/models"), "https://api.openai.com/v1/models");
    }

    #[test]
    fn test_auth_header() {
        let with_key = OpenAiCompatAdapter::new("x", Some("sk-abc".into()));
        assert_eq!(with_key.auth(), Some("Bearer sk-abc".into()));
        let no_key = OpenAiCompatAdapter::new("x", None);
        assert_eq!(no_key.auth(), None);
    }
}
