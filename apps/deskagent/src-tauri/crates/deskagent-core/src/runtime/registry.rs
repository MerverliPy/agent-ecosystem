//! Model registry: pick the backend + model, remember the choice, and expose the
//! catalog for the BenchKit-powered model picker (Phase 6 Task 2 wires the dataset).

use super::{Backend, ModelInfo, RuntimeError};
use crate::store::MemoryStore;

/// Which backend to prefer when several are configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Ollama,
    LlamaCpp,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::Ollama => "ollama",
            BackendKind::LlamaCpp => "llama.cpp",
        }
    }
}

pub struct ModelRegistry {
    pub backend: Box<dyn Backend>,
    pub kind: BackendKind,
}

impl ModelRegistry {
    pub fn new(kind: BackendKind, base_url: Option<String>) -> Self {
        let backend: Box<dyn Backend> = match kind {
            BackendKind::Ollama => Box::new(super::ollama::OllamaBackend::new(base_url)),
            BackendKind::LlamaCpp => Box::new(super::llama_cpp::LlamaCppBackend::new(base_url)),
        };
        Self { backend, kind }
    }

    pub fn list(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        self.backend.list_models()
    }

    pub fn chat(&self, model: &str, system: &str, messages: &[super::ChatMsg]) -> Result<super::Generation, RuntimeError> {
        self.backend.chat(model, system, messages)
    }

    /// Persist the chosen model in the store meta so the picker restores it.
    pub fn remember_choice(&self, store: &MemoryStore, model: &str) -> rusqlite::Result<()> {
        store.set_meta("runtime.backend", self.kind.as_str())?;
        store.set_meta("runtime.model", model)
    }

    pub fn remembered_choice(store: &MemoryStore) -> Option<(BackendKind, String)> {
        let kind = store.meta("runtime.backend").ok()??;
        let model = store.meta("runtime.model").ok()??;
        let kind = match kind.as_str() {
            "llama.cpp" => BackendKind::LlamaCpp,
            _ => BackendKind::Ollama,
        };
        Some((kind, model))
    }

    /// Metal hint for llama.cpp on Apple Silicon (TurboFieldfare-compatible): the
    /// server should be launched with Metal enabled. Pure metadata for P0.
    pub fn metal_hint(kind: BackendKind, platform: &str) -> bool {
        kind == BackendKind::LlamaCpp && platform == "macos"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_server::serve;
    use crate::store::{MemoryStore, StoreConfig};
    use std::sync::Arc;

    #[test]
    fn registry_lists_and_chats() {
        let server = serve(Arc::new(|method, path| {
            if method == "GET" && path == "/api/tags" {
                (200, r#"{"models":[{"name":"llama3.2:3b","size":1}]}"#.into())
            } else if method == "POST" && path == "/api/chat" {
                (200, r#"{"message":{"content":"ok"},"total_duration":1000000}"#.into())
            } else {
                (404, "{}".into())
            }
        }));
        let reg = ModelRegistry::new(BackendKind::Ollama, Some(server));
        assert_eq!(reg.list().unwrap()[0].name, "llama3.2:3b");
        let gen = reg.chat("llama3.2:3b", "", &[]).unwrap();
        assert_eq!(gen.text, "ok");
    }

    #[test]
    fn remember_choice_roundtrip() {
        let store = MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap();
        let reg = ModelRegistry::new(BackendKind::Ollama, None);
        reg.remember_choice(&store, "llama3.2:3b").unwrap();
        let (kind, model) = ModelRegistry::remembered_choice(&store).unwrap();
        assert_eq!(kind, BackendKind::Ollama);
        assert_eq!(model, "llama3.2:3b");
    }

    #[test]
    fn metal_hint_only_for_llama_cpp_on_macos() {
        assert!(ModelRegistry::metal_hint(BackendKind::LlamaCpp, "macos"));
        assert!(!ModelRegistry::metal_hint(BackendKind::LlamaCpp, "linux"));
        assert!(!ModelRegistry::metal_hint(BackendKind::Ollama, "macos"));
    }

    /// Live smoke test against a real local Ollama — skipped by default.
    /// Run with: cargo test -p deskagent-core -- --ignored ollama_live
    #[test]
    #[ignore = "requires a running local Ollama"]
    fn ollama_live_chat() {
        let reg = ModelRegistry::new(BackendKind::Ollama, None);
        let models = reg.list().expect("ollama reachable");
        assert!(!models.is_empty(), "ollama has no models");
        let gen = reg
            .chat(
                &models[0].name,
                "Reply with exactly one word.",
                &[super::super::ChatMsg {
                    role: "user".into(),
                    content: "Say hello".into(),
                }],
            )
            .expect("chat completes");
        println!("LIVE ollama {} -> {:?}", models[0].name, gen.text);
        assert!(!gen.text.is_empty());
    }
}
