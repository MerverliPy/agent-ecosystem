//! Hybrid retrieval: keyword (token overlap) + embedding (cosine) with Reciprocal Rank
//! Fusion, a strict injection budget, and companion/project scoping (DEC-0009).

use serde::{Deserialize, Serialize};

use crate::embed::{Embedder, HashEmbedder};
use crate::store::{MemoryEvent, MemoryStore, ScopeType};

pub const DEFAULT_INJECTION_BUDGET_CHARS: usize = 4000;
pub const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalQuery {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<String>>,
    pub limit: usize,
    /// Strict injection budget in characters; retrieval never exceeds it.
    pub budget_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub hits: Vec<RetrievedMemory>,
    pub total_chars: usize,
    pub budget_chars: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedMemory {
    pub memory: MemoryEvent,
    pub score: f64,
    pub via: Vec<String>,
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() > 1)
        .collect()
}

fn keyword_score(query: &[String], content: &str) -> f64 {
    let content_tokens = tokens(content);
    if content_tokens.is_empty() || query.is_empty() {
        return 0.0;
    }
    let mut score = 0.0;
    for q in query {
        let count = content_tokens.iter().filter(|t| *t == q).count() as f64;
        score += count;
    }
    score / content_tokens.len() as f64
}

fn in_scope(memory: &MemoryEvent, query: &RetrievalQuery) -> bool {
    match memory.scope.scope_type {
        // companion memories apply everywhere; project memories only to their project
        ScopeType::Companion => true,
        ScopeType::Project => {
            query.project_id.is_some() && memory.scope.project_id.as_deref() == query.project_id.as_deref()
        }
    }
}

fn kind_ok(memory: &MemoryEvent, query: &RetrievalQuery) -> bool {
    match &query.kinds {
        None => true,
        Some(kinds) => kinds.iter().any(|k| k == memory.kind.as_str()),
    }
}

/// Hybrid retrieval over approved memories only, with strict injection budget.
pub fn retrieve(store: &MemoryStore, query: &RetrievalQuery) -> rusqlite::Result<RetrievalResult> {
    let mut embedder = HashEmbedder::default();
    let q_tokens = tokens(&query.text);
    let q_vec = if q_tokens.is_empty() {
        Vec::new()
    } else {
        embedder.embed(&query.text)
    };

    let candidates: Vec<MemoryEvent> = store
        .list_approved()?
        .into_iter()
        .filter(|m| in_scope(m, query))
        .filter(|m| kind_ok(m, query))
        .collect();

    // keyword ranking
    let mut kw_ranked: Vec<(&MemoryEvent, f64)> = candidates
        .iter()
        .map(|m| (m, keyword_score(&q_tokens, &m.content)))
        .filter(|(_, s)| *s > 0.0)
        .collect();
    kw_ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    // embedding ranking
    let mut emb_ranked: Vec<(&MemoryEvent, f64)> = candidates
        .iter()
        .map(|m| {
            let vec = m.embedding.clone().unwrap_or_else(|| embedder.embed(&m.content));
            let sim = if q_vec.is_empty() { 0.0 } else { crate::embed::cosine(&q_vec, &vec) as f64 };
            (m, sim)
        })
        .filter(|(_, s)| *s > 0.0)
        .collect();
    emb_ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    // RRF fusion
    let mut fused: Vec<(String, f64, Vec<String>)> = Vec::new();
    let rank_of = |list: &[(&MemoryEvent, f64)], id: &str| -> Option<usize> {
        list.iter().position(|(m, _)| m.id == id)
    };
    for m in &candidates {
        let mut score = 0.0;
        let mut via = Vec::new();
        if let Some(r) = rank_of(&kw_ranked, &m.id) {
            score += 1.0 / (RRF_K + r as f64);
            via.push("keyword".into());
        }
        if let Some(r) = rank_of(&emb_ranked, &m.id) {
            score += 1.0 / (RRF_K + r as f64);
            via.push("embedding".into());
        }
        if score > 0.0 {
            fused.push((m.id.clone(), score, via));
        }
    }
    fused.sort_by(|a, b| b.1.total_cmp(&a.1));
    fused.truncate(query.limit.max(1));

    // strict injection budget: fill until the char budget is exhausted
    let mut hits = Vec::new();
    let mut total_chars = 0usize;
    let mut truncated = false;
    for (id, score, via) in fused {
        if let Some(memory) = store.get_memory(&id)? {
            let chars = memory.content.chars().count() + 40; // +label overhead
            if total_chars + chars > query.budget_chars && !hits.is_empty() {
                truncated = true;
                break;
            }
            total_chars += chars;
            hits.push(RetrievedMemory { memory, score, via });
        }
    }

    Ok(RetrievalResult {
        hits,
        total_chars,
        budget_chars: query.budget_chars,
        truncated,
    })
}

/// Convenience: default budget query.
pub fn retrieve_text(store: &MemoryStore, text: &str, project_id: Option<String>, limit: usize) -> rusqlite::Result<RetrievalResult> {
    retrieve(
        store,
        &RetrievalQuery {
            text: text.to_string(),
            project_id,
            kinds: None,
            limit,
            budget_chars: DEFAULT_INJECTION_BUDGET_CHARS,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{ApprovalStatus, MemoryKind, MemoryScope, MemorySource, MemoryStore, StoreConfig};

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    fn event(id: &str, content: &str, scope: MemoryScope, approval: ApprovalStatus) -> MemoryEvent {
        MemoryEvent {
            id: id.into(),
            kind: MemoryKind::Semantic,
            content: content.into(),
            summary: None,
            source: MemorySource::Extraction,
            confidence: 0.8,
            created_at: "2026-08-15T00:00:00Z".into(),
            updated_at: None,
            episode_id: None,
            scope,
            approval,
            tags: None,
            embedding: None,
        }
    }

    fn companion(project: Option<&str>) -> MemoryScope {
        MemoryScope {
            scope_type: if project.is_some() { ScopeType::Project } else { ScopeType::Companion },
            project_id: project.map(String::from),
            project_path: None,
        }
    }

    #[test]
    fn retrieval_returns_scoped_hits_ranked() {
        let s = store();
        s.insert_memory(&event("a", "User prefers TypeScript for new services.", companion(None), ApprovalStatus::Approved)).unwrap();
        s.insert_memory(&event("b", "To deploy staging run the deploy script then check health.", companion(Some("bench-site")), ApprovalStatus::Approved)).unwrap();
        s.insert_memory(&event("c", "User hates JavaScript for new services.", companion(None), ApprovalStatus::Approved)).unwrap();

        let res = retrieve_text(&s, "deploy staging script", Some("bench-site".into()), 10).unwrap();
        assert!(!res.hits.is_empty());
        assert_eq!(res.hits[0].memory.id, "b", "project-scoped memory should lead for the project query");
        assert!(res.hits.iter().all(|h| h.via.contains(&"keyword".to_string()) || h.via.contains(&"embedding".to_string())));

        // project memory must NOT leak to a companion-scoped query
        let res2 = retrieve_text(&s, "deploy staging script", None, 10).unwrap();
        assert!(res2.hits.iter().all(|h| h.memory.id != "b"));
    }

    #[test]
    fn rejected_and_pending_memories_are_never_retrieved() {
        let s = store();
        s.insert_memory(&event("p", "pending preference for X", companion(None), ApprovalStatus::Pending)).unwrap();
        s.insert_memory(&event("r", "rejected preference for X", companion(None), ApprovalStatus::Rejected)).unwrap();
        let res = retrieve_text(&s, "preference for X", None, 10).unwrap();
        assert_eq!(res.hits.len(), 0);
    }

    #[test]
    fn injection_budget_is_strict() {
        let s = store();
        for i in 0..20 {
            s.insert_memory(&event(
                &format!("m{i}"),
                &format!("User fact number {i} about the deployment workflow details"),
                companion(None),
                ApprovalStatus::Approved,
            ))
            .unwrap();
        }
        let res = retrieve(
            &s,
            &RetrievalQuery {
                text: "deployment workflow fact".into(),
                project_id: None,
                kinds: None,
                limit: 20,
                budget_chars: 500,
            },
        )
        .unwrap();
        assert!(res.total_chars <= 500, "budget exceeded: {}", res.total_chars);
        assert!(res.hits.len() < 20);
    }

    #[test]
    fn kind_filter_works() {
        let s = store();
        let mut proc = event("p1", "To deploy run the script.", companion(None), ApprovalStatus::Approved);
        proc.kind = MemoryKind::Procedural;
        s.insert_memory(&proc).unwrap();
        s.insert_memory(&event("s1", "User prefers TypeScript.", companion(None), ApprovalStatus::Approved)).unwrap();
        let res = retrieve(
            &s,
            &RetrievalQuery {
                text: "script".into(),
                project_id: None,
                kinds: Some(vec!["procedural".into()]),
                limit: 5,
                budget_chars: 1000,
            },
        )
        .unwrap();
        assert!(res.hits.iter().all(|h| h.memory.kind == MemoryKind::Procedural));
    }

    #[test]
    fn empty_query_returns_empty() {
        let s = store();
        s.insert_memory(&event("a", "some content", companion(None), ApprovalStatus::Approved)).unwrap();
        let res = retrieve_text(&s, "", None, 5).unwrap();
        assert_eq!(res.hits.len(), 0);
    }
}
