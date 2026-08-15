//! Conversation wiring: persona + scoped memories injected into chat context,
//! with "I remember…" citations carrying their sources (DEC-0009 retrieval).

use crate::consolidation::get_persona;
use crate::retrieval::{retrieve, DEFAULT_INJECTION_BUDGET_CHARS};
use crate::sessions::{append_message, context_usage, get_session};
use crate::store::{MemoryEvent, MemoryStore};

pub const CONTEXT_HISTORY_KEEP: usize = 40;
pub const CITATION_LABEL_OVERHEAD: usize = 60;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatContext {
    pub system: String,
    pub citations: Vec<MemoryEvent>,
    pub used_chars: usize,
    pub budget_chars: usize,
    pub truncated: bool,
}

/// Assemble the system prompt + citation memory set for a session's next turn:
/// persona preamble + retrieved memories (project-scoped when the session has a
/// project) + the trimmed conversation history, all within a strict char budget.
pub fn build_chat_context(
    store: &MemoryStore,
    session_id: &str,
    user_turn: &str,
    budget_chars: Option<usize>,
) -> rusqlite::Result<ChatContext> {
    let budget = budget_chars.unwrap_or(DEFAULT_INJECTION_BUDGET_CHARS);
    let Some(session) = get_session(store, session_id)? else {
        return Ok(ChatContext {
            system: String::new(),
            citations: vec![],
            used_chars: 0,
            budget_chars: budget,
            truncated: false,
        });
    };

    let mut system = String::new();
    if let Some(persona) = get_persona(store)? {
        system.push_str(&format!(
            "You are DeskAgent, a personal assistant. Persona (v{}): {}\nPreferences: {}\nFacts: {}\nSkills: {}\n",
            persona.version,
            persona.summary,
            persona.preferences.join(" | "),
            persona.facts.join(" | "),
            persona.skills.join(" | ")
        ));
    } else {
        system.push_str("You are DeskAgent, a local-first personal assistant. No persona generated yet.\n");
    }

    let retrieved = retrieve(
        store,
        &crate::retrieval::RetrievalQuery {
            text: user_turn.to_string(),
            project_id: session.project_id.clone(),
            kinds: None,
            limit: 8,
            budget_chars: budget,
        },
    )?;

    let mut citations: Vec<MemoryEvent> = Vec::new();
    if !retrieved.hits.is_empty() {
        system.push_str("\nMemories you may use (cite each with “I remember…”):\n");
        for (i, hit) in retrieved.hits.iter().enumerate() {
            let src = hit.memory.source.as_str();
            let kind = hit.memory.kind.as_str();
            let line = format!(
                "[{}] ({} · {} · conf {:.2}) {}\n",
                i + 1,
                kind,
                src,
                hit.memory.confidence,
                hit.memory.content
            );
            system.push_str(&line);
            citations.push(hit.memory.clone());
        }
        system.push_str("\nAlways reference the source id when you recall something.\n");
    }

    // history (already stored in the session; the caller trims for the request)
    let history_chars = context_usage(&session, CONTEXT_HISTORY_KEEP);
    let system_chars = system.chars().count();
    let used = system_chars + history_chars + CITATION_LABEL_OVERHEAD;

    Ok(ChatContext {
        system,
        citations,
        used_chars: used,
        budget_chars: budget,
        truncated: retrieved.truncated,
    })
}

/// Attach an assistant message with its memory citations (the "I remember…" payload).
pub fn attach_assistant_with_citations(
    store: &MemoryStore,
    session_id: &str,
    text: &str,
    citations: Vec<MemoryEvent>,
) -> rusqlite::Result<Option<crate::sessions::Session>> {
    append_message(store, session_id, "assistant", text, Some(citations))
}

/// Convenience for the chat loop: complete one assistant turn with memory context.
pub fn assistant_turn(
    store: &MemoryStore,
    session_id: &str,
    user_turn: &str,
    generate: impl FnOnce(&str, &str) -> Result<String, String>,
) -> Result<crate::sessions::Session, String> {
    let ctx = build_chat_context(store, session_id, user_turn, None).map_err(|e| e.to_string())?;
    let text = generate(&ctx.system, user_turn).map_err(|e| e.to_string())?;
    attach_assistant_with_citations(store, session_id, &text, ctx.citations)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::decide;
    use crate::capture::capture_conversation;
    use crate::store::{MemoryStore, StoreConfig};

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    const FIXTURE: &[(&str, &str)] = &[
        ("user", "I prefer TypeScript for new services."),
        ("assistant", "Got it."),
        ("user", "To deploy staging, run `bash scripts/deploy.sh staging`."),
        ("assistant", "Noted."),
        ("user", "My favorite editor is Neovim."),
        ("assistant", "Noted."),
    ];

    #[test]
    fn context_includes_persona_memories_and_history() {
        let s = store();
        let (session, proposals) = capture_conversation(&s, Some("bench-site".into()), FIXTURE, 5).unwrap();
        // approve the proposals so they become retrievable
        for p in s.list_by_approval(crate::store::ApprovalStatus::Pending).unwrap() {
            // find the card for this memory and approve
            let cards = crate::approvals::list_cards(&s).unwrap();
            if let Some(card) = cards.iter().find(|c| c.event.as_ref().map(|e| e.id == p.id).unwrap_or(false)) {
                decide(&s, &card.id, true).unwrap();
            }
        }
        let _ = proposals;
        let ctx = build_chat_context(&s, &session.id, "what do I prefer?", None).unwrap();
        assert!(ctx.system.contains("DeskAgent"));
        assert!(ctx.citations.len() >= 1, "expected memories in context");
        assert!(ctx.system.to_lowercase().contains("typescript"));
        assert!(ctx.used_chars > 0);
    }

    #[test]
    fn assistant_turn_attaches_citations() {
        let s = store();
        let (session, _) = capture_conversation(&s, None, FIXTURE, 5).unwrap();
        let updated = assistant_turn(&s, &session.id, "hi", |_sys, _u| Ok("hello".into())).unwrap();
        let last = updated.messages.last().unwrap();
        assert_eq!(last.role, "assistant");
        assert_eq!(last.content, "hello");
        // citations are attached even when empty (retrieval only sees approved memories)
        assert!(last.citations.is_some());
    }

    #[test]
    fn missing_session_returns_empty_context() {
        let s = store();
        let ctx = build_chat_context(&s, "nope", "hi", None).unwrap();
        assert_eq!(ctx.system, "");
        assert_eq!(ctx.citations.len(), 0);
    }
}
