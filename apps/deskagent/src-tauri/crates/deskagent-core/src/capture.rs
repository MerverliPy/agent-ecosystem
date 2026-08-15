//! Capture pipeline: every conversation turn is appended as a raw episode; every N
//! turns (default 5) an extraction pass distills facts/preferences into semantic and
//! procedural proposals (max 20 memories per pass, all routed through approval).

use crate::approvals::propose;
use crate::sessions::{append_message, create_session, get_session, Session};
use crate::store::{
    ApprovalStatus, MemoryEvent, MemoryKind, MemoryScope, MemorySource, MemoryStore, ScopeType,
    new_id,
};

pub const DEFAULT_TURNS_PER_PASS: usize = 5;
pub const MAX_MEMORIES_PER_PASS: usize = 20;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Keyword rules for deterministic extraction (no LLM needed at P0; the LLM-assisted
/// extraction path is a Phase 6+ refinement).
const PREFERENCE_MARKERS: &[&str] = &[
    "i prefer", "i like", "i love", "i want", "i need", "my favorite", "my favourite",
    "please remember", "important", "always", "never", "don't want", "do not want", "hate",
];
const PROCEDURAL_MARKERS: &[&str] = &[
    "when i", "to deploy", "to run", "the way to", "how to", "steps", "first ",
    "then ", "run `", "i use", "command is", "shortcut", "the workflow", "i do it",
];

/// Append a conversation turn: writes the message to the session AND stores a raw
/// episodic memory (source=conversation). Raw episodes mirror the user's own log, so
/// they are stored approved; *distilled* memories always go through approval cards.
pub fn capture_turn(
    store: &MemoryStore,
    session_id: &str,
    role: &str,
    content: &str,
    scope: MemoryScope,
) -> rusqlite::Result<Option<Session>> {
    let Some(_session) = get_session(store, session_id)? else {
        return Ok(None);
    };
    append_message(store, session_id, role, content, None)?;

    let episode = MemoryEvent {
        id: new_id("epi"),
        kind: MemoryKind::Episodic,
        content: format!("[{role}] {content}"),
        summary: None,
        source: MemorySource::Conversation,
        confidence: 0.9,
        created_at: now_iso(),
        updated_at: None,
        episode_id: None,
        scope,
        approval: ApprovalStatus::Approved,
        tags: None,
        embedding: None,
    };
    store.insert_memory(&episode)?;
    get_session(store, session_id)
}

/// Count turns since the last extraction pass (meta key per session).
pub fn turns_since_pass(store: &MemoryStore, session_id: &str) -> rusqlite::Result<i64> {
    let key = format!("extract_turns.{session_id}");
    Ok(store.meta(&key)?.and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// Run the extraction pass for a session. Distills semantic + procedural proposals
/// from user turns. Returns the number of proposals created (each is approval-pending).
pub fn run_extraction_pass(
    store: &MemoryStore,
    session_id: &str,
    turns_per_pass: usize,
    max_memories: usize,
) -> rusqlite::Result<usize> {
    let Some(session) = get_session(store, session_id)? else {
        return Ok(0);
    };
    let user_turns: Vec<&str> = session
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .collect();

    let mut created = 0usize;
    // each pass examines the most recent `turns_per_pass` user turns
    let start = user_turns.len().saturating_sub(turns_per_pass);
    for turn in &user_turns[start..] {
        if created >= max_memories {
            break;
        }
        let lower = turn.to_lowercase();

        if PREFERENCE_MARKERS.iter().any(|m| lower.contains(m)) {
            let fact = distill_preference(turn);
            let ev = MemoryEvent {
                id: new_id("sem"),
                kind: MemoryKind::Semantic,
                content: fact.clone(),
                summary: Some(fact),
                source: MemorySource::Extraction,
                confidence: 0.7,
                created_at: now_iso(),
                updated_at: None,
                episode_id: Some(session_id.to_string()),
                scope: session_scope(&session),
                approval: ApprovalStatus::Pending,
                tags: Some(vec!["extracted".into(), "preference".into()]),
                embedding: None,
            };
            propose(store, &ev, "Remember a preference".into())?;
            created += 1;
        }

        if created >= max_memories {
            break;
        }
        if PROCEDURAL_MARKERS.iter().any(|m| lower.contains(m)) {
            let proc = distill_procedure(turn);
            let ev = MemoryEvent {
                id: new_id("proc"),
                kind: MemoryKind::Procedural,
                content: proc.clone(),
                summary: Some(proc),
                source: MemorySource::Extraction,
                confidence: 0.65,
                created_at: now_iso(),
                updated_at: None,
                episode_id: Some(session_id.to_string()),
                scope: session_scope(&session),
                approval: ApprovalStatus::Pending,
                tags: Some(vec!["extracted".into(), "how-to".into()]),
                embedding: None,
            };
            propose(store, &ev, "Remember a workflow step".into())?;
            created += 1;
        }
    }

    let key = format!("extract_turns.{session_id}");
    let next = turns_since_pass(store, session_id)? + 1;
    store.set_meta(&key, &next.to_string())?;
    Ok(created)
}

fn session_scope(session: &Session) -> MemoryScope {
    match &session.project_id {
        Some(pid) => MemoryScope {
            scope_type: ScopeType::Project,
            project_id: Some(pid.clone()),
            project_path: None,
        },
        None => MemoryScope {
            scope_type: ScopeType::Companion,
            project_id: None,
            project_path: None,
        },
    }
}

fn distill_preference(turn: &str) -> String {
    let cleaned = turn
        .replace("please remember", "")
        .replace("important:", "")
        .trim()
        .to_string();
    // Keep it as a factual sentence about the user.
    let sentence = cleaned.trim_end_matches(['.', '!']).trim();
    let lower = sentence.to_lowercase();
    if lower.starts_with("i ") || lower.starts_with("my ") || lower.starts_with("we ") {
        sentence.to_string()
    } else {
        format!("User prefers: {sentence}")
    }
}

fn distill_procedure(turn: &str) -> String {
    let cleaned = turn.trim().trim_end_matches(['.', '!']).trim();
    format!("Workflow: {cleaned}")
}

/// Convenience: capture a full fixture conversation turn-by-turn with auto extraction.
pub fn capture_conversation(
    store: &MemoryStore,
    project_id: Option<String>,
    turns: &[(&str, &str)],
    turns_per_pass: usize,
) -> rusqlite::Result<(Session, usize)> {
    let session = create_session(store, project_id.clone())?;
    let scope = match &project_id {
        Some(pid) => MemoryScope {
            scope_type: ScopeType::Project,
            project_id: Some(pid.clone()),
            project_path: None,
        },
        None => MemoryScope {
            scope_type: ScopeType::Companion,
            project_id: None,
            project_path: None,
        },
    };
    let mut proposals = 0usize;
    let mut count = 0usize;
    for (role, content) in turns {
        capture_turn(store, &session.id, role, content, scope.clone())?;
        count += 1;
        if count % turns_per_pass == 0 {
            proposals += run_extraction_pass(store, &session.id, turns_per_pass, MAX_MEMORIES_PER_PASS)?;
        }
    }
    // reload so the returned session carries the captured messages
    let session = get_session(store, &session.id)?.expect("session exists");
    Ok((session, proposals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{MemoryStore, StoreConfig};

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    const FIXTURE: &[(&str, &str)] = &[
        ("user", "I prefer TypeScript for new services."),
        ("assistant", "Got it."),
        ("user", "To deploy staging, run `bash scripts/deploy.sh staging`."),
        ("assistant", "Noted."),
        ("user", "Also, my favorite editor is Neovim."),
        ("assistant", "Noted."),
    ];

    #[test]
    fn capture_stores_episodes_and_extracts_proposals() {
        let s = store();
        let (session, proposals) = capture_conversation(&s, Some("bench-site".into()), FIXTURE, 5).unwrap();
        assert_eq!(session.messages.len(), 6);
        assert_eq!(s.count_memories().unwrap(), 6 + proposals as i64);
        // episodes are approved; proposals are pending
        assert_eq!(s.list_approved().unwrap().len(), 6);
        assert_eq!(s.list_by_approval(ApprovalStatus::Pending).unwrap().len(), proposals);
    }

    #[test]
    fn extraction_distills_semantic_and_procedural() {
        let s = store();
        let (_session, proposals) = capture_conversation(&s, None, FIXTURE, 5).unwrap();
        assert_eq!(proposals, 3, "preference x2 + procedure x1");
        let pending = s.list_by_approval(ApprovalStatus::Pending).unwrap();
        assert!(pending.iter().any(|m| m.kind == MemoryKind::Semantic));
        assert!(pending.iter().any(|m| m.kind == MemoryKind::Procedural));
        assert!(pending.iter().any(|m| m.content.contains("TypeScript")));
        assert!(pending.iter().any(|m| m.content.contains("deploy")));
        // proposals carry the project scope from the session
        assert_eq!(pending[0].scope.scope_type, ScopeType::Companion);
    }

    #[test]
    fn max_memories_per_pass_is_honored() {
        let s = store();
        let turns: Vec<(&str, String)> = (0..10)
            .map(|i| ("user", format!("I prefer option number {i} for everything.")))
            .collect();
        let refs: Vec<(&str, &str)> = turns.iter().map(|(r, c)| (*r, c.as_str())).collect();
        let (_, proposals) = capture_conversation(&s, None, &refs, 2).unwrap();
        assert!(proposals <= MAX_MEMORIES_PER_PASS);
        // 10 turns / pass every 2 turns = 5 passes; each pass distills up to 20,
        // but the session has at most 10 preference turns → capped at 10.
        assert_eq!(proposals, 10);
    }

    #[test]
    fn capture_to_missing_session_returns_none() {
        let s = store();
        let scope = MemoryScope {
            scope_type: ScopeType::Companion,
            project_id: None,
            project_path: None,
        };
        assert!(capture_turn(&s, "missing", "user", "hi", scope).unwrap().is_none());
    }
}
