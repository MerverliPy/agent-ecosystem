//! Skill integration: install/update/remove skills from a SkillHub registry
//! (matching the Phase 3 registry API + skillhub.json manifest + skillhub.lock.json
//! format). Installed skills surface as *procedural memory proposals* (approval-gated).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::approvals::propose;
use crate::store::{ApprovalStatus, MemoryEvent, MemoryKind, MemoryScope, MemorySource, MemoryStore, ScopeType, new_id};
use crate::RuntimeError;

pub const SKILLHUB_DEFAULT_REGISTRY: &str = "http://127.0.0.1:8787";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub harnesses: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Lockfile format — compatible with apps/skillhub-cli's skillhub.lock.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLock {
    pub name: String,
    pub version: String,
    pub registry: String,
    pub installed_at: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    pub manifest: SkillManifest,
    pub lock: SkillLock,
    pub dir: String,
    pub memory_proposal_id: Option<String>,
}

fn registry_detail(registry: &str, owner: &str, name: &str) -> Result<serde_json::Value, RuntimeError> {
    let url = format!("{registry}/api/packages/{owner}/{name}");
    ureq::get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(RuntimeError::from)?
        .into_json()
        .map_err(|e| RuntimeError::Parse(e.to_string()))
}

fn registry_files(registry: &str, owner: &str, name: &str, version: &str) -> Result<HashMap<String, String>, RuntimeError> {
    let url = format!("{registry}/api/packages/{owner}/{name}/{version}/files");
    let resp: serde_json::Value = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(RuntimeError::from)?
        .into_json()
        .map_err(|e| RuntimeError::Parse(e.to_string()))?;
    let files = resp
        .get("files")
        .and_then(|f| f.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .ok_or_else(|| RuntimeError::Parse("missing files map".into()))?;
    Ok(files)
}

fn latest_version(detail: &serde_json::Value) -> Option<String> {
    detail
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|v| v.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Install a skill into `skills_dir/{owner}__{name}`. Path-traversal guarded
/// (mirrors the skillhub-cli guards). Returns the installed skill; a procedural
/// memory proposal is created when `store` is provided.
pub fn install_skill(
    store: &MemoryStore,
    registry: &str,
    owner: &str,
    name: &str,
    version: Option<String>,
    skills_dir: &Path,
) -> Result<InstalledSkill, RuntimeError> {
    if !owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(RuntimeError::Io("invalid owner/name (path traversal guard)".into()));
    }

    let detail = registry_detail(registry, owner, name)?;
    let version = match version {
        Some(v) => v,
        None => latest_version(&detail).ok_or_else(|| RuntimeError::Parse("no versions published".into()))?,
    };
    let files = registry_files(registry, owner, name, &version)?;

    // validate the manifest
    let manifest_raw = files
        .get("skillhub.json")
        .ok_or_else(|| RuntimeError::Parse("skillhub.json missing from package".into()))?;
    let manifest: SkillManifest = serde_json::from_str(manifest_raw)
        .map_err(|e| RuntimeError::Parse(format!("invalid skillhub.json: {e}")))?;

    let dir = skills_dir.join(format!("{owner}__{name}"));
    std::fs::create_dir_all(&dir).map_err(RuntimeError::from)?;

    let mut written = Vec::new();
    for (path, content) in &files {
        let rel = Path::new(path);
        // guard: no absolute paths or parent traversal
        if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            continue;
        }
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(RuntimeError::from)?;
        }
        std::fs::write(&target, content).map_err(RuntimeError::from)?;
        written.push(path.clone());
    }

    let lock = SkillLock {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        registry: registry.to_string(),
        installed_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        files: written,
    };
    let lock_json = serde_json::to_string_pretty(&lock).map_err(|e| RuntimeError::Parse(e.to_string()))?;
    std::fs::write(dir.join("skillhub.lock.json"), lock_json).map_err(RuntimeError::from)?;

    // surface as procedural memory (approval-gated, DEC-0009)
    let memory_proposal_id = create_skill_memory_proposal(store, owner, name, &manifest);

    Ok(InstalledSkill {
        manifest,
        lock,
        dir: dir.to_string_lossy().into_owned(),
        memory_proposal_id,
    })
}

fn create_skill_memory_proposal(
    store: &MemoryStore,
    owner: &str,
    name: &str,
    manifest: &SkillManifest,
) -> Option<String> {
    let ev = MemoryEvent {
        id: new_id("proc"),
        kind: MemoryKind::Procedural,
        content: format!(
            "Installed skill {}/{} v{} — runnable in {}",
            owner,
            name,
            manifest.version,
            manifest.harnesses.join(", ")
        ),
        summary: Some(format!("Skill: {}", manifest.name)),
        source: MemorySource::Api,
        confidence: 0.9,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        updated_at: None,
        episode_id: None,
        scope: MemoryScope {
            scope_type: ScopeType::Companion,
            project_id: None,
            project_path: None,
        },
        approval: ApprovalStatus::Pending,
        tags: Some(vec!["skill".into(), manifest.name.clone()]),
        embedding: None,
    };
    propose(store, &ev, format!("Skill installed: {}", manifest.name))
        .ok()
        .map(|_| ev.id)
}

pub fn remove_skill(skills_dir: &Path, owner: &str, name: &str) -> Result<(), RuntimeError> {
    let dir = skills_dir.join(format!("{owner}__{name}"));
    if !dir.exists() {
        return Err(RuntimeError::Io("skill not installed".into()));
    }
    std::fs::remove_dir_all(&dir).map_err(RuntimeError::from)
}

pub fn installed_skills(skills_dir: &Path) -> Result<Vec<SkillLock>, RuntimeError> {
    let mut out = Vec::new();
    if !skills_dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(skills_dir).map_err(RuntimeError::from)? {
        let entry = entry.map_err(RuntimeError::from)?;
        let lock_path = entry.path().join("skillhub.lock.json");
        if lock_path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&lock_path) {
                if let Ok(lock) = serde_json::from_str::<SkillLock>(&raw) {
                    out.push(lock);
                }
            }
        }
    }
    Ok(out)
}

/// Local-only fallback: install from a directory that already contains a skill.
pub fn install_from_dir(store: &MemoryStore, src: &Path, skills_dir: &Path) -> Result<InstalledSkill, RuntimeError> {
    let manifest_raw = std::fs::read_to_string(src.join("skillhub.json"))
        .map_err(|_| RuntimeError::Parse("skillhub.json missing".into()))?;
    let manifest: SkillManifest =
        serde_json::from_str(&manifest_raw).map_err(|e| RuntimeError::Parse(e.to_string()))?;
    let dir = skills_dir.join(format!("local__{}", manifest.name));
    std::fs::create_dir_all(&dir).map_err(RuntimeError::from)?;
    let mut written = Vec::new();
    for entry in std::fs::read_dir(src).map_err(RuntimeError::from)? {
        let entry = entry.map_err(RuntimeError::from)?;
        if entry.file_type().map_err(RuntimeError::from)?.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            std::fs::copy(entry.path(), dir.join(&name)).map_err(RuntimeError::from)?;
            written.push(name);
        }
    }
    let lock = SkillLock {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        registry: "local".into(),
        installed_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        files: written,
    };
    std::fs::write(dir.join("skillhub.lock.json"), serde_json::to_string_pretty(&lock).unwrap())
        .map_err(RuntimeError::from)?;
    let _ = store;
    Ok(InstalledSkill {
        manifest,
        lock,
        dir: dir.to_string_lossy().into_owned(),
        memory_proposal_id: None,
    })
}

pub fn skills_dir_from(base: &Path) -> PathBuf {
    base.join("skills")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_server::serve;
    use crate::store::{MemoryStore, StoreConfig};
    use std::sync::Arc;

    const MANIFEST: &str = r#"{"name":"demo-skill","version":"1.0.0","harnesses":["pi"],"description":"demo","license":"MIT"}"#;
    const SKILL_MD: &str = "# demo skill\n\nRun: echo hello";

    fn registry_server() -> String {
        serve(Arc::new(|_method, path| {
            if path.starts_with("/api/packages/demo/demo-skill") && !path.ends_with("/files") {
                (
                    200,
                    r#"{"name":"demo-skill","description":"demo","license":"MIT","repo":"","verified":true,"high_risk":false,"downloads":1,"versions":[{"version":"1.0.0","published_at":"2026-08-15T00:00:00Z","verified":true,"harnesses":["pi"],"permissions":[]}]}"#.into(),
                )
            } else if path.ends_with("/files") {
                (
                    200,
                    format!(r#"{{"files":{{"skillhub.json":{MANIFEST:?},"SKILL.md":{SKILL_MD:?}}}}}"#),
                )
            } else {
                (404, "{}".into())
            }
        }))
    }

    #[test]
    fn installs_skill_and_creates_memory_proposal() {
        let store = MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap();
        let tmp = std::env::temp_dir().join(format!("slop-skills-{}", uuid::Uuid::new_v4()));
        let installed = install_skill(&store, &registry_server(), "demo", "demo-skill", None, &tmp).unwrap();
        assert_eq!(installed.manifest.name, "demo-skill");
        assert_eq!(installed.manifest.harnesses, vec!["pi".to_string()]);
        assert!(tmp.join("demo__demo-skill").join("SKILL.md").exists());
        assert!(tmp.join("demo__demo-skill").join("skillhub.lock.json").exists());
        // procedural memory proposal exists (pending)
        assert!(installed.memory_proposal_id.is_some());
        let pending = store.list_by_approval(ApprovalStatus::Pending).unwrap();
        assert!(pending.iter().any(|m| m.kind == MemoryKind::Procedural && m.content.contains("demo-skill")));
        // listed as installed
        let listed = installed_skills(&tmp).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].version, "1.0.0");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn traversal_guards_block_bad_names() {
        let store = MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap();
        let tmp = std::env::temp_dir().join("guard-test");
        assert!(install_skill(&store, "http://x", "..", "skill", None, &tmp).is_err());
    }

    #[test]
    fn install_from_local_dir_works() {
        let store = MemoryStore::open(StoreConfig { path: ":memory:".into(), encrypt: false }).unwrap();
        let src = std::env::temp_dir().join(format!("skill-src-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("skillhub.json"), MANIFEST).unwrap();
        std::fs::write(src.join("SKILL.md"), SKILL_MD).unwrap();
        let dst = std::env::temp_dir().join(format!("skill-dst-{}", uuid::Uuid::new_v4()));
        let _installed = install_from_dir(&store, &src, &dst).unwrap();
        assert!(dst.join("local__demo-skill").join("SKILL.md").exists());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }
}
