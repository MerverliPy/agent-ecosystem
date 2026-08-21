use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkgSummary {
    pub name: String,
    pub description: String,
    pub version: String,
    pub verified: bool,
    pub high_risk: bool,
    pub downloads: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub published_at: String,
    pub verified: bool,
    pub harnesses: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PkgDetail {
    pub name: String,
    pub description: String,
    pub license: String,
    pub repo: String,
    pub verified: bool,
    pub high_risk: bool,
    pub downloads: u64,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PkgFiles {
    pub files: HashMap<String, String>,
}

pub struct Client {
    pub base: String,
}

impl Client {
    pub fn new(base: &str) -> Self {
        Self { base: base.trim_end_matches('/').to_string() }
    }

    pub fn search(&self, q: &str) -> anyhow::Result<Vec<PkgSummary>> {
        let url = format!("{}/api/search?q={}", self.base, q);
        let resp = ureq::get(&url).call()?;
        Ok(resp.into_json()?)
    }

    pub fn info(&self, name: &str) -> anyhow::Result<PkgDetail> {
        let (owner, n) = split_name(name)?;
        let url = format!("{}/api/packages/{}/{}", self.base, owner, n);
        let resp = ureq::get(&url).call()?;
        Ok(resp.into_json()?)
    }

    pub fn files(&self, name: &str, version: &str) -> anyhow::Result<PkgFiles> {
        let (owner, n) = split_name(name)?;
        let url = format!("{}/api/packages/{}/{}/{}/files", self.base, owner, n, version);
        let resp = ureq::get(&url).call()?;
        Ok(resp.into_json()?)
    }

    /// Register an owner and mint its first publish capability token.
    pub fn register_owner(&self, owner: &str) -> anyhow::Result<(String, String)> {
        let url = format!("{}/api/owners/register", self.base);
        let body = serde_json::json!({ "owner": owner });
        let resp = ureq::post(&url).send_json(&body)?;
        let v: serde_json::Value = resp.into_json()?;
        let owner_out = v["owner"].as_str().unwrap_or(owner).to_string();
        let token = v["token"].as_str().ok_or_else(|| anyhow::anyhow!("register did not return a token"))?.to_string();
        Ok((owner_out, token))
    }

    /// Revoke a capability token by presenting it (self-revocation).
    pub fn revoke_token(&self, token: &str) -> anyhow::Result<u16> {
        let url = format!("{}/api/owners/revoke", self.base);
        let body = serde_json::json!({ "token": token });
        let resp = ureq::post(&url).send_json(&body)?;
        Ok(resp.status())
    }

    /// Publish a package. Requires a capability token for the owning namespace.
    pub fn publish(&self, payload: &serde_json::Value, token: Option<&str>) -> anyhow::Result<u16> {
        let url = format!("{}/api/publish", self.base);
        let mut req = ureq::post(&url);
        if let Some(t) = token {
            req = req.set("Authorization", &format!("Bearer {t}"));
        }
        let resp = req.send_json(payload)?;
        Ok(resp.status())
    }
}

fn split_name(name: &str) -> anyhow::Result<(String, String)> {
    let mut it = name.splitn(2, '/');
    let owner = it.next().unwrap_or("").to_string();
    let n = it.next().unwrap_or("").to_string();
    if owner.is_empty() || n.is_empty() {
        anyhow::bail!("invalid package name '{}' — expected owner/name", name);
    }
    Ok((owner, n))
}
