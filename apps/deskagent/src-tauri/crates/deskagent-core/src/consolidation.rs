//! Consolidation & persona: regenerate the persona every N new memories (default 50),
//! dedupe near-identical memories, detect conflicts, and apply time decay.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::embed::{Embedder, HashEmbedder, cosine};
use crate::store::{MemoryEvent, MemoryKind, MemorySource, MemoryStore};

pub const PERSONA_REGEN_INTERVAL: i64 = 50;
pub const CONFLICT_THRESHOLD: f64 = 0.35;
pub const DECAY_DROP: f64 = 0.1;
pub const DUPLICATE_COSINE: f32 = 0.98;
pub const DUPLICATE_TOKEN_DIFF: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub version: i64,
    pub generated_at: String,
    pub summary: String,
    pub facts: Vec<String>,
    pub preferences: Vec<String>,
    pub skills: Vec<String>,
    pub memories_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub deduped: usize,
    pub conflicts: usize,
    pub decayed: usize,
    pub persona_regenerated: bool,
    pub persona: Option<Persona>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn signature_tokens(content: &str) -> std::collections::BTreeSet<String> {
    content
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

/// True when one token set is the other plus at most `DUPLICATE_TOKEN_DIFF` extra tokens.
fn token_subset(a: &std::collections::BTreeSet<String>, b: &std::collections::BTreeSet<String>) -> bool {
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if large.len() - small.len() > DUPLICATE_TOKEN_DIFF {
        return false;
    }
    small.is_subset(large)
}

/// Dedupe near-identical approved semantic memories (exact signature or cosine > 0.95).
/// The merged memory keeps the newer/higher-confidence variant, tagged as synthesis.
pub fn dedupe(store: &MemoryStore) -> rusqlite::Result<usize> {
    let approved = store.list_approved()?;
    let sem: Vec<MemoryEvent> = approved
        .into_iter()
        .filter(|m| m.kind == MemoryKind::Semantic)
        .collect();
    let mut embedder = HashEmbedder::default();

    let mut kept: Vec<&MemoryEvent> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();

    for m in &sem {
        let sig: std::collections::BTreeSet<String> = signature_tokens(&m.content);
        let mut is_dup = false;
        for k in &kept {
            let k_sig = signature_tokens(&k.content);
            if sig == k_sig {
                is_dup = true;
            } else if token_subset(&sig, &k_sig) {
                // one is the other plus a couple of tokens (e.g. "…(edited)")
                is_dup = true;
            } else if sig.len() >= 8 && k_sig.len() >= 8 {
                // cosine only for longer texts — short texts are unreliable in low dims
                let a = embedder.embed(&m.content);
                let b = embedder.embed(&k.content);
                if cosine(&a, &b) > DUPLICATE_COSINE {
                    is_dup = true;
                }
            }
            if is_dup {
                break;
            }
        }
        if is_dup {
            dropped.push(m.id.clone());
        } else {
            kept.push(m);
        }
    }

    let mut merged_sources: Vec<String> = Vec::new();
    for id in &dropped {
        if let Some(ev) = store.get_memory(id)? {
            merged_sources.extend(ev.episode_id.clone());
            store.delete_memory(id)?;
        }
    }
    // Tag the surviving memory as synthesized if we merged anything into it.
    if !dropped.is_empty() {
        if let Some(survivor) = kept.first() {
            let mut updated = (*survivor).clone();
            updated.source = MemorySource::Synthesis;
            updated.episode_id = Some(merged_sources.join(","));
            store.insert_memory(&updated)?;
        }
    }
    Ok(dropped.len())
}

/// Detect opposite-polarity semantic memories on the same topic (like/dislike, prefer/avoid).
pub fn detect_conflicts(store: &MemoryStore) -> rusqlite::Result<Vec<(MemoryEvent, MemoryEvent)>> {
    let sem: Vec<MemoryEvent> = store
        .list_approved()?
        .into_iter()
        .filter(|m| m.kind == MemoryKind::Semantic)
        .collect();
    let mut embedder = HashEmbedder::default();
    let mut conflicts = Vec::new();

    for (i, a) in sem.iter().enumerate() {
        for b in sem.iter().skip(i + 1) {
            let sim = cosine(&embedder.embed(&a.content), &embedder.embed(&b.content));
            if sim as f64 > CONFLICT_THRESHOLD && polarity(a) != polarity(b) && polarity(a) != Polarity::Neutral && polarity(b) != Polarity::Neutral {
                conflicts.push((a.clone(), b.clone()));
            }
        }
    }
    Ok(conflicts)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Polarity {
    Positive,
    Negative,
    Neutral,
}

fn polarity(m: &MemoryEvent) -> Polarity {
    let l = m.content.to_lowercase();
    let positive = ["prefer", "like", "love", "favorite", "favourite", "want", "always", "good", "works"];
    let negative = ["hate", "dislike", "never", "bad", "broken", "don't want", "do not want", "avoid", "fails"];
    let pos = positive.iter().filter(|p| l.contains(**p)).count();
    let neg = negative.iter().filter(|p| l.contains(**p)).count();
    if pos > neg {
        Polarity::Positive
    } else if neg > pos {
        Polarity::Negative
    } else {
        Polarity::Neutral
    }
}

/// Apply time decay: memories with a decay half-life lose confidence over elapsed time;
/// confidence dropping below the drop threshold is deleted (user-owned cleanup).
pub fn apply_decay(store: &MemoryStore, now: &str, half_life_days: f64, drop_below: f64) -> rusqlite::Result<usize> {
    let now_ts = chrono::DateTime::parse_from_rfc3339(now)
        .map(|d| d.timestamp())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp());
    let half_life_secs = half_life_days * 86_400.0;
    let mut decayed = 0usize;

    for m in store.list_approved()? {
        let created = chrono::DateTime::parse_from_rfc3339(&m.created_at)
            .map(|d| d.timestamp())
            .unwrap_or(now_ts);
        let age = (now_ts - created).max(0) as f64;
        let factor = 0.5_f64.powf(age / half_life_secs);
        let next = m.confidence * factor;
        if next < drop_below {
            store.delete_memory(&m.id)?;
        } else if (next - m.confidence).abs() > 1e-9 {
            store.update_confidence(&m.id, next)?;
            decayed += 1;
        }
    }
    Ok(decayed)
}

/// Regenerate the persona from approved semantic + procedural memories.
pub fn regenerate_persona(store: &MemoryStore) -> rusqlite::Result<Persona> {
    let approved = store.list_approved()?;
    let mut preferences: Vec<String> = approved
        .iter()
        .filter(|m| m.kind == MemoryKind::Semantic)
        .filter(|m| {
            let l = m.content.to_lowercase();
            ["prefer", "like", "favorite", "favourite", "want"].iter().any(|p| l.contains(p))
        })
        .map(|m| m.content.clone())
        .collect();
    let mut facts: Vec<String> = approved
        .iter()
        .filter(|m| m.kind == MemoryKind::Semantic)
        .filter(|m| {
            let l = m.content.to_lowercase();
            !["prefer", "like", "favorite", "favourite"].iter().any(|p| l.contains(p))
        })
        .map(|m| m.content.clone())
        .collect();
    let skills: Vec<String> = approved
        .iter()
        .filter(|m| m.kind == MemoryKind::Procedural)
        .map(|m| m.content.clone())
        .collect();

    // cap the persona surface (keep it small enough to inject into context)
    preferences.truncate(12);
    facts.truncate(12);
    let skills = skills.into_iter().take(8).collect::<Vec<_>>();

    let version = store.meta("persona_version")?.and_then(|v| v.parse().ok()).unwrap_or(0) + 1;
    let generated_at = now_iso();
    let memories_count = approved.len() as i64;
    let summary = build_summary(&preferences, &facts, &skills);
    let persona = Persona {
        version,
        generated_at: generated_at.clone(),
        summary,
        facts,
        preferences,
        skills,
        memories_count,
    };

    store.set_meta("persona_version", &version.to_string())?;
    store.connection().execute(
        "INSERT INTO persona(id, version, generated_at, json) VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET version = excluded.version, generated_at = excluded.generated_at, json = excluded.json",
        rusqlite::params![version, generated_at, serde_json::to_string(&persona).expect("persona json")],
    )?;
    Ok(persona)
}

pub fn get_persona(store: &MemoryStore) -> rusqlite::Result<Option<Persona>> {
    store
        .connection()
        .query_row(
            "SELECT json FROM persona WHERE id = 1",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map(|j| j.and_then(|s| serde_json::from_str(&s).ok()))
}

fn build_summary(preferences: &[String], facts: &[String], skills: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !preferences.is_empty() {
        parts.push(format!("{} preference(s) on record.", preferences.len()));
    }
    if !facts.is_empty() {
        parts.push(format!("{} fact(s) about the user.", facts.len()));
    }
    if !skills.is_empty() {
        parts.push(format!("{} workflow(s) known.", skills.len()));
    }
    if parts.is_empty() {
        "No consolidated memories yet.".to_string()
    } else {
        format!("Persona from {} approved memories — {}.", facts.len() + preferences.len() + skills.len(), parts.join(" "))
    }
}

/// Full consolidation pass: dedupe → conflicts (reported) → decay → persona regen if due.
pub fn consolidate(store: &MemoryStore, now: &str, half_life_days: f64) -> rusqlite::Result<ConsolidationReport> {
    let deduped = dedupe(store)?;
    let conflicts = detect_conflicts(store)?.len();
    let decayed = if half_life_days > 0.0 {
        apply_decay(store, now, half_life_days, DECAY_DROP)?
    } else {
        0
    };

    let approved = store.count_approved()?;
    let last_regen: i64 = store.meta("persona_regen_at_count")?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let regen_due = approved >= last_regen + PERSONA_REGEN_INTERVAL;
    let persona = if regen_due {
        let p = regenerate_persona(store)?;
        store.set_meta("persona_regen_at_count", &approved.to_string())?;
        Some(p)
    } else {
        None
    };

    Ok(ConsolidationReport {
        deduped,
        conflicts,
        decayed,
        persona_regenerated: regen_due,
        persona,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{ApprovalStatus, MemoryScope, MemoryStore, ScopeType, StoreConfig, new_id};

    fn store() -> MemoryStore {
        MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap()
    }

    fn approved_event(content: &str, kind: MemoryKind) -> MemoryEvent {
        MemoryEvent {
            id: new_id("mem"),
            kind,
            content: content.to_string(),
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

    fn insert_approved(s: &MemoryStore, ev: MemoryEvent) {
        let mut e = ev;
        e.approval = ApprovalStatus::Approved;
        s.insert_memory(&e).unwrap();
    }

    #[test]
    fn dedupe_merges_exact_and_near_duplicates() {
        let s = store();
        insert_approved(&s, approved_event("User prefers TypeScript for new services.", MemoryKind::Semantic));
        insert_approved(&s, approved_event("User prefers TypeScript for new services.", MemoryKind::Semantic));
        insert_approved(&s, approved_event("User prefers TypeScript for new services. (edited)", MemoryKind::Semantic));
        insert_approved(&s, approved_event("User prefers Rust for CLIs.", MemoryKind::Semantic));
        let deduped = dedupe(&s).unwrap();
        assert_eq!(deduped, 2);
        assert_eq!(s.list_approved().unwrap().len(), 2);
    }

    #[test]
    fn conflicts_detected_on_opposite_polarity() {
        let s = store();
        insert_approved(&s, approved_event("User prefers dark mode in the editor.", MemoryKind::Semantic));
        insert_approved(&s, approved_event("User hates dark mode in the editor.", MemoryKind::Semantic));
        let conflicts = detect_conflicts(&s).unwrap();
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn decay_drops_stale_low_confidence() {
        let s = store();
        let old = chrono::Utc::now() - chrono::Duration::days(400);
        let mut ev = approved_event("An old working note that has aged.", MemoryKind::Working);
        ev.created_at = old.to_rfc3339();
        insert_approved(&s, ev);
        let now = chrono::Utc::now().to_rfc3339();
        let _decayed = apply_decay(&s, &now, 30.0, 0.1).unwrap();
        // confidence 0.8 * 0.5^(400/30) ≈ 0.8 * 9.7e-5 ≈ 7.8e-5 → below 0.1 → deleted
        assert_eq!(s.count_memories().unwrap(), 0);
    }

    #[test]
    fn persona_regenerates_from_approved_memories() {
        let s = store();
        insert_approved(&s, approved_event("User prefers TypeScript.", MemoryKind::Semantic));
        insert_approved(&s, approved_event("User works at a startup.", MemoryKind::Semantic));
        insert_approved(&s, approved_event("To deploy run the script.", MemoryKind::Procedural));
        let p = regenerate_persona(&s).unwrap();
        assert_eq!(p.version, 1);
        assert!(p.preferences.iter().any(|x| x.contains("TypeScript")));
        assert!(p.facts.iter().any(|x| x.contains("startup")));
        assert_eq!(p.skills.len(), 1);
        assert_eq!(p.memories_count, 3);
        // persisted
        let loaded = get_persona(&s).unwrap().unwrap();
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn consolidate_reports_and_regens_when_due() {
        let s = store();
        for i in 0..PERSONA_REGEN_INTERVAL {
            insert_approved(&s, approved_event(&format!("User fact number {i}."), MemoryKind::Semantic));
        }
        let report = consolidate(&s, &now_iso(), 365.0).unwrap();
        assert!(report.persona_regenerated);
        assert!(report.persona.is_some());
        assert_eq!(report.conflicts, 0);
    }
}
