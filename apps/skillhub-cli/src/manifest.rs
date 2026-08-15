use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: String,
    pub repo: String,
    pub harnesses: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default = "default_entry")]
    pub entrypoint: String,
}

fn default_entry() -> String {
    "SKILL.md".to_string()
}

const PERMISSIVE_LICENSES: &[&str] = &[
    "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "MIT-0", "Zlib", "Unlicense", "CC0-1.0", "MPL-2.0",
];
const KNOWN_HARNESSES: &[&str] = &[
    "claude-code", "codex", "cursor", "gemini-cli", "copilot", "pi", "openclaw",
];
const KNOWN_PERMISSIONS: &[&str] = &[
    "files.read", "files.write", "shell", "network", "secrets", "browser",
];

impl Manifest {
    pub fn from_json(s: &str) -> anyhow::Result<Self> {
        let m: Manifest = serde_json::from_str(s)?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let parts: Vec<&str> = self.name.split('/').collect();
        let name_ok = parts.len() == 2
            && parts.iter().all(|p| {
                !p.is_empty()
                    && p.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            });
        anyhow::ensure!(name_ok, "name must be owner/name, lowercase [a-z0-9-]");

        let v: Vec<&str> = self.version.split('.').collect();
        anyhow::ensure!(
            v.len() == 3 && v.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
            "version must be MAJOR.MINOR.PATCH"
        );

        anyhow::ensure!(
            !self.description.is_empty() && self.description.chars().count() <= 200,
            "description must be 1..200 chars"
        );
        anyhow::ensure!(
            PERMISSIVE_LICENSES.contains(&self.license.as_str()),
            "license '{}' is not permissive (DEC-0002)",
            self.license
        );
        anyhow::ensure!(!self.repo.is_empty(), "repo is required");
        anyhow::ensure!(
            !self.harnesses.is_empty() && self.harnesses.iter().all(|h| KNOWN_HARNESSES.contains(&h.as_str())),
            "harnesses must be a non-empty subset of {:?}",
            KNOWN_HARNESSES
        );
        let uniq: HashSet<&str> = self.harnesses.iter().map(|s| s.as_str()).collect();
        anyhow::ensure!(uniq.len() == self.harnesses.len(), "duplicate harnesses");

        for d in &self.dependencies {
            let dp: Vec<&str> = d.name.split('/').collect();
            anyhow::ensure!(dp.len() == 2 && dp.iter().all(|p| !p.is_empty()), "dependency name must be owner/name");
            let vv = d.version.strip_prefix('^').unwrap_or(&d.version);
            anyhow::ensure!(
                vv.split('.').count() == 3 && vv.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
                "dependency version must be MAJOR.MINOR.PATCH or ^MAJOR.MINOR.PATCH"
            );
        }

        anyhow::ensure!(
            self.permissions.iter().all(|p| KNOWN_PERMISSIONS.contains(&p.as_str())),
            "unknown permission in {:?}",
            self.permissions
        );
        Ok(())
    }

    pub fn is_high_risk(&self) -> bool {
        self.permissions.iter().any(|p| p == "shell" || p == "network")
    }
}

/// Numeric semver compare; returns Ordering.
pub fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> (u64, u64, u64) {
        let mut it = s.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
        (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
    };
    parse(a).cmp(&parse(b))
}
