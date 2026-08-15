use std::path::PathBuf;

pub struct Harness {
    pub id: &'static str,
    pub dir: PathBuf,
}

fn env_or(key: &str, fallback: PathBuf) -> PathBuf {
    std::env::var(key).map(PathBuf::from).unwrap_or(fallback)
}

/// All known harness skill directories (whether or not they exist).
pub fn candidates() -> Vec<Harness> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let h = PathBuf::from(&home);
    vec![
        Harness { id: "claude-code", dir: env_or("CLAUDE_CONFIG_DIR", h.join(".claude/skills")) },
        Harness { id: "codex", dir: env_or("CODEX_HOME", h.join(".codex/skills")) },
        Harness { id: "cursor", dir: h.join(".cursor/skills") },
        Harness { id: "gemini-cli", dir: h.join(".gemini/skills") },
        Harness { id: "copilot", dir: h.join(".copilot/skills") },
        Harness { id: "pi", dir: h.join(".pi/agent/skills") },
        Harness { id: "openclaw", dir: h.join(".openclaw/skills") },
    ]
}

/// Harnesses whose skills dir exists on this machine.
pub fn detected() -> Vec<Harness> {
    candidates().into_iter().filter(|h| h.dir.exists()).collect()
}

/// Resolve the install directory: explicit --dir > --harness > first detected > pi default.
pub fn resolve(harness: Option<&str>, dir: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(d) = dir {
        return Ok(PathBuf::from(d));
    }
    if let Some(h) = harness {
        let all = candidates();
        let names: Vec<&str> = all.iter().map(|c| c.id).collect();
        let m = all
            .iter()
            .find(|c| c.id == h)
            .ok_or_else(|| anyhow::anyhow!("unknown harness '{}'; known: {}", h, names.join(", ")))?;
        return Ok(m.dir.clone());
    }
    let det = detected();
    if !det.is_empty() {
        return Ok(det[0].dir.clone());
    }
    candidates()
        .into_iter()
        .find(|c| c.id == "pi")
        .map(|c| c.dir)
        .ok_or_else(|| anyhow::anyhow!("no harness detected on this machine"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_harness_known() {
        let all = candidates();
        assert!(all.iter().any(|h| h.id == "pi"));
    }

    #[test]
    fn unknown_harness_rejected() {
        assert!(resolve(Some("nope"), None).is_err());
    }
}
