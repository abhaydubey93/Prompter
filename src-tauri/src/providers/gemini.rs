//! Google Gemini adapter — native Generative Language API (use-case §13).
//!
//! Auth: API key in `?key=` query param.
//! Streaming: `streamGenerateContent?alt=sse` → SSE lines with
//! `candidates[0].content.parts[0].text`.
//! Models: `GET /v1beta/models?key=`.

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use super::{ChunkStream, LLMAdapter, ProviderError};
use crate::types::{ChatParams, Message, ModelInfo};

pub struct GeminiAdapter {
    base: String,
    api_key: String,
    client: Client,
}

impl GeminiAdapter {
    pub fn new(base_url: &str, api_key: String) -> Self {
        Self {
            base: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    models: Vec<GeminiModel>,
}
#[derive(Deserialize)]
struct GeminiModel {
    name: String,
}

/// Gemini roles are "user"/"model" (not "assistant").
fn to_gemini_role(role: &str) -> &str {
    match role {
        "assistant" => "model",
        "system" => "user",
        other => other,
    }
}

#[async_trait]
impl LLMAdapter for GeminiAdapter {
    fn id(&self) -> &str {
        "gemini"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/v1beta/models", self.base))
            .query(&[("key", &self.api_key), ("pageSize", &"100".to_string())])
            .send()
            .await
            .map_err(|e| ProviderError::Unreachable(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Status(resp.status().as_u16(), "list models".into()));
        }
        let body: ModelsResponse =
            resp.json().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
        Ok(body
            .models
            .into_iter()
            .filter_map(|m| {
                // Strip "models/" prefix from name like "models/gemini-2.0-flash".
                let id = m.name.strip_prefix("models/").unwrap_or(&m.name).to_string();
                if id.starts_with("gemini-") {
                    Some(ModelInfo { id: id.clone(), name: id })
                } else {
                    None
                }
            })
            .collect())
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        params: ChatParams,
    ) -> Result<ChunkStream, ProviderError> {
        let contents: Vec<_> = messages
            .iter()
            .map(|m| {
                json!({
                    "role": to_gemini_role(&m.role),
                    "parts": [{ "text": m.content }],
                })
            })
            .collect();
        let body = json!({
            "contents": contents,
            "generationConfig": {
                "temperature": params.temperature,
                "maxOutputTokens": params.max_tokens,
            }
        });

        // model param is already bare id ("gemini-2.0-flash"); not "provider:model".
        let model = params.model.rsplit(':').next().unwrap_or(&params.model);
        let url = format!(
            "{}/v1beta/models/{model}:streamGenerateContent",
            self.base
        );

        let resp = self
            .client
            .post(&url)
            .query(&[("alt", &"sse".to_string()), ("key", &self.api_key)])
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
                            match serde_json::from_str::<GeminiChunk>(payload) {
                                Ok(c) => {
                                    if let Some(text) = c.candidates.first()
                                        .and_then(|cand| cand.content.as_ref())
                                        .and_then(|content| content.parts.first())
                                        .and_then(|p| p.text.as_ref())
                                    {
                                        if !text.is_empty() {
                                            yield Ok(text.clone());
                                        }
                                    }
                                }
                                Err(e) => {
                                    yield Err(ProviderError::Parse(format!(
                                        "bad Gemini SSE line: {e}: {payload}"
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
            .get(format!("{}/v1beta/models", self.base))
            .query(&[("key", &self.api_key), ("pageSize", &"1".to_string())])
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[derive(Deserialize)]
struct GeminiChunk {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}
#[derive(Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContent>,
}
#[derive(Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}
#[derive(Deserialize)]
struct GeminiPart {
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_mapping() {
        assert_eq!(to_gemini_role("user"), "user");
        assert_eq!(to_gemini_role("assistant"), "model");
        assert_eq!(to_gemini_role("system"), "user");
    }
}
