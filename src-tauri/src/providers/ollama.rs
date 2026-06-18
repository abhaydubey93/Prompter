//! Ollama adapter — native `/api/chat` (NDJSON streaming) + `/api/tags`.
//!
//! Endpoint default: `http://localhost:11434`. No auth. Streaming returns one
//! JSON object per line; `message.content` carries each token chunk and the
//! final line has `done: true`.

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use super::{ChunkStream, LLMAdapter, ProviderError};
use crate::types::{ChatParams, Message, ModelInfo};

pub struct OllamaAdapter {
    base: String,
    client: Client,
}

impl OllamaAdapter {
    pub fn new(base_url: &str) -> Self {
        let base = base_url.trim_end_matches('/').to_string();
        Self { base, client: Client::new() }
    }
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagsModel>,
}
#[derive(Deserialize)]
struct TagsModel {
    name: String,
}
#[derive(Deserialize)]
struct ChatChunk {
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}
#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

#[async_trait]
impl LLMAdapter for OllamaAdapter {
    fn id(&self) -> &str {
        "ollama"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.base))
            .send()
            .await
            .map_err(|e| ProviderError::Unreachable(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Status(
                resp.status().as_u16(),
                "failed to list models".into(),
            ));
        }
        let body: TagsResponse =
            resp.json().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
        Ok(body
            .models
            .into_iter()
            .map(|m| ModelInfo { id: m.name.clone(), name: m.name })
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
            "options": {
                "temperature": params.temperature,
                "num_predict": params.max_tokens,
            }
        });

        let resp = self
            .client
            .post(format!("{}/api/chat", self.base))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Unreachable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, text));
        }

        // Convert the NDJSON byte stream into a Stream<Result<String>>.
        let mut byte_stream = resp.bytes_stream();
        let stream = async_stream::stream! {
            let mut buf: Vec<u8> = Vec::new();
            while let Some(chunk_res) = byte_stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buf.extend_from_slice(&bytes);
                        // split on newlines — each line is a complete JSON object
                        loop {
                            let Some(pos) = buf.iter().position(|b| *b == b'\n') else {
                                break;
                            };
                            let line: Vec<u8> = buf.drain(..=pos).collect();
                            let line_str = String::from_utf8_lossy(&line).trim().to_string();
                            if line_str.is_empty() { continue; }
                            match serde_json::from_str::<ChatChunk>(&line_str) {
                                Ok(c) => {
                                    if let Some(err) = &c.error {
                                        yield Err(ProviderError::Parse(err.clone()));
                                        return;
                                    }
                                    if let Some(msg) = &c.message {
                                        if !msg.content.is_empty() {
                                            yield Ok(msg.content.clone());
                                        }
                                    }
                                    if c.done { return; }
                                }
                                Err(e) => {
                                    yield Err(ProviderError::Parse(format!(
                                        "bad NDJSON line: {e}: {line_str}"
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
        self.client
            .get(format!("{}/api/tags", self.base))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}
