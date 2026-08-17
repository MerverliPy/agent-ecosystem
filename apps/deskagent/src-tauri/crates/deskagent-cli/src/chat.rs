//! CLI chat engine: capture the user turn, run the extraction pass on schedule, then
//! complete the assistant turn through the remembered/overridden backend — with the
//! DEC-0005 deterministic fallback when the runtime is offline.
//!
//! Mirrors the Tauri shell's `session_append` + `chat_complete` commands exactly
//! (same core calls, same fallback text shape). `deskagent-core` is untouched; this
//! is CLI-side wiring only.

use deskagent_core::capture::{
    capture_turn, run_extraction_pass, turns_since_pass, DEFAULT_TURNS_PER_PASS, MAX_MEMORIES_PER_PASS,
};
use deskagent_core::conversation::{
    attach_assistant_with_citations, build_chat_context, CONTEXT_HISTORY_KEEP,
};
use deskagent_core::runtime::registry::{BackendKind, ModelRegistry};
use deskagent_core::runtime::ChatMsg;
use deskagent_core::sessions::get_session;
use deskagent_core::store::{MemoryScope, MemoryStore, ScopeType};

/// Outcome of one full user turn (capture + extraction + assistant completion).
#[derive(Debug, Clone)]
pub struct TurnOutcome {
    pub session_id: String,
    pub model: String,
    pub reply: String,
    pub citations: usize,
    pub offline: bool,
    pub used_chars: usize,
    pub truncated: bool,
    pub extraction_proposals: usize,
}

/// The assistant-completion half of a turn (used directly by headless commands too).
#[derive(Debug, Clone)]
pub struct Completion {
    pub model: String,
    pub reply: String,
    pub citations: usize,
    pub offline: bool,
    pub used_chars: usize,
    pub truncated: bool,
}

/// Resolve the memory scope for a session (companion or project), like the shell's
/// `scope_for_session` helper.
pub fn scope_for_session(store: &MemoryStore, session_id: &str) -> Result<MemoryScope, String> {
    let session = get_session(store, session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    Ok(match session.project_id {
        Some(pid) => MemoryScope {
            scope_type: ScopeType::Project,
            project_id: Some(pid),
            project_path: None,
        },
        None => MemoryScope {
            scope_type: ScopeType::Companion,
            project_id: None,
            project_path: None,
        },
    })
}

/// One full chat turn: user message (raw episode) → scheduled extraction pass →
/// assistant completion with citations. `backend`/`base_url`/`model` override the
/// remembered choice for this turn (a `model` is also persisted as the new default).
pub fn chat_turn(
    store: &MemoryStore,
    session_id: &str,
    content: &str,
    backend: Option<BackendKind>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<TurnOutcome, String> {
    let scope = scope_for_session(store, session_id)?;
    if capture_turn(store, session_id, "user", content, scope)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("session not found".to_string());
    }

    // Extraction pass every N user turns (default 5, max 20 proposals per pass) —
    // mirrors the shell's `session_append` guard.
    let turns = turns_since_pass(store, session_id).map_err(|e| e.to_string())?;
    let extraction_proposals = if turns >= DEFAULT_TURNS_PER_PASS as i64 {
        run_extraction_pass(store, session_id, DEFAULT_TURNS_PER_PASS, MAX_MEMORIES_PER_PASS)
            .map_err(|e| e.to_string())?
    } else {
        0
    };

    let c = complete_turn(store, session_id, content, backend, base_url, model)?;
    Ok(TurnOutcome {
        session_id: session_id.to_string(),
        model: c.model,
        reply: c.reply,
        citations: c.citations,
        offline: c.offline,
        used_chars: c.used_chars,
        truncated: c.truncated,
        extraction_proposals,
    })
}

/// Complete one assistant turn: context (persona + scoped retrieval, strict budget),
/// truncated history, backend chat, DEC-0005 deterministic fallback when offline.
pub fn complete_turn(
    store: &MemoryStore,
    session_id: &str,
    user_turn: &str,
    backend: Option<BackendKind>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<Completion, String> {
    let (remembered_kind, remembered_model) =
        ModelRegistry::remembered_choice(store).unwrap_or((BackendKind::Ollama, "unknown".to_string()));
    let kind = backend.unwrap_or(remembered_kind);
    // Persist a base_url override so later turns reuse it (the shell reads
    // `runtime.base_url` meta; here we also write it).
    let base = match base_url {
        Some(url) => {
            let _ = store.set_meta("runtime.base_url", &url);
            Some(url)
        }
        None => store.meta("runtime.base_url").ok().flatten(),
    };
    let reg = ModelRegistry::new(kind, base);

    let model_name = match model {
        Some(name) => {
            // Explicit model → also remember it (web-picker parity: `runtime_pick`).
            let _ = reg.remember_choice(store, &name);
            name
        }
        None => match remembered_model.as_str() {
            "unknown" => match reg.list() {
                // No choice remembered yet: auto-pick the first reachable model so a
                // plain `deskagent chat "…"` works on a machine with a local runtime.
                Ok(list) if !list.is_empty() => {
                    let name = list[0].name.clone();
                    let _ = reg.remember_choice(store, &name);
                    name
                }
                _ => "unknown".to_string(),
            },
            _ => remembered_model,
        },
    };

    let ctx = build_chat_context(store, session_id, user_turn, None).map_err(|e| e.to_string())?;
    let history = get_session(store, session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    let msgs: Vec<ChatMsg> = history
        .messages
        .iter()
        .take(CONTEXT_HISTORY_KEEP)
        .map(|m| ChatMsg {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let (reply, offline) = match reg.chat(&model_name, &ctx.system, &msgs) {
        Ok(gen) => (gen.text, false),
        Err(err) => (
            format!("[runtime offline: {err}]\n\nDeterministic fallback:\n\n{user_turn}"),
            true,
        ),
    };
    let citations = ctx.citations.len();

    attach_assistant_with_citations(store, session_id, &reply, ctx.citations)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;

    Ok(Completion {
        model: model_name,
        reply,
        citations,
        offline,
        used_chars: ctx.used_chars,
        truncated: ctx.truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskagent_core::approvals::{decide, list_cards, pending_count};
    use deskagent_core::sessions::{create_session, get_session, list_sessions};
    use deskagent_core::store::{MemoryStore, StoreConfig};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig {
            path: ":memory:".into(),
            encrypt: false,
        })
        .unwrap()
    }

    /// Tiny canned HTTP server for deterministic offline tests (same shape as the
    /// core's `runtime::test_server`).
    fn serve_stub(routes: Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>) -> String {
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

    const DEAD_URL: &str = "http://127.0.0.1:1";

    #[test]
    fn offline_backend_falls_back_deterministic_and_still_attaches() {
        let s = store();
        let sess = create_session(&s, None).unwrap();
        let out = complete_turn(&s, &sess.id, "hello there", None, Some(DEAD_URL.into()), None).unwrap();
        assert!(out.offline, "dead port must be treated as offline");
        assert!(out.reply.contains("Deterministic fallback"));
        assert!(out.reply.contains("hello there"));
        // assistant turn is still stored (DEC-0005: the fallback is the reply)
        let stored = get_session(&s, &sess.id).unwrap().unwrap();
        assert_eq!(stored.messages.last().unwrap().role, "assistant");
        assert!(stored.messages.last().unwrap().content.contains("Deterministic fallback"));
    }

    #[test]
    fn missing_session_is_an_error() {
        let s = store();
        let err = complete_turn(&s, "nope", "hi", None, Some(DEAD_URL.into()), None).unwrap_err();
        assert!(err.contains("session not found"));
    }

    #[test]
    fn chat_turn_auto_picks_first_model_and_remembers_it() {
        let s = store();
        let server = serve_stub(Arc::new(|method, path| {
            if method == "GET" && path == "/api/tags" {
                (200, r#"{"models":[{"name":"stub-model","size":1234567}]}"#.into())
            } else if method == "POST" && path == "/api/chat" {
                (200, r#"{"message":{"content":"hi from stub"},"total_duration":1000000}"#.into())
            } else {
                (404, "{}".into())
            }
        }));
        let sess = create_session(&s, None).unwrap();
        let out = chat_turn(&s, &sess.id, "hi", Some(BackendKind::Ollama), Some(server), None).unwrap();
        assert_eq!(out.reply, "hi from stub");
        assert!(!out.offline);
        assert_eq!(out.model, "stub-model");
        assert!(!out.reply.contains("Deterministic fallback"));
        // auto-picked model persisted as the remembered choice
        let (kind, model) = ModelRegistry::remembered_choice(&s).unwrap();
        assert_eq!(kind, BackendKind::Ollama);
        assert_eq!(model, "stub-model");
        // user + assistant turns captured
        let stored = get_session(&s, &sess.id).unwrap().unwrap();
        assert_eq!(stored.messages.len(), 2);
        assert_eq!(stored.messages[0].role, "user");
        assert_eq!(stored.messages[1].role, "assistant");
    }

    #[test]
    fn explicit_model_overrides_and_persists_choice() {
        let s = store();
        let server = serve_stub(Arc::new(|_m, _p| (200, r#"{"message":{"content":"ok"}}"#.into())));
        let sess = create_session(&s, None).unwrap();
        let out = chat_turn(
            &s,
            &sess.id,
            "x",
            Some(BackendKind::Ollama),
            Some(server),
            Some("pinned-model".into()),
        )
        .unwrap();
        assert_eq!(out.model, "pinned-model");
        let (_, model) = ModelRegistry::remembered_choice(&s).unwrap();
        assert_eq!(model, "pinned-model");
    }

    #[test]
    fn extraction_pass_fires_on_schedule_and_approval_unlocks_citations() {
        let s = store();
        let sess = create_session(&s, None).unwrap();
        let turns = [
            "I prefer TypeScript for new services.",
            "Please remember: my favorite editor is Neovim.",
            "I always run cargo test before pushing.",
            "To deploy staging, run `bash scripts/deploy.sh staging`.",
            "I like dark mode and coffee.",
        ];
        for (i, t) in turns.iter().enumerate() {
            let out = chat_turn(&s, &sess.id, t, None, Some(DEAD_URL.into()), None).unwrap();
            if i == 4 {
                assert!(out.extraction_proposals > 0, "5th user turn fires the extraction pass");
            }
        }
        let cards = list_cards(&s).unwrap();
        let pending: Vec<_> = cards.iter().filter(|c| c.status == "pending").collect();
        assert!(pending.len() >= 2, "fixture turns should distill several proposals");
        assert_eq!(pending_count(&s).unwrap() as usize, pending.len());

        // approve every pending card (inline Y/n flow, headless path)
        for card in pending {
            let d = decide(&s, &card.id, true).unwrap();
            assert!(d.approved);
        }
        assert_eq!(pending_count(&s).unwrap(), 0);

        // a later turn about preferences must now retrieve the approved memory
        let out = chat_turn(&s, &sess.id, "what do I prefer?", None, Some(DEAD_URL.into()), None).unwrap();
        assert!(out.citations >= 1, "approved memories must be retrievable (got {})", out.citations);
        assert!(out.offline);
    }

    #[test]
    fn headless_chat_subcommand_shape() {
        // The `chat` subcommand creates a session when none is given. Exercise the
        // same create-then-chat sequence the CLI main runs.
        let s = store();
        let session = create_session(&s, None).unwrap();
        let out = chat_turn(&s, &session.id, "hi", None, Some(DEAD_URL.into()), None).unwrap();
        assert_eq!(out.session_id, session.id);
        let sessions = list_sessions(&s).unwrap();
        assert_eq!(sessions[0].title, "hi");
    }

    /// Live smoke against a real local Ollama — skipped by default. Run with:
    ///     cargo test -p deskagent-cli -- --ignored deskagent_chat_live
    /// or headless via:
    ///     DESKAGENT_DATA_DIR=$(mktemp -d) cargo run -p deskagent-cli -- chat "Hello, DeskAgent."
    #[test]
    #[ignore = "requires a running local Ollama"]
    fn deskagent_chat_live() {
        let s = store();
        let session = create_session(&s, None).unwrap();
        let out = chat_turn(&s, &session.id, "Hello, DeskAgent.", None, None, None).unwrap();
        println!("LIVE deskagent chat -> model={} offline={} reply={:?}", out.model, out.offline, out.reply);
        assert!(!out.offline, "a local Ollama was expected; run `ollama serve` first");
        assert!(!out.reply.is_empty());
        assert!(!out.reply.contains("Deterministic fallback"));
    }
}