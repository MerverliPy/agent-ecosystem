//! Ollama backend adapter — talks to a local `ollama serve` (default http://127.0.0.1:11434).
//! Native `/api/tags` + `/api/chat` endpoints (stream: false).

use super::{Backend, ChatMsg, Generation, ModelInfo, RuntimeError};

pub const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";

pub struct OllamaBackend {
    base: String,
}

impl OllamaBackend {
    pub fn new(base: Option<String>) -> Self {
        Self {
            base: base.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
        }
    }
}

impl Backend for OllamaBackend {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn base_url(&self) -> &str {
        &self.base
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        let resp: serde_json::Value = ureq::get(&format!("{}/api/tags", self.base))
            .timeout(std::time::Duration::from_secs(5))
            .call()?
            .into_json()
            .map_err(|e| RuntimeError::Parse(e.to_string()))?;
        let models = resp
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let name = m.get("name")?.as_str()?.to_string();
                        Some(ModelInfo {
                            name,
                            size_bytes: m.get("size").and_then(|s| s.as_u64()),
                            family: m.get("details").and_then(|d| d.get("family")).and_then(|f| f.as_str()).map(String::from),
                            parameter_size: m.get("details").and_then(|d| d.get("parameter_size")).and_then(|f| f.as_str()).map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    fn chat(&self, model: &str, system: &str, messages: &[ChatMsg]) -> Result<Generation, RuntimeError> {
        let mut ollama_msgs: Vec<serde_json::Value> = Vec::new();
        if !system.is_empty() {
            ollama_msgs.push(serde_json::json!({ "role": "system", "content": system }));
        }
        for m in messages {
            ollama_msgs.push(serde_json::json!({ "role": m.role, "content": m.content }));
        }
        let body = serde_json::json!({
            "model": model,
            "messages": ollama_msgs,
            "stream": false,
        });
        let resp: serde_json::Value = ureq::post(&format!("{}/api/chat", self.base))
            .timeout(std::time::Duration::from_secs(300))
            .send_json(body)
            .map_err(RuntimeError::from)?
            .into_json()
            .map_err(|e| RuntimeError::Parse(e.to_string()))?;

        let text = resp
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();
        let duration_ms = resp
            .get("total_duration")
            .and_then(|d| d.as_u64())
            .map(|ns| ns / 1_000_000)
            .unwrap_or(0);
        let tokens_per_sec = match (
            resp.get("eval_count").and_then(|v| v.as_u64()),
            resp.get("eval_duration").and_then(|v| v.as_u64()),
        ) {
            (Some(tokens), Some(ns)) if ns > 0 => Some(tokens as f64 / (ns as f64 / 1e9)),
            _ => None,
        };
        Ok(Generation {
            text,
            model: model.to_string(),
            duration_ms,
            tokens_per_sec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_server::serve;
    use std::sync::Arc;

    #[test]
    fn ollama_lists_models() {
        let server = serve(Arc::new(|_m, p| {
            if p == "/api/tags" {
                (
                    200,
                    r#"{"models":[{"name":"llama3.2:3b","size":2010000000,"details":{"family":"llama","parameter_size":"3.2B"}}]}"#.into(),
                )
            } else {
                (404, "{}".into())
            }
        }));
        let backend = OllamaBackend::new(Some(server));
        let models = backend.list_models().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "llama3.2:3b");
        assert_eq!(models[0].family.as_deref(), Some("llama"));
        assert_eq!(models[0].parameter_size.as_deref(), Some("3.2B"));
    }

    #[test]
    fn ollama_chat_parses_generation() {
        let server = serve(Arc::new(|method, path| {
            if method == "POST" && path == "/api/chat" {
                (
                    200,
                    r#"{"message":{"content":"Hello from llama"},"total_duration":1000000000,"eval_count":20,"eval_duration":1000000000}"#.into(),
                )
            } else {
                (404, "{}".into())
            }
        }));
        let backend = OllamaBackend::new(Some(server));
        let gen = backend
            .chat("llama3.2:3b", "be brief", &[ChatMsg { role: "user".into(), content: "hi".into() }])
            .unwrap();
        assert_eq!(gen.text, "Hello from llama");
        assert_eq!(gen.duration_ms, 1000);
        assert!((gen.tokens_per_sec.unwrap() - 20.0).abs() < 1e-6);
    }

    #[test]
    fn http_error_surfaces_as_runtime_error() {
        let server = serve(Arc::new(|_m, _p| (500, "{}".into())));
        let backend = OllamaBackend::new(Some(server));
        assert!(matches!(
            backend.list_models(),
            Err(RuntimeError::Http(500))
        ));
    }
}
