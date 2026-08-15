//! SQLite memory store: the four memory kinds, scopes, approvals, sessions, messages.
//! rusqlite with the `bundled` feature — zero system dependency, deterministic.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    Episodic,
    Semantic,
    Procedural,
    Working,
}

impl MemoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryKind::Episodic => "episodic",
            MemoryKind::Semantic => "semantic",
            MemoryKind::Procedural => "procedural",
            MemoryKind::Working => "working",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemorySource {
    Conversation,
    User,
    File,
    Api,
    Reflection,
    Extraction,
    Synthesis,
    Other,
}

impl MemorySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemorySource::Conversation => "conversation",
            MemorySource::User => "user",
            MemorySource::File => "file",
            MemorySource::Api => "api",
            MemorySource::Reflection => "reflection",
            MemorySource::Extraction => "extraction",
            MemorySource::Synthesis => "synthesis",
            MemorySource::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeType {
    Companion,
    Project,
}

impl ScopeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScopeType::Companion => "companion",
            ScopeType::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryScope {
    #[serde(rename = "type")]
    pub scope_type: ScopeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApprovalStatus::Pending => "pending",
            ApprovalStatus::Approved => "approved",
            ApprovalStatus::Rejected => "rejected",
        }
    }
}

/// A memory event — mirrors `shared/schemas/memory-event.schema.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEvent {
    pub id: String,
    pub kind: MemoryKind,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub source: MemorySource,
    pub confidence: f64,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_id: Option<String>,
    pub scope: MemoryScope,
    pub approval: ApprovalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Optional embedding vector (local, model name in the store's meta).
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

/// The encrypted-at-rest payload for a memory row.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedRow {
    nonce: String,
    cipher: String,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id(prefix: &str) -> String {
    let u = uuid::Uuid::new_v4();
    format!("{prefix}-{u}")
}

/// Storage configuration: where the DB lives and whether content columns are encrypted.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub path: String,
    pub encrypt: bool,
}

pub struct MemoryStore {
    conn: Connection,
    /// Encryption key (None when encryption is off). Content columns are stored
    /// base64(nonce) + base64(ciphertext) JSON when present.
    key: Option<[u8; 32]>,
}

impl MemoryStore {
    pub fn open(config: StoreConfig) -> rusqlite::Result<Self> {
        let conn = if config.path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(Path::new(&config.path))?
        };
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        let store = MemoryStore {
            conn,
            key: None,
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_encrypted(config: StoreConfig, key: [u8; 32]) -> rusqlite::Result<Self> {
        let mut store = Self::open(config)?;
        store.key = Some(key);
        Ok(store)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                content_enc TEXT NOT NULL,
                summary_enc TEXT,
                source TEXT NOT NULL,
                confidence REAL NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT,
                episode_id TEXT,
                scope_type TEXT NOT NULL,
                project_id TEXT,
                project_path TEXT,
                approval TEXT NOT NULL,
                tags TEXT,
                embedding BLOB
            );
            CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind);
            CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope_type, project_id);
            CREATE INDEX IF NOT EXISTS idx_memories_approval ON memories(approval);
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                project_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                citations TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                description TEXT NOT NULL,
                event_json TEXT,
                created_at TEXT NOT NULL,
                status TEXT NOT NULL,
                decided_at TEXT
            );
            CREATE TABLE IF NOT EXISTS persona (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                version INTEGER NOT NULL,
                generated_at TEXT NOT NULL,
                json TEXT NOT NULL
            );
            "#,
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES (?1, ?2)",
            params!["schema_version", SCHEMA_VERSION.to_string()],
        )?;
        Ok(())
    }

    pub fn meta(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| r.get(0))
            .optional()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- encryption helpers --------------------------------------------------

    fn encrypt_field(&self, plain: &str) -> String {
        match &self.key {
            None => plain.to_string(),
            Some(key) => {
                let payload = crate::encrypt::encrypt_string(key, plain);
                serde_json::to_string(&payload).expect("serialize encrypted payload")
            }
        }
    }

    fn decrypt_field(&self, stored: &str) -> String {
        match &self.key {
            None => stored.to_string(),
            Some(key) => {
                let payload: EncryptedRow = serde_json::from_str(stored).expect("stored payload");
                crate::encrypt::decrypt_string(key, &payload.nonce, &payload.cipher)
                    .expect("decrypt memory field")
            }
        }
    }

    // ---- memories -------------------------------------------------------------

    pub fn insert_memory(&self, event: &MemoryEvent) -> rusqlite::Result<()> {
        let tags = event
            .tags
            .as_ref()
            .map(|t| serde_json::to_string(t).expect("tags json"));
        let embedding = event
            .embedding
            .as_ref()
            .map(|v| serialize_vec_f32(v));
        self.conn.execute(
            "INSERT INTO memories (
                id, kind, content_enc, summary_enc, source, confidence, created_at, updated_at,
                episode_id, scope_type, project_id, project_path, approval, tags, embedding
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
             ON CONFLICT(id) DO UPDATE SET
                kind=excluded.kind, content_enc=excluded.content_enc, summary_enc=excluded.summary_enc,
                source=excluded.source, confidence=excluded.confidence, updated_at=excluded.updated_at,
                episode_id=excluded.episode_id, scope_type=excluded.scope_type, project_id=excluded.project_id,
                project_path=excluded.project_path, approval=excluded.approval, tags=excluded.tags,
                embedding=excluded.embedding",
            params![
                event.id,
                event.kind.as_str(),
                self.encrypt_field(&event.content),
                event.summary.as_deref().map(|s| self.encrypt_field(s)),
                event.source.as_str(),
                event.confidence,
                event.created_at,
                event.updated_at,
                event.episode_id,
                event.scope.scope_type.as_str(),
                event.scope.project_id,
                event.scope.project_path,
                event.approval.as_str(),
                tags,
                embedding,
            ],
        )?;
        Ok(())
    }

    pub fn get_memory(&self, id: &str) -> rusqlite::Result<Option<MemoryEvent>> {
        self.conn
            .query_row(
                "SELECT id, kind, content_enc, summary_enc, source, confidence, created_at, updated_at,
                        episode_id, scope_type, project_id, project_path, approval, tags, embedding
                 FROM memories WHERE id = ?1",
                params![id],
                |row| self.row_to_event(row),
            )
            .optional()
    }

    /// All memories, newest first.
    pub fn list_memories(&self) -> rusqlite::Result<Vec<MemoryEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, content_enc, summary_enc, source, confidence, created_at, updated_at,
                    episode_id, scope_type, project_id, project_path, approval, tags, embedding
             FROM memories ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| self.row_to_event(row))?;
        rows.collect()
    }

    /// Memories that have been approved (retrieval only ever sees these).
    pub fn list_approved(&self) -> rusqlite::Result<Vec<MemoryEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, content_enc, summary_enc, source, confidence, created_at, updated_at,
                    episode_id, scope_type, project_id, project_path, approval, tags, embedding
             FROM memories WHERE approval = 'approved' ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| self.row_to_event(row))?;
        rows.collect()
    }

    pub fn list_by_approval(&self, approval: ApprovalStatus) -> rusqlite::Result<Vec<MemoryEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, content_enc, summary_enc, source, confidence, created_at, updated_at,
                    episode_id, scope_type, project_id, project_path, approval, tags, embedding
             FROM memories WHERE approval = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![approval.as_str()], |row| self.row_to_event(row))?;
        rows.collect()
    }

    pub fn update_approval(&self, id: &str, status: ApprovalStatus) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE memories SET approval = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now_iso(), id],
        )
    }

    pub fn update_confidence(&self, id: &str, confidence: f64) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE memories SET confidence = ?1, updated_at = ?2 WHERE id = ?3",
            params![confidence, now_iso(), id],
        )
    }

    pub fn delete_memory(&self, id: &str) -> rusqlite::Result<usize> {
        self.conn.execute("DELETE FROM memories WHERE id = ?1", params![id])
    }

    /// DEC-0009 export: full JSON dump of every memory (decrypted).
    pub fn export_all(&self) -> rusqlite::Result<serde_json::Value> {
        let events = self.list_memories()?;
        Ok(serde_json::json!({
            "schema": "memory-event.schema.json",
            "exported_at": now_iso(),
            "count": events.len(),
            "memories": events,
        }))
    }

    /// DEC-0009 delete: wipe every memory and approval (the user owns the data).
    pub fn wipe_all(&self) -> rusqlite::Result<()> {
        self.conn
            .execute_batch("DELETE FROM memories; DELETE FROM approvals; DELETE FROM persona;")?;
        Ok(())
    }

    pub fn count_memories(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
    }

    pub fn count_approved(&self) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE approval = 'approved'",
            [],
            |r| r.get(0),
        )
    }

    // ---- row mapping ----------------------------------------------------------

    fn row_to_event(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEvent> {
        let id: String = row.get(0)?;
        let kind: String = row.get(1)?;
        let content_enc: String = row.get(2)?;
        let summary_enc: Option<String> = row.get(3)?;
        let source: String = row.get(4)?;
        let confidence: f64 = row.get(5)?;
        let created_at: String = row.get(6)?;
        let updated_at: Option<String> = row.get(7)?;
        let episode_id: Option<String> = row.get(8)?;
        let scope_type: String = row.get(9)?;
        let project_id: Option<String> = row.get(10)?;
        let project_path: Option<String> = row.get(11)?;
        let approval: String = row.get(12)?;
        let tags: Option<String> = row.get(13)?;
        let embedding: Option<Vec<u8>> = row.get(14)?;

        let kind = match kind.as_str() {
            "episodic" => MemoryKind::Episodic,
            "semantic" => MemoryKind::Semantic,
            "procedural" => MemoryKind::Procedural,
            "working" => MemoryKind::Working,
            other => return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown kind {other}").into(),
            )),
        };
        let source = match source.as_str() {
            "conversation" => MemorySource::Conversation,
            "user" => MemorySource::User,
            "file" => MemorySource::File,
            "api" => MemorySource::Api,
            "reflection" => MemorySource::Reflection,
            "extraction" => MemorySource::Extraction,
            "synthesis" => MemorySource::Synthesis,
            _ => MemorySource::Other,
        };
        let scope_type = if scope_type == "project" {
            ScopeType::Project
        } else {
            ScopeType::Companion
        };
        let approval = match approval.as_str() {
            "pending" => ApprovalStatus::Pending,
            "rejected" => ApprovalStatus::Rejected,
            _ => ApprovalStatus::Approved,
        };
        let embedding = embedding
            .map(|bytes| deserialize_vec_f32(&bytes))
            .filter(|v| !v.is_empty());

        Ok(MemoryEvent {
            id,
            kind,
            content: self.decrypt_field(&content_enc),
            summary: summary_enc.map(|s| self.decrypt_field(&s)),
            source,
            confidence,
            created_at,
            updated_at,
            episode_id,
            scope: MemoryScope {
                scope_type,
                project_id,
                project_path,
            },
            approval,
            tags: tags.and_then(|t| serde_json::from_str(&t).ok()),
            embedding,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn serialize_vec_f32(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn deserialize_vec_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> MemoryStore {
        MemoryStore::open(StoreConfig {
            path: ":memory:".into(),
            encrypt: false,
        })
        .unwrap()
    }

    fn sample(kind: MemoryKind) -> MemoryEvent {
        MemoryEvent {
            id: new_id("mem"),
            kind,
            content: "User prefers TypeScript for new services.".into(),
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
            approval: ApprovalStatus::Approved,
            tags: None,
            embedding: None,
        }
    }

    #[test]
    fn insert_get_list_roundtrip() {
        let store = test_store();
        let ev = sample(MemoryKind::Semantic);
        store.insert_memory(&ev).unwrap();
        let got = store.get_memory(&ev.id).unwrap().unwrap();
        assert_eq!(got.content, ev.content);
        assert_eq!(got.kind, MemoryKind::Semantic);
        assert_eq!(got.approval, ApprovalStatus::Approved);
        assert_eq!(store.list_memories().unwrap().len(), 1);
    }

    #[test]
    fn upsert_updates_in_place() {
        let store = test_store();
        let mut ev = sample(MemoryKind::Semantic);
        store.insert_memory(&ev).unwrap();
        ev.content = "User prefers Rust for CLIs.".into();
        ev.confidence = 0.95;
        store.insert_memory(&ev).unwrap();
        assert_eq!(store.count_memories().unwrap(), 1);
        assert_eq!(store.get_memory(&ev.id).unwrap().unwrap().confidence, 0.95);
    }

    #[test]
    fn approval_filtering_and_updates() {
        let store = test_store();
        let mut pending = sample(MemoryKind::Working);
        pending.approval = ApprovalStatus::Pending;
        store.insert_memory(&pending).unwrap();
        assert_eq!(store.list_approved().unwrap().len(), 0);
        assert_eq!(store.list_by_approval(ApprovalStatus::Pending).unwrap().len(), 1);
        store.update_approval(&pending.id, ApprovalStatus::Approved).unwrap();
        assert_eq!(store.list_approved().unwrap().len(), 1);
    }

    #[test]
    fn delete_and_wipe() {
        let store = test_store();
        let ev = sample(MemoryKind::Episodic);
        store.insert_memory(&ev).unwrap();
        assert_eq!(store.delete_memory(&ev.id).unwrap(), 1);
        assert!(store.get_memory(&ev.id).unwrap().is_none());
        store.insert_memory(&ev).unwrap();
        store.wipe_all().unwrap();
        assert_eq!(store.count_memories().unwrap(), 0);
    }

    #[test]
    fn export_all_is_valid_json_with_memories() {
        let store = test_store();
        store.insert_memory(&sample(MemoryKind::Semantic)).unwrap();
        let dump = store.export_all().unwrap();
        assert_eq!(dump["count"].as_i64().unwrap(), 1);
        assert_eq!(dump["memories"][0]["content"].as_str().unwrap(), "User prefers TypeScript for new services.");
    }

    #[test]
    fn encrypted_roundtrip_and_ciphertext_difference() {
        let key = [7u8; 32];
        let store = MemoryStore::open_encrypted(
            StoreConfig { path: ":memory:".into(), encrypt: true },
            key,
        )
        .unwrap();
        let ev = sample(MemoryKind::Semantic);
        store.insert_memory(&ev).unwrap();
        let got = store.get_memory(&ev.id).unwrap().unwrap();
        assert_eq!(got.content, ev.content);
        // raw bytes in the DB must not contain the plaintext
        let raw: String = store
            .connection()
            .query_row("SELECT content_enc FROM memories WHERE id = ?1", params![ev.id], |r| r.get(0))
            .unwrap();
        assert!(!raw.contains("TypeScript"), "plaintext leaked into the DB");
        assert!(raw.contains("nonce"), "expected an encrypted payload");
    }
}
