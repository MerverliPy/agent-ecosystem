//! Action sandbox: risky tool calls (shell, file writes, network) render as approval
//! cards and are blocked until click-to-approve. A single undo log is shared with
//! memory-write approvals so every accepted mutation can be rolled back (DEC-0009).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::store::MemoryStore;

pub const RISKY_KINDS: &[&str] = &["shell", "file_write", "network"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionProposal {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub risk: String,
    pub created_at: String,
    pub status: String,
    pub undo_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEntry {
    pub id: String,
    pub scope: String, // "action" | "memory"
    pub target_id: String,
    pub description: String,
    pub created_at: String,
    pub status: String, // "open" | "reverted"
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Propose a risky action. It stays `pending` (not executed) until approved.
pub fn propose_action(
    store: &MemoryStore,
    kind: &str,
    description: &str,
    risk: &str,
    undo_description: &str,
) -> rusqlite::Result<ActionProposal> {
    if !RISKY_KINDS.contains(&kind) {
        return Err(rusqlite::Error::InvalidParameterName(format!("unknown risky kind {kind}")));
    }
    let proposal = ActionProposal {
        id: crate::store::new_id("act"),
        kind: kind.to_string(),
        description: description.to_string(),
        risk: risk.to_string(),
        created_at: now_iso(),
        status: "pending".into(),
        undo_description: undo_description.to_string(),
    };
    store.connection().execute(
        "INSERT INTO actions(id, kind, description, risk, created_at, status, undo_description)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            proposal.id,
            proposal.kind,
            proposal.description,
            proposal.risk,
            proposal.created_at,
            proposal.status,
            proposal.undo_description
        ],
    )?;
    Ok(proposal)
}

/// Decide an action: approving records an undo entry (the reversal itself is the
/// app layer's job — this is the log + gate). Rejecting just closes it.
pub fn decide_action(store: &MemoryStore, id: &str, approved: bool) -> rusqlite::Result<Option<UndoEntry>> {
    let Some(p) = get_action(store, id)? else {
        return Ok(None);
    };
    let status = if approved { "approved" } else { "rejected" };
    store.connection().execute(
        "UPDATE actions SET status = ?1, decided_at = ?2 WHERE id = ?3",
        params![status, now_iso(), id],
    )?;
    if approved {
        let undo = UndoEntry {
            id: crate::store::new_id("undo"),
            scope: "action".into(),
            target_id: p.id,
            description: p.undo_description,
            created_at: now_iso(),
            status: "open".into(),
        };
        store.connection().execute(
            "INSERT INTO undo_log(id, scope, target_id, description, created_at, status) VALUES (?1,?2,?3,?4,?5,?6)",
            params![undo.id, undo.scope, undo.target_id, undo.description, undo.created_at, undo.status],
        )?;
        Ok(Some(undo))
    } else {
        Ok(None)
    }
}

pub fn get_action(store: &MemoryStore, id: &str) -> rusqlite::Result<Option<ActionProposal>> {
    store
        .connection()
        .query_row(
            "SELECT id, kind, description, risk, created_at, status, undo_description FROM actions WHERE id = ?1",
            params![id],
            |r| {
                Ok(ActionProposal {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    description: r.get(2)?,
                    risk: r.get(3)?,
                    created_at: r.get(4)?,
                    status: r.get(5)?,
                    undo_description: r.get(6)?,
                })
            },
        )
        .optional()
}

pub fn list_actions(store: &MemoryStore) -> rusqlite::Result<Vec<ActionProposal>> {
    let mut stmt = store.connection().prepare(
        "SELECT id, kind, description, risk, created_at, status, undo_description FROM actions ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ActionProposal {
            id: r.get(0)?,
            kind: r.get(1)?,
            description: r.get(2)?,
            risk: r.get(3)?,
            created_at: r.get(4)?,
            status: r.get(5)?,
            undo_description: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// Record an undo entry for an approved memory write (shared undo log).
pub fn record_memory_undo(store: &MemoryStore, memory_id: &str, description: &str) -> rusqlite::Result<UndoEntry> {
    let undo = UndoEntry {
        id: crate::store::new_id("undo"),
        scope: "memory".into(),
        target_id: memory_id.to_string(),
        description: description.to_string(),
        created_at: now_iso(),
        status: "open".into(),
    };
    store.connection().execute(
        "INSERT INTO undo_log(id, scope, target_id, description, created_at, status) VALUES (?1,?2,?3,?4,?5,?6)",
        params![undo.id, undo.scope, undo.target_id, undo.description, undo.created_at, undo.status],
    )?;
    Ok(undo)
}

pub fn list_undo(store: &MemoryStore) -> rusqlite::Result<Vec<UndoEntry>> {
    let mut stmt = store.connection().prepare(
        "SELECT id, scope, target_id, description, created_at, status FROM undo_log ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(UndoEntry {
            id: r.get(0)?,
            scope: r.get(1)?,
            target_id: r.get(2)?,
            description: r.get(3)?,
            created_at: r.get(4)?,
            status: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// Mark an undo entry as reverted (the caller performs the actual reversal).
pub fn revert_undo(store: &MemoryStore, id: &str) -> rusqlite::Result<Option<UndoEntry>> {
    let n = store.connection().execute(
        "UPDATE undo_log SET status = 'reverted' WHERE id = ?1 AND status = 'open'",
        params![id],
    )?;
    if n == 0 {
        return Ok(None);
    }
    store
        .connection()
        .query_row(
            "SELECT id, scope, target_id, description, created_at, status FROM undo_log WHERE id = ?1",
            params![id],
            |r| {
                Ok(UndoEntry {
                    id: r.get(0)?,
                    scope: r.get(1)?,
                    target_id: r.get(2)?,
                    description: r.get(3)?,
                    created_at: r.get(4)?,
                    status: r.get(5)?,
                })
            },
        )
        .optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{MemoryStore, StoreConfig};

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    #[test]
    fn risky_action_lifecycle_with_undo() {
        let s = store();
        let p = propose_action(&s, "shell", "run: rm -rf /tmp/x", "high", "restore /tmp/x from backup").unwrap();
        assert_eq!(p.status, "pending");
        assert_eq!(p.risk, "high");
        let undo = decide_action(&s, &p.id, true).unwrap().unwrap();
        assert_eq!(undo.scope, "action");
        assert_eq!(undo.status, "open");
        assert_eq!(get_action(&s, &p.id).unwrap().unwrap().status, "approved");
        assert_eq!(list_undo(&s).unwrap().len(), 1);

        // revert
        let reverted = revert_undo(&s, &undo.id).unwrap().unwrap();
        assert_eq!(reverted.status, "reverted");
        assert!(revert_undo(&s, &undo.id).unwrap().is_none());
    }

    #[test]
    fn rejecting_closes_without_undo() {
        let s = store();
        let p = propose_action(&s, "network", "POST https://example.com/data", "medium", "no-op").unwrap();
        assert!(decide_action(&s, &p.id, false).unwrap().is_none());
        assert_eq!(list_undo(&s).unwrap().len(), 0);
        assert_eq!(get_action(&s, &p.id).unwrap().unwrap().status, "rejected");
    }

    #[test]
    fn unknown_risky_kind_rejected() {
        let s = store();
        assert!(propose_action(&s, "telepathy", "mind read", "high", "none").is_err());
    }

    #[test]
    fn memory_undo_shared_log() {
        let s = store();
        let undo = record_memory_undo(&s, "mem-1", "un-remember extracted fact mem-1").unwrap();
        assert_eq!(undo.scope, "memory");
        assert_eq!(undo.target_id, "mem-1");
        assert_eq!(list_undo(&s).unwrap().len(), 1);
    }
}
