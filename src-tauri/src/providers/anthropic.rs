//! Anthropic adapter — native `/v1/messages` (use-case §13).
//!
//! Headers: `x-api-key` + `anthropic-version: 2023-06-01`.
//! Streaming: SSE `content_block_delta` events carry `delta.text`.
//! Models: no public list endpoint → return the known Claude family.

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use super::{ChunkStream, LLMAdapter, ProviderError};
use crate::types::{ChatParams, Message, ModelInfo};

const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicAdapter {
    base: String,
    api_key: String,
    client: Client,
}

impl AnthropicAdapter {
    pub fn new(base_url: &str, api_key: String) -> Self {
        Self {
            base: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: Client::new(),
        }
    }
}

/// Known Claude model family (Anthropic exposes no list endpoint).
fn known_models() -> Vec<ModelInfo> {
    ["claude-sonnet-4-20250514", "claude-3-7-sonnet-20250219", "claude-3-5-haiku-20241022"]
        .into_iter()
        .map(|m| ModelInfo { id: m.to_string(), name: m.to_string() })
        .collect()
}

#[async_trait]
impl LLMAdapter for AnthropicAdapter {
    fn id(&self) -> &str {
        "anthropic"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(known_models())
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        params: ChatParams,
    ) -> Result<ChunkStream, ProviderError> {
        // Anthropic splits system message from the conversation.
        let (system, convo): (String, Vec<Message>) = {
            let mut sys = String::new();
            let mut conv = Vec::new();
            for m in messages {
                if m.role == "system" {
                    if !sys.is_empty() { sys.push_str("\n\n"); }
                    sys.push_str(&m.content);
                } else {
                    conv.push(m);
                }
            }
            (sys, conv)
        };

        let mut body = json!({
            "model": params.model,
            "messages": convo.iter().map(|m| json!({
                "role": m.role,
                "content": m.content,
            })).collect::<Vec<_>>(),
            "stream": true,
            "max_tokens": params.max_tokens,
            "temperature": params.temperature,
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }

        let resp = self
            .client
            .post(format!("{}/v1/messages", self.base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Unreachable(e.to_string()))?;

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
                        loop {
                            let Some(pos) = buf.iter().position(|b| *b == b'\n') else { break; };
                            let line: Vec<u8> = buf.drain(..=pos).collect();
                            let line_str = String::from_utf8_lossy(&line).trim().to_string();
                            if line_str.is_empty() { continue; }
                            let Some(payload) = line_str.strip_prefix("data:") else { continue; };
                            let payload = payload.trim();
                            // event type is on the preceding "event:" line; we
                            // only care about content_block_delta / message_stop.
                            match serde_json::from_str::<AnthropicEvent>(payload) {
                                Ok(ev) => {
                                    if let Some(delta) = ev.delta.as_ref() {
                                        if delta.delta_type == "text_delta" {
                                            if let Some(text) = &delta.text {
                                                if !text.is_empty() {
                                                    yield Ok(text.clone());
                                                }
                                            }
                                        }
                                    }
                                    if ev.event_type.as_deref() == Some("message_stop") {
                                        return;
                                    }
                                }
                                Err(_) => { /* ignore non-JSON event lines */ }
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
        // Anthropic has no cheap GET; probe a minimal models-style call by
        // hitting the messages endpoint with HEAD-like weight (1 token).
        // For health, just check TCP+TLS reachability of the API host.
        self.client
            .get(format!("{}/v1/models", self.base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .await
            .map(|r| r.status().is_success() || r.status().as_u16() == 404)
            .unwrap_or(false)
    }
}

#[derive(Deserialize)]
struct AnthropicEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
}
#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type", default)]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
}
