//! Session persistence: conversations live in SQLite, shared between the chat UI
//! and the capture pipeline (raw episodes are derived from these messages).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::store::{MemoryEvent, MemoryStore, new_id};

pub const CONTEXT_BUDGET_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<MemoryEvent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ChatMessage>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn create_session(store: &MemoryStore, project_id: Option<String>) -> rusqlite::Result<Session> {
    let id = new_id("sess");
    let now = now_iso();
    store.connection().execute(
        "INSERT INTO sessions(id, title, project_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, "New conversation", project_id, now, now],
    )?;
    Ok(Session {
        id,
        title: "New conversation".into(),
        project_id,
        created_at: now.clone(),
        updated_at: now,
        messages: vec![],
    })
}

pub fn list_sessions(store: &MemoryStore) -> rusqlite::Result<Vec<Session>> {
    let mut stmt = store
        .connection()
        .prepare("SELECT id, title, project_id, created_at, updated_at FROM sessions ORDER BY updated_at DESC")?;
    let ids = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|(id, title, project_id, created_at, updated_at)| {
            let messages = load_messages(store, &id)?;
            Ok(Session {
                id,
                title,
                project_id,
                created_at,
                updated_at,
                messages,
            })
        })
        .collect()
}

pub fn get_session(store: &MemoryStore, id: &str) -> rusqlite::Result<Option<Session>> {
    let row = store.connection().query_row(
        "SELECT id, title, project_id, created_at, updated_at FROM sessions WHERE id = ?1",
        params![id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        },
    ).optional()?;
    match row {
        Some((id, title, project_id, created_at, updated_at)) => {
            let messages = load_messages(store, &id)?;
            Ok(Some(Session {
                id,
                title,
                project_id,
                created_at,
                updated_at,
                messages,
            }))
        }
        None => Ok(None),
    }
}

fn load_messages(store: &MemoryStore, session_id: &str) -> rusqlite::Result<Vec<ChatMessage>> {
    let mut stmt = store.connection().prepare(
        "SELECT id, session_id, role, content, created_at, citations FROM messages
         WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |r| {
        let citations: Option<String> = r.get(5)?;
        Ok(ChatMessage {
            id: r.get(0)?,
            session_id: r.get(1)?,
            role: r.get(2)?,
            content: r.get(3)?,
            created_at: r.get(4)?,
            citations: citations.and_then(|c| serde_json::from_str(&c).ok()),
        })
    })?;
    rows.collect()
}

/// Append a message; returns the updated session. The first user message becomes the title.
pub fn append_message(
    store: &MemoryStore,
    session_id: &str,
    role: &str,
    content: &str,
    citations: Option<Vec<MemoryEvent>>,
) -> rusqlite::Result<Option<Session>> {
    let Some(mut session) = get_session(store, session_id)? else {
        return Ok(None);
    };
    if session.title == "New conversation" && role == "user" {
        let one_line: String = content.trim().split_whitespace().collect::<Vec<_>>().join(" ");
        session.title = if one_line.chars().count() > 60 {
            let truncated: String = one_line.chars().take(59).collect();
            format!("{truncated}…")
        } else {
            one_line
        };
        store.connection().execute(
            "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![session.title, now_iso(), session_id],
        )?;
    }
    let msg = ChatMessage {
        id: new_id("msg"),
        session_id: session_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        created_at: now_iso(),
        citations,
    };
    let citations_json = msg
        .citations
        .as_ref()
        .map(|c| serde_json::to_string(c).expect("citations json"));
    store.connection().execute(
        "INSERT INTO messages(id, session_id, role, content, created_at, citations) VALUES (?1,?2,?3,?4,?5,?6)",
        params![msg.id, msg.session_id, msg.role, msg.content, msg.created_at, citations_json],
    )?;
    store.connection().execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![now_iso(), session_id],
    )?;
    session.messages.push(msg);
    Ok(Some(session))
}

/// Total characters of the last `keep` messages — used for the strict injection budget.
pub fn context_usage(session: &Session, keep: usize) -> usize {
    session
        .messages
        .iter()
        .rev()
        .take(keep)
        .map(|m| m.content.chars().count())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{MemoryStore, StoreConfig};

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    #[test]
    fn session_lifecycle() {
        let s = store();
        let sess = create_session(&s, Some("bench-site".into())).unwrap();
        assert_eq!(sess.title, "New conversation");
        assert_eq!(sess.messages.len(), 0);

        let updated = append_message(&s, &sess.id, "user", "How do I deploy staging?", None).unwrap().unwrap();
        assert!(updated.title.starts_with("How do I deploy"));
        assert_eq!(updated.messages.len(), 1);
        append_message(&s, &sess.id, "assistant", "Run the deploy script.", None).unwrap();

        let listed = list_sessions(&s).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].messages.len(), 2);
        assert!(context_usage(&listed[0], 40) > 0);
    }

    #[test]
    fn append_to_missing_session_returns_none() {
        let s = store();
        assert!(append_message(&s, "nope", "user", "hi", None).unwrap().is_none());
    }

    #[test]
    fn first_user_message_becomes_title_once() {
        let s = store();
        let sess = create_session(&s, None).unwrap();
        append_message(&s, &sess.id, "user", "First message here", None).unwrap();
        append_message(&s, &sess.id, "user", "Second message here", None).unwrap();
        let got = get_session(&s, &sess.id).unwrap().unwrap();
        assert_eq!(got.title, "First message here");
    }
}
