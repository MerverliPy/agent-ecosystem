//! llama.cpp server adapter — OpenAI-compatible `/v1/chat/completions` endpoint.
//! On Apple Silicon this is the TurboFieldfare-compatible Metal path (the server is
//! launched with Metal enabled); the adapter itself only needs a base URL.

use super::{Backend, ChatMsg, Generation, ModelInfo, RuntimeError};

pub const DEFAULT_LLAMACPP_URL: &str = "http://127.0.0.1:8080";

pub struct LlamaCppBackend {
    base: String,
}

impl LlamaCppBackend {
    pub fn new(base: Option<String>) -> Self {
        Self {
            base: base.unwrap_or_else(|| DEFAULT_LLAMACPP_URL.to_string()),
        }
    }
}

impl Backend for LlamaCppBackend {
    fn name(&self) -> &'static str {
        "llama.cpp"
    }

    fn base_url(&self) -> &str {
        &self.base
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        // llama.cpp serves a single model per process; /v1/models lists it.
        let resp: serde_json::Value = ureq::get(&format!("{}/v1/models", self.base))
            .timeout(std::time::Duration::from_secs(5))
            .call()?
            .into_json()
            .map_err(|e| RuntimeError::Parse(e.to_string()))?;
        let models = resp
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let name = m.get("id")?.as_str()?.to_string();
                        Some(ModelInfo {
                            name,
                            size_bytes: m.get("size").and_then(|s| s.as_u64()),
                            family: None,
                            parameter_size: m.get("detail").and_then(|d| d.as_str()).map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    fn chat(&self, model: &str, system: &str, messages: &[ChatMsg]) -> Result<Generation, RuntimeError> {
        let mut all: Vec<serde_json::Value> = Vec::new();
        if !system.is_empty() {
            all.push(serde_json::json!({ "role": "system", "content": system }));
        }
        for m in messages {
            all.push(serde_json::json!({ "role": m.role, "content": m.content }));
        }
        let body = serde_json::json!({
            "model": model,
            "messages": all,
            "stream": false,
        });
        let resp: serde_json::Value = ureq::post(&format!("{}/v1/chat/completions", self.base))
            .timeout(std::time::Duration::from_secs(300))
            .send_json(body)
            .map_err(RuntimeError::from)?
            .into_json()
            .map_err(|e| RuntimeError::Parse(e.to_string()))?;

        let text = resp
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();
        let tokens_per_sec = resp
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|v| v.as_u64())
            .map(|t| t as f64 / 1.0)
            .or(Some(0.0));
        let duration_ms = 0;
        Ok(Generation {
            text,
            model: model.to_string(),
            duration_ms,
            tokens_per_sec: tokens_per_sec.filter(|t| *t > 0.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_server::serve;
    use std::sync::Arc;

    #[test]
    fn llama_cpp_chat_parses_openai_shape() {
        let server = serve(Arc::new(|method, path| {
            if method == "POST" && path == "/v1/chat/completions" {
                (
                    200,
                    r#"{"choices":[{"message":{"content":"Metal works"}}],"usage":{"completion_tokens":7}}"#.into(),
                )
            } else {
                (404, "{}".into())
            }
        }));
        let backend = LlamaCppBackend::new(Some(server));
        let gen = backend
            .chat("model", "", &[ChatMsg { role: "user".into(), content: "hi".into() }])
            .unwrap();
        assert_eq!(gen.text, "Metal works");
        assert_eq!(gen.tokens_per_sec, Some(7.0));
    }

    #[test]
    fn llama_cpp_lists_models() {
        let server = serve(Arc::new(|_m, p| {
            if p == "/v1/models" {
                (200, r#"{"data":[{"id":"qwen2.5:7b","size":4000000000}]}"#.into())
            } else {
                (404, "{}".into())
            }
        }));
        let backend = LlamaCppBackend::new(Some(server));
        let models = backend.list_models().unwrap();
        assert_eq!(models[0].name, "qwen2.5:7b");
    }
}
