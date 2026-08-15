use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    pub name: String,
    pub version: String,
    pub source: String,
    pub harness: String,
    pub installed_at: String,
    pub checksum: String,
}

pub fn lock_path(dir: &Path) -> std::path::PathBuf {
    dir.join("skillhub.lock.json")
}

pub fn read(dir: &Path) -> Vec<LockEntry> {
    fs::read_to_string(lock_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write(dir: &Path, entries: &[LockEntry]) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(lock_path(dir), serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

pub fn upsert(dir: &Path, entry: LockEntry) -> anyhow::Result<()> {
    let mut v = read(dir);
    v.retain(|e| e.name != entry.name);
    v.push(entry);
    v.sort_by(|a, b| a.name.cmp(&b.name));
    write(dir, &v)
}

pub fn find<'a>(dir: &Path, name: &str) -> Option<LockEntry> {
    read(dir).into_iter().find(|e| e.name == name)
}

pub fn remove(dir: &Path, name: &str) -> anyhow::Result<bool> {
    let mut v = read(dir);
    let before = v.len();
    v.retain(|e| e.name != name);
    if v.len() == before {
        return Ok(false);
    }
    write(dir, &v)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_remove_roundtrip() {
        let dir = std::env::temp_dir().join(format!("skillhub-lock-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let e = LockEntry {
            name: "demo/hello-skill".into(),
            version: "1.0.0".into(),
            source: "http://registry".into(),
            harness: "pi".into(),
            installed_at: "now".into(),
            checksum: "abc".into(),
        };
        upsert(&dir, e.clone()).unwrap();
        assert_eq!(read(&dir).len(), 1);
        assert!(find(&dir, "demo/hello-skill").is_some());
        assert!(remove(&dir, "demo/hello-skill").unwrap());
        assert!(read(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
