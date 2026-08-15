//! Propose-to-remember approval cards (DEC-0009): every distilled memory write routes
//! through an approval card; approvals and rejections are recorded as learning signal.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::store::{ApprovalStatus, MemoryEvent, MemoryStore, new_id};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalCard {
    pub id: String,
    pub kind: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<MemoryEvent>,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub card_id: String,
    pub approved: bool,
    pub decided_at: String,
    /// Confidence delta applied to the underlying memory (+0.1 approved / -0.1 rejected).
    pub confidence_delta: f64,
}

pub const APPROVED_DELTA: f64 = 0.1;
pub const REJECTED_DELTA: f64 = -0.1;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Create a pending approval card for a memory write. The memory is stored immediately
/// with approval=pending; retrieval only ever sees approved memories.
pub fn propose(store: &MemoryStore, event: &MemoryEvent, description: String) -> rusqlite::Result<ApprovalCard> {
    store.insert_memory(event)?;
    let card = ApprovalCard {
        id: new_id("appr"),
        kind: "memory_write".into(),
        description,
        event: Some(event.clone()),
        created_at: now_iso(),
        status: "pending".into(),
    };
    store.connection().execute(
        "INSERT INTO approvals(id, kind, description, event_json, created_at, status) VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            card.id,
            card.kind,
            card.description,
            serde_json::to_string(&card.event).expect("event json"),
            card.created_at,
            card.status
        ],
    )?;
    Ok(card)
}

/// Record a user decision: updates the memory approval + confidence and archives the card.
pub fn decide(store: &MemoryStore, card_id: &str, approved: bool) -> rusqlite::Result<ApprovalDecision> {
    let card = get_card(store, card_id)?.ok_or_else(|| {
        rusqlite::Error::QueryReturnedNoRows
    })?;
    let Some(event) = card.event.clone() else {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    };

    let status = if approved {
        ApprovalStatus::Approved
    } else {
        ApprovalStatus::Rejected
    };
    let delta = if approved { APPROVED_DELTA } else { REJECTED_DELTA };
    let next_confidence = (event.confidence + delta).clamp(0.0, 1.0);

    store.update_approval(&event.id, status)?;
    store.update_confidence(&event.id, next_confidence)?;
    store.connection().execute(
        "UPDATE approvals SET status = ?1, decided_at = ?2 WHERE id = ?3",
        params![status.as_str(), now_iso(), card_id],
    )?;
    // shared undo log: an approved memory write can be rolled back (DEC-0009)
    if approved {
        let _ = crate::sandbox::record_memory_undo(
            store,
            &event.id,
            &format!("un-remember: {}", event.content.chars().take(80).collect::<String>()),
        );
    }

    Ok(ApprovalDecision {
        card_id: card_id.to_string(),
        approved,
        decided_at: now_iso(),
        confidence_delta: delta,
    })
}

pub fn get_card(store: &MemoryStore, id: &str) -> rusqlite::Result<Option<ApprovalCard>> {
    store
        .connection()
        .query_row(
            "SELECT id, kind, description, event_json, created_at, status FROM approvals WHERE id = ?1",
            params![id],
            |r| {
                Ok(ApprovalCard {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    description: r.get(2)?,
                    event: r.get::<_, Option<String>>(3)?.and_then(|j| serde_json::from_str(&j).ok()),
                    created_at: r.get(4)?,
                    status: r.get(5)?,
                })
            },
        )
        .optional()
}

pub fn list_cards(store: &MemoryStore) -> rusqlite::Result<Vec<ApprovalCard>> {
    let mut stmt = store.connection().prepare(
        "SELECT id, kind, description, event_json, created_at, status FROM approvals ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ApprovalCard {
            id: r.get(0)?,
            kind: r.get(1)?,
            description: r.get(2)?,
            event: r.get::<_, Option<String>>(3)?.and_then(|j| serde_json::from_str(&j).ok()),
            created_at: r.get(4)?,
            status: r.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn pending_count(store: &MemoryStore) -> rusqlite::Result<i64> {
    store.connection().query_row(
        "SELECT COUNT(*) FROM approvals WHERE status = 'pending'",
        [],
        |r| r.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{
        MemoryEvent, MemoryKind, MemoryScope, MemorySource, MemoryStore, ScopeType, StoreConfig,
    };

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    fn event() -> MemoryEvent {
        MemoryEvent {
            id: new_id("mem"),
            kind: MemoryKind::Semantic,
            content: "User prefers Rust for CLIs.".into(),
            summary: None,
            source: MemorySource::Extraction,
            confidence: 0.8,
            created_at: now_iso(),
            updated_at: None,
            episode_id: None,
            scope: MemoryScope {
                scope_type: ScopeType::Companion,
                project_id: None,
                project_path: None,
            },
            approval: ApprovalStatus::Pending,
            tags: None,
            embedding: None,
        }
    }

    #[test]
    fn propose_creates_pending_card_and_pending_memory() {
        let s = store();
        let card = propose(&s, &event(), "Remember this".into()).unwrap();
        assert_eq!(card.status, "pending");
        assert_eq!(pending_count(&s).unwrap(), 1);
        assert_eq!(s.list_by_approval(ApprovalStatus::Pending).unwrap().len(), 1);
        assert_eq!(s.list_approved().unwrap().len(), 0);
    }

    #[test]
    fn approve_applies_learning_signal() {
        let s = store();
        let ev = event();
        let card = propose(&s, &ev, "Remember this".into()).unwrap();
        let decision = decide(&s, &card.id, true).unwrap();
        assert!(decision.approved);
        assert_eq!(decision.confidence_delta, APPROVED_DELTA);
        let stored = s.get_memory(&ev.id).unwrap().unwrap();
        assert_eq!(stored.approval, ApprovalStatus::Approved);
        assert!((stored.confidence - 0.9).abs() < 1e-9);
        assert_eq!(pending_count(&s).unwrap(), 0);
    }

    #[test]
    fn reject_downgrades_confidence() {
        let s = store();
        let ev = event();
        let card = propose(&s, &ev, "Remember this".into()).unwrap();
        decide(&s, &card.id, false).unwrap();
        let stored = s.get_memory(&ev.id).unwrap().unwrap();
        assert_eq!(stored.approval, ApprovalStatus::Rejected);
        assert!((stored.confidence - 0.7).abs() < 1e-9);
    }

    #[test]
    fn decide_on_unknown_card_errors() {
        let s = store();
        assert!(decide(&s, "missing", true).is_err());
    }

    #[test]
    fn list_cards_returns_history() {
        let s = store();
        let card = propose(&s, &event(), "desc".into()).unwrap();
        decide(&s, &card.id, true).unwrap();
        let cards = list_cards(&s).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].status, "approved");
    }
}
