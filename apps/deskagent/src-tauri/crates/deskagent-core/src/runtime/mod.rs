//! Runtime layer: pluggable local-model backends (Ollama, llama.cpp server) with a
//! model registry. Blocking HTTP via `ureq` (MIT) — fits the SQLite-first core style.
//! Metal path on Apple Silicon is the default llama.cpp configuration on macOS
//! (TurboFieldfare-compatible); the adapter itself is URL-based and platform-agnostic.

pub mod llama_cpp;
pub mod ollama;
pub mod registry;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub text: String,
    pub model: String,
    pub duration_ms: u64,
    pub tokens_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_size: Option<String>,
}

#[derive(Debug)]
pub enum RuntimeError {
    Http(u16),
    Io(String),
    Parse(String),
    NotConfigured(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Http(code) => write!(f, "backend HTTP error {code}"),
            RuntimeError::Io(msg) => write!(f, "backend I/O error: {msg}"),
            RuntimeError::Parse(msg) => write!(f, "backend parse error: {msg}"),
            RuntimeError::NotConfigured(msg) => write!(f, "backend not configured: {msg}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<std::io::Error> for RuntimeError {
    fn from(e: std::io::Error) -> Self {
        RuntimeError::Io(e.to_string())
    }
}

impl From<ureq::Error> for RuntimeError {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(code, _) => RuntimeError::Http(code),
            other => RuntimeError::Io(other.to_string()),
        }
    }
}

pub trait Backend: Send + Sync {
    /// Stable identifier, e.g. "ollama" or "llama.cpp".
    fn name(&self) -> &'static str;
    fn base_url(&self) -> &str;
    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError>;
    fn chat(&self, model: &str, system: &str, messages: &[ChatMsg]) -> Result<Generation, RuntimeError>;
}

#[cfg(test)]
pub mod test_server {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    /// Tiny canned HTTP server for deterministic offline tests.
    pub fn serve(routes: Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 8192];
                let Ok(n) = stream.read(&mut buf) else { continue };
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let method = req.split_whitespace().next().unwrap_or("GET").to_string();
                let (code, body) = routes(&method, &path);
                let response = format!(
                    "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }
}
