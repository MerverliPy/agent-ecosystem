// SkillHub registry — axum + SQLite. Versioned, immutable package storage with search + download counts.
// Endpoints:
//   GET  /health
//   GET  /api/search?q=<text>                          -> Vec<Summary>
//   GET  /api/packages/{owner}/{name}                  -> Detail (latest verified flags + versions)
//   GET  /api/packages/{owner}/{name}/{version}/files  -> {files: {path: content}} (increments download count)
//   POST /api/publish                                  -> 201 | 409 | 400
//
// Canonical identity model: a package's canonical id is the string `owner/name` where each segment
// matches `[a-z0-9][a-z0-9-]*` (same grammar as the skill-manifest schema). Every handler resolves
// ids through `canonical_id()` so the write key-space (manifest.name) and the read key-space
// (URL owner/name) are guaranteed to coincide.
//
// DB reset (DECIDED 2026-08-15, planning-milestone-2.md §6.1): the registry DB is a runtime,
// gitignored, developer-seeded artifact with no real users. This build resets it under the
// canonical `owner/name` model. No migration code ships — `init()` creates the schema fresh.
use axum::{
    extract::{Path, Query, Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Db = Arc<Mutex<Connection>>;

#[derive(Clone)]
struct RateLimits {
    publish_ip: u32,    // per-IP publishes per window
    publish_token: u32, // per-token publishes per window
    read_global: u32,   // all reads per window
    window_secs: i64,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self { publish_ip: 30, publish_token: 60, read_global: 600, window_secs: 60 }
    }
}

/// Minimal in-memory fixed-window token bucket (equivalent of tower-governor).
#[derive(Clone)]
struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, FixedWindow>>>,
    limits: RateLimits,
}

#[derive(Default)]
struct FixedWindow {
    start: i64,
    count: u32,
}

impl RateLimiter {
    fn new(limits: RateLimits) -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())), limits }
    }

    /// Consume one token from `key`'s bucket in the current window. False => over limit.
    fn allow(&self, key: &str, limit: u32) -> bool {
        let now = now_epoch();
        let window_id = now / self.limits.window_secs;
        let mut m = self.inner.lock().unwrap();
        let e = m.entry(key.to_string()).or_default();
        if e.start != window_id {
            e.start = window_id;
            e.count = 0;
        }
        if e.count >= limit {
            return false;
        }
        e.count += 1;
        true
    }
}

#[derive(Clone)]
struct AppState {
    db: Db,
    /// HMAC signing secret for capability tokens. From env SKILLHUB_REGISTRY_SECRET
    /// (never logged, never embedded in code); a random in-process secret is used in dev.
    secret: Vec<u8>,
    limiter: RateLimiter,
}

/// Validate a single `owner` or `name` segment against the canonical grammar
/// `[a-z0-9][a-z0-9-]*` (lowercase alphanumeric, then alphanumeric or hyphen).
fn valid_segment(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Canonical package identifier: validate and normalize `owner/name`.
/// This is the single path every registry lookup and publish resolves an id through,
/// so the write key-space (manifest.name) and read key-space (URL owner/name) always agree.
fn canonical_id(id: &str) -> Result<String, String> {
    let mut it = id.split('/');
    let owner = it.next().unwrap_or("");
    let name = it.next().unwrap_or("");
    if it.next().is_some() || !valid_segment(owner) || !valid_segment(name) {
        return Err(format!(
            "invalid package id '{id}' — expected owner/name of [a-z0-9][a-z0-9-]* segments"
        ));
    }
    Ok(format!("{owner}/{name}"))
}

// ---------- capability tokens (self-contained, HMAC-signed) ----------
// A token is `base64url(claims).hex_hmac(secret)` where claims = {sub, scope, jti, exp}.
// Self-contained per DECIDED #2 (no external IdP); the registry secret authenticates them.
// Tokens are per-owner and scoped (e.g. `publish:<owner>`); revocation is layered in later.

#[derive(Debug, Clone)]
struct Claims {
    sub: String,
    scope: String,
    jti: String,
    exp: i64,
}

fn b64url(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).ok()
}

fn hmac_hex(secret: &[u8], data: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256>>::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().iter().map(|b| format!("{:02x}", b)).collect()
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

fn random_hex() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Mint a scoped capability token for an owner. Default expiry 30 days.
/// Returns the token string and its claims (the caller records the jti for revocation).
fn mint_token(secret: &[u8], owner: &str, scope: &str) -> (String, Claims) {
    let claims = Claims {
        sub: owner.to_string(),
        scope: scope.to_string(),
        jti: random_hex(),
        exp: now_epoch() + 86400 * 30,
    };
    let encoded = serde_json::json!({
        "sub": claims.sub, "scope": claims.scope, "jti": claims.jti, "exp": claims.exp
    })
    .to_string();
    let enc = b64url(encoded.as_bytes());
    let sig = hmac_hex(secret, enc.as_bytes());
    (format!("{enc}.{sig}"), claims)
}

/// Persist a minted capability so it can be revoked later.
fn record_capability(conn: &Connection, claims: &Claims) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO capabilities (jti, owner, scope, expires_at, revoked)
         VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![claims.jti, claims.sub, claims.scope, claims.exp],
    )?;
    Ok(())
}

fn owner_revoked(conn: &Connection, owner: &str) -> bool {
    conn.query_row("SELECT revoked FROM owners WHERE owner = ?1", [owner], |r| r.get::<_, i64>(0))
        .map(|r| r != 0)
        .unwrap_or(false)
}

fn owner_pubkey(conn: &Connection, owner: &str) -> Option<String> {
    conn.query_row("SELECT pubkey FROM owners WHERE owner = ?1", [owner], |r| r.get::<_, String>(0))
        .ok()
        .filter(|p| !p.is_empty())
}

/// True if the token's jti has been explicitly revoked.
fn is_revoked(conn: &Connection, jti: &str) -> bool {
    conn.query_row(
        "SELECT revoked FROM capabilities WHERE jti = ?1",
        [jti],
        |r| r.get::<_, i64>(0),
    )
    .map(|r| r != 0)
    .unwrap_or(false) // unknown jti is treated as not-revoked (self-contained token)
}

/// Revoke a capability token by jti.
fn revoke_capability(conn: &Connection, jti: &str) -> anyhow::Result<u16> {
    let n = conn.execute("UPDATE capabilities SET revoked = 1 WHERE jti = ?1", [jti])?;
    Ok(if n == 0 { 404 } else { 200 })
}

/// Verify a token's signature + expiry; returns its claims, or None if invalid.
fn verify_token(secret: &[u8], token: &str) -> Option<Claims> {
    let (enc, sig) = token.split_once('.')?;
    let expected = hmac_hex(secret, enc.as_bytes());
    if !constant_eq(sig.as_bytes(), expected.as_bytes()) {
        return None;
    }
    let payload = b64url_decode(enc)?;
    let v: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    let claims = Claims {
        sub: v.get("sub")?.as_str()?.to_string(),
        scope: v.get("scope")?.as_str()?.to_string(),
        jti: v.get("jti")?.as_str()?.to_string(),
        exp: v.get("exp")?.as_i64()?,
    };
    if claims.exp < now_epoch() {
        return None;
    }
    Some(claims)
}

/// Extract a `Bearer <token>` from the Authorization header.
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let h = headers.get(axum::http::header::AUTHORIZATION)?;
    let s = h.to_str().ok()?;
    let t = s.strip_prefix("Bearer ")?;
    if t.is_empty() {
        return None;
    }
    Some(t.to_string())
}

// ---------- input hardening ----------

// Embed the canonical skill-manifest JSON-schema (shared/). At rest this rejects manifests
// that violate name/version grammar, description bounds, license/harness/permission enums,
// and any unlisted field (additionalProperties: false).
const MANIFEST_SCHEMA: &str = include_str!("../../../shared/schemas/skill-manifest.schema.json");

fn manifest_validator() -> &'static jsonschema::Validator {
    static V: std::sync::OnceLock<jsonschema::Validator> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let schema: serde_json::Value = serde_json::from_str(MANIFEST_SCHEMA).expect("embedded schema is valid JSON");
        jsonschema::validator_for(&schema).expect("embedded schema compiles")
    })
}

const MAX_FILE_SIZE: usize = 2 * 1024 * 1024; // 2 MiB per file
const MAX_TOTAL_SIZE: usize = 10 * 1024 * 1024; // 10 MiB per publish
const MAX_FILES: usize = 1000;

/// Reject absolute paths, backslash separators, `.`/`..`/empty segments.
fn safe_rel_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.split('/').any(|seg| seg == ".." || seg == "." || seg.is_empty())
}

fn validate_files(files: &HashMap<String, String>) -> Result<(), &'static str> {
    if files.len() > MAX_FILES {
        return Err("too many files");
    }
    let mut total = 0usize;
    for (path, content) in files {
        if !safe_rel_path(path) {
            return Err("unsafe file path (absolute, traversal, or empty segment)");
        }
        if content.len() > MAX_FILE_SIZE {
            return Err("file exceeds size cap");
        }
        total += content.len();
    }
    if total > MAX_TOTAL_SIZE {
        return Err("package exceeds total size cap");
    }
    Ok(())
}

// ---------- package signing (registry CA issues per-owner Ed25519 keys) ----------
// The registry acts as a CA: on registration it generates a per-owner Ed25519 keypair, stores
// the public key, and returns the signing key to the owner. Publishers sign the canonical
// package digest; the registry verifies the signature against the owner's stored public key.

use ed25519_dalek::{Signer, Verifier};

fn owner_keypair() -> ed25519_dalek::SigningKey {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    ed25519_dalek::SigningKey::from_bytes(&bytes)
}

fn pubkey_b64(key: &ed25519_dalek::SigningKey) -> String {
    b64url(key.verifying_key().as_bytes())
}

fn signing_key_b64(key: &ed25519_dalek::SigningKey) -> String {
    b64url(key.as_bytes())
}

#[cfg(test)]
fn signing_key_from_b64(s: &str) -> Option<ed25519_dalek::SigningKey> {
    let bytes = b64url_decode(s)?;
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(ed25519_dalek::SigningKey::from_bytes(&arr))
}

fn pubkey_from_b64(s: &str) -> Option<ed25519_dalek::VerifyingKey> {
    let bytes = b64url_decode(s)?;
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    ed25519_dalek::VerifyingKey::from_bytes(&arr).ok()
}

/// Canonical byte string signed by the publisher: the manifest bytes followed by each file
/// (paths sorted, length-prefixed). Both CLI and registry compute this identically.
fn package_digest_input(manifest: &serde_json::Value, files: &HashMap<String, String>) -> Vec<u8> {
    let mut out = serde_json::to_vec(manifest).unwrap_or_default();
    let mut paths: Vec<&String> = files.keys().collect();
    paths.sort();
    for p in paths {
        out.extend_from_slice(format!("{p}\x00{}\n", files[p].len()).as_bytes());
        out.extend_from_slice(files[p].as_bytes());
    }
    out
}

#[cfg(test)]
fn sign_package(signing_key: &ed25519_dalek::SigningKey, manifest: &serde_json::Value, files: &HashMap<String, String>) -> String {
    let digest = package_digest_input(manifest, files);
    b64url(&signing_key.sign(&digest).to_bytes())
}

// ---------- schema + db layer ----------

fn init(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS packages (
            name TEXT PRIMARY KEY,
            description TEXT NOT NULL,
            license TEXT NOT NULL,
            repo TEXT NOT NULL,
            verified INTEGER NOT NULL DEFAULT 0,
            high_risk INTEGER NOT NULL DEFAULT 0,
            downloads INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS versions (
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            harnesses TEXT NOT NULL,
            permissions TEXT NOT NULL,
            entrypoint TEXT NOT NULL,
            scan_json TEXT NOT NULL,
            published_at TEXT NOT NULL,
            PRIMARY KEY (name, version)
        );
        CREATE TABLE IF NOT EXISTS files (
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            path TEXT NOT NULL,
            content TEXT NOT NULL,
            PRIMARY KEY (name, version, path)
        );
        CREATE TABLE IF NOT EXISTS owners (
            owner TEXT PRIMARY KEY,
            pubkey TEXT NOT NULL DEFAULT '',
            revoked INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS capabilities (
            jti TEXT PRIMARY KEY,
            owner TEXT NOT NULL,
            scope TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            revoked INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct Summary {
    name: String,
    description: String,
    version: String,
    verified: bool,
    high_risk: bool,
    downloads: u64,
}

#[derive(Debug, Serialize)]
struct VersionInfo {
    version: String,
    published_at: String,
    verified: bool,
    harnesses: Vec<String>,
    permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Detail {
    name: String,
    description: String,
    license: String,
    repo: String,
    verified: bool,
    high_risk: bool,
    downloads: u64,
    versions: Vec<VersionInfo>,
}

fn search_db(conn: &Connection, q: &str) -> anyhow::Result<Vec<Summary>> {
    let like = format!("%{}%", q);
    let mut stmt = conn.prepare(
        "SELECT p.name, p.description, p.license, p.repo, p.verified, p.high_risk, p.downloads,
                (SELECT v.version FROM versions v WHERE v.name = p.name ORDER BY v.published_at DESC LIMIT 1)
         FROM packages p
         WHERE p.name LIKE ?1 OR p.description LIKE ?1
         ORDER BY p.downloads DESC, p.name",
    )?;
    let rows = stmt.query_map([&like], |r| {
        Ok(Summary {
            name: r.get(0)?,
            description: r.get(1)?,
            version: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
            verified: r.get(4)?,
            high_risk: r.get(5)?,
            downloads: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn detail_db(conn: &Connection, name: &str) -> anyhow::Result<Option<Detail>> {
    let mut stmt = conn.prepare("SELECT description, license, repo, verified, high_risk, downloads FROM packages WHERE name = ?1")?;
    let mut rows = stmt.query_map([name], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, u64>(5)?,
        ))
    })?;
    let Some(row) = rows.next() else {
        return Ok(None);
    };
    let (description, license, repo, verified, high_risk, downloads) = row?;

    let mut vstmt = conn.prepare(
        "SELECT version, published_at, scan_json, harnesses, permissions FROM versions WHERE name = ?1 ORDER BY published_at DESC",
    )?;
    let versions = vstmt
        .query_map([name], |r| {
            let scan: String = r.get(2)?;
            let verified_v: bool = serde_json::from_str(&scan)
                .map(|s: serde_json::Value| s["verified"].as_bool().unwrap_or(false))
                .unwrap_or(false);
            let harnesses: Vec<String> = serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default();
            let permissions: Vec<String> = serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default();
            Ok(VersionInfo {
                version: r.get(0)?,
                published_at: r.get(1)?,
                verified: verified_v,
                harnesses,
                permissions,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(Detail {
        name: name.to_string(),
        description,
        license,
        repo,
        verified,
        high_risk,
        downloads,
        versions,
    }))
}

fn files_db(conn: &Connection, name: &str, version: &str) -> anyhow::Result<Option<HashMap<String, String>>> {
    let mut stmt = conn.prepare("SELECT path, content FROM files WHERE name = ?1 AND version = ?2 ORDER BY path")?;
    let mut rows = stmt.query_map(rusqlite::params![name, version], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut files = HashMap::new();
    while let Some(row) = rows.next() {
        let (p, c) = row?;
        files.insert(p, c);
    }
    if files.is_empty() {
        return Ok(None);
    }
    Ok(Some(files))
}

fn bump_downloads(conn: &Connection, name: &str) -> anyhow::Result<()> {
    conn.execute("UPDATE packages SET downloads = downloads + 1 WHERE name = ?1", [name])?;
    Ok(())
}

// ---------- HTTP handlers ----------

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

async fn search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let q = q.q.unwrap_or_default();
    let db = state.db.lock().unwrap();
    match search_db(&db, &q) {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn pkg_detail(State(state): State<AppState>, Path((owner, name)): Path<(String, String)>) -> impl IntoResponse {
    let full = match canonical_id(&format!("{owner}/{name}")) {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };
    let db = state.db.lock().unwrap();
    match detail_db(&db, &full) {
        Ok(Some(d)) => (StatusCode::OK, Json(d)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn pkg_files(State(state): State<AppState>, Path((owner, name, version)): Path<(String, String, String)>) -> impl IntoResponse {
    let full = match canonical_id(&format!("{owner}/{name}")) {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };
    let db = state.db.lock().unwrap();
    match files_db(&db, &full, &version) {
        Ok(Some(files)) => {
            let _ = bump_downloads(&db, &full);
            (StatusCode::OK, Json(serde_json::json!({"files": files}))).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "package or version not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct PublishPayload {
    manifest: serde_json::Value,
    files: HashMap<String, String>,
    scan: serde_json::Value,
    /// base64url Ed25519 signature over the canonical package digest (manifest + files).
    #[serde(default)]
    signature: Option<String>,
}

fn publish_db(conn: &mut Connection, p: &PublishPayload) -> anyhow::Result<u16> {
    let m = &p.manifest;
    let raw_name = m["name"].as_str().ok_or_else(|| anyhow::anyhow!("manifest.name required"))?;
    let name = match canonical_id(raw_name) {
        Ok(n) => n,
        Err(_) => return Ok(400), // invalid owner/name grammar — write key-space must match read key-space
    };
    let version = m["version"].as_str().ok_or_else(|| anyhow::anyhow!("manifest.version required"))?.to_string();
    let description = m["description"].as_str().unwrap_or("").to_string();
    let license = m["license"].as_str().unwrap_or("").to_string();
    let repo = m["repo"].as_str().unwrap_or("").to_string();
    let harnesses: Vec<String> = m["harnesses"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let permissions: Vec<String> = m["permissions"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let entrypoint = m["entrypoint"].as_str().unwrap_or("SKILL.md").to_string();

    // JSON-schema validation of the manifest (name/version grammar, enums, sizes, extra props)
    if let Err(errs) = manifest_validator().validate(m) {
        let _ = errs;
        return Ok(400);
    }
    if name.is_empty() || version.is_empty() || description.is_empty() {
        return Ok(400);
    }
    // file path + size caps
    if let Err(reason) = validate_files(&p.files) {
        let _ = reason;
        return Ok(400);
    }
    let high_risk = permissions.iter().any(|p| p == "shell" || p == "network");
    let verified = p.scan["verified"].as_bool().unwrap_or(false);
    let scan_json = serde_json::to_string(&p.scan)?;

    let tx = conn.transaction()?;
    let exists: Option<i64> = tx
        .query_row("SELECT 1 FROM versions WHERE name = ?1 AND version = ?2", rusqlite::params![name, version], |r| r.get(0))
        .optional()?;
    if exists.is_some() {
        return Ok(409);
    }
    tx.execute(
        "INSERT OR IGNORE INTO packages (name, description, license, repo, verified, high_risk, downloads, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        rusqlite::params![
            name,
            description,
            license,
            repo,
            if verified { 1 } else { 0 },
            if high_risk { 1 } else { 0 },
            now_iso()
        ],
    )?;
    tx.execute(
        "INSERT INTO versions (name, version, harnesses, permissions, entrypoint, scan_json, published_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            name,
            version,
            serde_json::to_string(&harnesses)?,
            serde_json::to_string(&permissions)?,
            entrypoint,
            scan_json,
            now_iso()
        ],
    )?;
    for (path, content) in &p.files {
        tx.execute(
            "INSERT INTO files (name, version, path, content) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, version, path, content],
        )?;
    }
    tx.commit()?;
    // refresh package-level verified flag to the newest version's status
    conn.execute("UPDATE packages SET verified = ?1, high_risk = ?2 WHERE name = ?3", rusqlite::params![if verified { 1 } else { 0 }, if high_risk { 1 } else { 0 }, name])?;
    Ok(201)
}

#[derive(Debug, Deserialize)]
struct RegisterReq {
    owner: String,
}

/// Register an owner and mint its first capability token (self-issued, scoped to `publish:<owner>`).
async fn register_owner(State(state): State<AppState>, Json(payload): Json<RegisterReq>) -> impl IntoResponse {
    if !valid_segment(&payload.owner) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid owner — must match [a-z0-9][a-z0-9-]*"}))).into_response();
    }
    // the registry CA issues a fresh per-owner Ed25519 keypair; only the public key is stored
    let keypair = owner_keypair();
    let pubkey = pubkey_b64(&keypair);
    let db = state.db.lock().unwrap();
    if let Err(e) = db.execute(
        "INSERT OR REPLACE INTO owners (owner, pubkey, revoked, created_at) VALUES (?1, ?2, 0, ?3)",
        rusqlite::params![payload.owner, pubkey, now_iso()],
    ) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }
    let (token, claims) = mint_token(&state.secret, &payload.owner, &format!("publish:{}", payload.owner));
    if let Err(e) = record_capability(&db, &claims) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }
    (StatusCode::CREATED, Json(serde_json::json!({
        "owner": payload.owner, "token": token,
        "pubkey": pubkey, "signing_key": signing_key_b64(&keypair),
    }))).into_response()
}

#[derive(Debug, Deserialize)]
struct RotateReq {
    token: String,
}

/// Roll over the owner's signing key: the CA issues a fresh keypair and the owner keeps the
/// new signing key. The previous public key stops verifying — effectively rotating the key.
async fn rotate_owner_key(State(state): State<AppState>, Json(payload): Json<RotateReq>) -> impl IntoResponse {
    let claims = match verify_token(&state.secret, &payload.token) {
        Some(c) => c,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid or expired token"}))).into_response(),
    };
    let db = state.db.lock().unwrap();
    if is_revoked(&db, &claims.jti) || owner_revoked(&db, &claims.sub) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "token or owner revoked"}))).into_response();
    }
    let keypair = owner_keypair();
    let pubkey = pubkey_b64(&keypair);
    if let Err(e) = db.execute("UPDATE owners SET pubkey = ?2 WHERE owner = ?1", rusqlite::params![claims.sub, pubkey]) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }
    (StatusCode::OK, Json(serde_json::json!({
        "owner": claims.sub, "pubkey": pubkey, "signing_key": signing_key_b64(&keypair),
    }))).into_response()
}

/// Revoke an owner namespace: mark it revoked and revoke all of its capability tokens.
async fn revoke_owner(State(state): State<AppState>, Json(payload): Json<RotateReq>) -> impl IntoResponse {
    let claims = match verify_token(&state.secret, &payload.token) {
        Some(c) => c,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid or expired token"}))).into_response(),
    };
    let db = state.db.lock().unwrap();
    if let Err(e) = db.execute("UPDATE owners SET revoked = 1 WHERE owner = ?1", [&claims.sub]) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }
    let _ = db.execute("UPDATE capabilities SET revoked = 1 WHERE owner = ?1", [&claims.sub]);
    (StatusCode::OK, Json(serde_json::json!({"status": "owner revoked", "owner": claims.sub}))).into_response()
}

#[derive(Debug, Deserialize)]
struct RevokeReq {
    token: String,
}

/// Revoke a capability token by presenting it (self-revocation). The token's jti is marked
/// revoked; any further publish with it is rejected.
async fn revoke_token(State(state): State<AppState>, Json(payload): Json<RevokeReq>) -> impl IntoResponse {
    let claims = match verify_token(&state.secret, &payload.token) {
        Some(c) => c,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid or expired token"}))).into_response(),
    };
    let db = state.db.lock().unwrap();
    match revoke_capability(&db, &claims.jti) {
        Ok(200) => (StatusCode::OK, Json(serde_json::json!({"status": "revoked"}))).into_response(),
        Ok(404) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "token not on record"}))).into_response(),
        Ok(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "unexpected"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn publish(State(state): State<AppState>, headers: HeaderMap, Json(payload): Json<PublishPayload>) -> impl IntoResponse {
    // authenticate: a valid, unexpired capability token must be presented
    let token = match bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "authentication required: send a Bearer capability token"}))).into_response(),
    };
    let claims = match verify_token(&state.secret, &token) {
        Some(c) => c,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid or expired token"}))).into_response(),
    };
    // revocable: reject tokens whose jti has been revoked
    {
        let db = state.db.lock().unwrap();
        if is_revoked(&db, &claims.jti) {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "token has been revoked"}))).into_response();
        }
    }
    // authorize: the token must carry the publish scope for its owner
    if claims.scope != format!("publish:{}", claims.sub) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "token does not grant publish scope"}))).into_response();
    }
    // validate grammar first (malformed names are always rejected as 400, regardless of token)
    let raw_name = match payload.manifest["name"].as_str() {
        Some(n) => n,
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "manifest.name required"}))).into_response(),
    };
    let canonical = match canonical_id(raw_name) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };
    // per-owner publish scope: only the owning identity may publish under owner/*
    let pkg_owner = canonical.split('/').next().unwrap_or("");
    if pkg_owner != claims.sub {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": format!("owner '{}' cannot publish under '{pkg_owner}/*'", claims.sub)}))).into_response();
    }
    let mut db = state.db.lock().unwrap();
    if owner_revoked(&db, &claims.sub) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "owner has been revoked"}))).into_response();
    }
    // publish integrity: signature must verify against the owner's registered public key
    let signature = match payload.signature.as_deref() {
        Some(s) if !s.is_empty() => s,
        _ => return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "missing package signature"}))).into_response(),
    };
    let vk = match owner_pubkey(&db, &claims.sub).and_then(|p| pubkey_from_b64(&p)) {
        Some(v) => v,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "owner has no registered signing key"}))).into_response(),
    };
    let sig_bytes = match b64url_decode(signature) {
        Some(s) if s.len() == 64 => s,
        _ => return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "malformed signature"}))).into_response(),
    };
    let digest = package_digest_input(&payload.manifest, &payload.files);
    let sig = ed25519_dalek::Signature::from_bytes(sig_bytes.as_slice().try_into().unwrap());
    if vk.verify(&digest, &sig).is_err() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "signature does not match owner public key"}))).into_response();
    }
    match publish_db(&mut db, &payload) {
        Ok(201) => (StatusCode::CREATED, Json(serde_json::json!({"status": "published"}))).into_response(),
        Ok(409) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": "version already exists (immutable)"}))).into_response(),
        Ok(400) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid manifest or package (schema, size, or path constraints)"}))).into_response(),
        Ok(other) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("unexpected status {other}")}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

// ---------- rate limiting (fixed-window; per-IP + per-token on publish, global on reads) ----------

fn rate_limited() -> axum::response::Response {
    (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({"error": "rate limit exceeded, retry shortly"}))).into_response()
}

async fn rate_limit(State(state): State<AppState>, req: Request, next: Next) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next().map(|x| x.trim().to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    let is_publish = req.method() == Method::POST && path.ends_with("/api/publish");
    let is_read = req.method() == Method::GET && path.starts_with("/api/");

    if is_publish {
        let limits = &state.limiter.limits;
        if !state.limiter.allow(&format!("publish:ip:{ip}"), limits.publish_ip) {
            return rate_limited();
        }
        if let Some(t) = bearer_token(req.headers()) {
            if !state.limiter.allow(&format!("publish:token:{t}"), limits.publish_token) {
                return rate_limited();
            }
        }
    } else if is_read {
        let limits = &state.limiter.limits;
        if !state.limiter.allow("read:global", limits.read_global) {
            return rate_limited();
        }
    }
    next.run(req).await
}

fn build_app(state: AppState) -> Router {
    let mw_state = state.clone();
    Router::new()
        .route("/health", get(health))
        .route("/api/search", get(search))
        .route("/api/packages/{owner}/{name}", get(pkg_detail))
        .route("/api/packages/{owner}/{name}/{version}/files", get(pkg_files))
        .route("/api/owners/register", post(register_owner))
        .route("/api/owners/rotate", post(rotate_owner_key))
        .route("/api/owners/revoke", post(revoke_token))
        .route("/api/owners/revoke-owner", post(revoke_owner))
        .route("/api/publish", post(publish))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_TOTAL_SIZE + 64 * 1024)) // request body cap
        .layer(middleware::from_fn_with_state(mw_state, rate_limit))
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port = std::env::var("SKILLHUB_REGISTRY_PORT").unwrap_or_else(|_| "8787".into());
    let db_path = std::env::var("SKILLHUB_REGISTRY_DB").unwrap_or_else(|_| "data/skillhub.db".into());
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let conn = Connection::open(&db_path)?;
    init(&conn)?;
    // capability-token signing secret: from env, else a random in-process secret (dev).
    let secret = std::env::var("SKILLHUB_REGISTRY_SECRET")
        .map(|s| s.into_bytes())
        .unwrap_or_else(|_| random_hex().into_bytes());
    let state = AppState { db: Arc::new(Mutex::new(conn)), secret, limiter: RateLimiter::new(RateLimits::default()) };

    let app = build_app(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("skillhub-registry listening on http://{addr} (db: {db_path})");
    axum::serve(listener, app).await?;
    Ok(())
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as i64, d as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn mem_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        conn
    }

    fn payload(name: &str, version: &str, files: &[(&str, &str)], verified: bool) -> PublishPayload {
        PublishPayload {
            manifest: serde_json::json!({
                "name": name, "version": version, "description": "test skill",
                "license": "MIT", "repo": "https://example.com/repo",
                "harnesses": ["pi"], "permissions": ["files.read"]
            }),
            files: files.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect(),
            scan: serde_json::json!({"verified": verified, "findings": []}),
            signature: None,
        }
    }

    #[test]
    fn canonical_id_valid_and_normalized() {
        assert_eq!(canonical_id("demo/a").unwrap(), "demo/a");
        assert_eq!(canonical_id("org-1/skill-2").unwrap(), "org-1/skill-2");
        assert!(canonical_id("demo").is_err()); // no slash
        assert!(canonical_id("demo/a/b").is_err()); // too many segments
        assert!(canonical_id("/a").is_err()); // empty owner
        assert!(canonical_id("demo/").is_err()); // empty name
        assert!(canonical_id("Demo/a").is_err()); // uppercase
        assert!(canonical_id("demo/a_b").is_err()); // underscore
        assert!(canonical_id("-demo/a").is_err()); // hyphen first char
    }

    #[test]
    fn publish_rejects_invalid_name() {
        let mut conn = mem_conn();
        // malformed ids rejected before any row is written (write key-space == read key-space)
        assert_eq!(publish_db(&mut conn, &payload("no_slash", "1.0.0", &[], true)).unwrap(), 400);
        assert_eq!(publish_db(&mut conn, &payload("A/b", "1.0.0", &[], true)).unwrap(), 400);
        assert_eq!(publish_db(&mut conn, &payload("a/b/c", "1.0.0", &[], true)).unwrap(), 400);
        assert_eq!(publish_db(&mut conn, &payload("demo/under_score", "1.0.0", &[], true)).unwrap(), 400);
        assert!(search_db(&conn, "a").unwrap().is_empty());
        assert!(detail_db(&conn, "demo/under_score").unwrap().is_none());
    }

    #[test]
    fn publish_then_search() {
        let mut conn = mem_conn();
        assert_eq!(publish_db(&mut conn, &payload("demo/a", "1.0.0", &[("SKILL.md", "# hi")], true)).unwrap(), 201);
        let res = search_db(&conn, "demo").unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "demo/a");
        assert!(res[0].verified);
        assert_eq!(res[0].downloads, 0);
    }

    #[test]
    fn duplicate_version_rejected() {
        let mut conn = mem_conn();
        publish_db(&mut conn, &payload("demo/b", "1.0.0", &[], true)).unwrap();
        assert_eq!(publish_db(&mut conn, &payload("demo/b", "1.0.0", &[], false)).unwrap(), 409);
    }

    #[test]
    fn detail_and_files_roundtrip() {
        let mut conn = mem_conn();
        publish_db(&mut conn, &payload("demo/c", "2.1.0", &[("SKILL.md", "# c"), ("scripts/run.sh", "echo hi")], false)).unwrap();
        let d = detail_db(&conn, "demo/c").unwrap().unwrap();
        assert_eq!(d.versions.len(), 1);
        assert_eq!(d.versions[0].version, "2.1.0");
        assert!(!d.verified);
        let f = files_db(&conn, "demo/c", "2.1.0").unwrap().unwrap();
        assert_eq!(f["scripts/run.sh"], "echo hi");
        bump_downloads(&conn, "demo/c").unwrap();
        let d2 = detail_db(&conn, "demo/c").unwrap().unwrap();
        assert_eq!(d2.downloads, 1);
    }

    #[test]
    fn unknown_package_404() {
        let conn = mem_conn();
        assert!(detail_db(&conn, "nope/x").unwrap().is_none());
        assert!(files_db(&conn, "nope/x", "1.0.0").unwrap().is_none());
    }

    // ---- HTTP integration: prove canonical_id() is the single path for lookups + publishes ----

    fn test_secret() -> Vec<u8> {
        b"test-registry-secret-for-unit-tests-0123456789".to_vec()
    }

    fn test_state() -> AppState {
        test_state_with_limits(RateLimits::default())
    }

    fn test_state_with_limits(limits: RateLimits) -> AppState {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        AppState { db: Arc::new(Mutex::new(conn)), secret: test_secret(), limiter: RateLimiter::new(limits) }
    }

    // register an owner in the state's DB and return its signing key (mirrors CA issue)
    fn register_key(state: &AppState, owner: &str) -> ed25519_dalek::SigningKey {
        let key = owner_keypair();
        let db = state.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO owners (owner, pubkey, revoked, created_at) VALUES (?1,?2,0,?3)",
            rusqlite::params![owner, pubkey_b64(&key), now_iso()],
        )
        .unwrap();
        key
    }

    // build a publish body signed by the owner's signing key
    fn signed_body(key: &ed25519_dalek::SigningKey, manifest: &serde_json::Value, files: &HashMap<String, String>) -> String {
        let signature = sign_package(key, manifest, files);
        serde_json::json!({"manifest": manifest, "files": files, "scan": {"verified": true}, "signature": signature}).to_string()
    }

    fn demo_manifest() -> serde_json::Value {
        serde_json::json!({"name": "demo/acme-skill", "version": "1.0.0", "description": "http test skill",
            "license": "MIT", "repo": "https://example.com/repo", "harnesses": ["pi"]})
    }

    fn demo_files() -> HashMap<String, String> {
        HashMap::from([("SKILL.md".to_string(), "# http".to_string())])
    }

    fn publish_body() -> String {
        serde_json::json!({
            "manifest": {
                "name": "demo/acme-skill", "version": "1.0.0", "description": "http test skill",
                "license": "MIT", "repo": "https://example.com/repo", "harnesses": ["pi"]
            },
            "files": { "SKILL.md": "# http" },
            "scan": { "verified": true, "findings": [] }
        })
        .to_string()
    }

    async fn get(app: &Router, uri: &str) -> (StatusCode, String) {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    async fn post_json(app: &Router, uri: &str, body: &str, token: Option<&str>) -> (StatusCode, String) {
        let mut b = Request::builder().method("POST").uri(uri).header("content-type", "application/json");
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        let resp = app.clone().oneshot(b.body(Body::from(body.to_string())).unwrap()).await.unwrap();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    #[tokio::test]
    async fn http_publish_then_reads_via_canonical_id() {
        let state = test_state();
        let key = register_key(&state, "demo");
        let app = build_app(state);
        let (token, _) = mint_token(&test_secret(), "demo", "publish:demo");
        // publish valid owner/name with the owner's token + signature -> 201
        let body = signed_body(&key, &demo_manifest(), &demo_files());
        let (s, _) = post_json(&app, "/api/publish", &body, Some(&token)).await;
        assert_eq!(s, StatusCode::CREATED);
        // read back via the URL owner/name key-space -> 200 (reads stay anonymous, same canonical id)
        let (s, body) = get(&app, "/api/packages/demo/acme-skill").await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("acme-skill"));
        let (s, body) = get(&app, "/api/packages/demo/acme-skill/1.0.0/files").await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("# http"));
    }

    #[tokio::test]
    async fn http_publish_requires_valid_signature() {
        let state = test_state();
        let key = register_key(&state, "demo");
        let other = register_key(&state, "other");
        let app = build_app(state);
        let (token, _) = mint_token(&test_secret(), "demo", "publish:demo");
        // missing signature -> 403
        let body = serde_json::json!({"manifest": demo_manifest(), "files": demo_files(), "scan": {"verified": true}}).to_string();
        let (s, _) = post_json(&app, "/api/publish", &body, Some(&token)).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        // signature made by a different key (not 'demo'/'other' registered for 'demo') -> 403
        let bad = signed_body(&other, &demo_manifest(), &demo_files());
        let (s, _) = post_json(&app, "/api/publish", &bad, Some(&token)).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn http_publish_requires_auth() {
        let app = build_app(test_state());
        // no token -> 401 (unauthenticated publish rejected)
        let (s, _) = post_json(&app, "/api/publish", &publish_body(), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        // garbage token -> 401
        let (s, _) = post_json(&app, "/api/publish", &publish_body(), Some("not-a-real-token")).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        // token signed with a different secret -> 401
        let (forged, _) = mint_token(b"different-secret", "demo", "publish:demo");
        let (s, _) = post_json(&app, "/api/publish", &publish_body(), Some(&forged)).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn http_publish_forbidden_for_other_owner() {
        let app = build_app(test_state());
        // a valid token for owner 'attacker' cannot publish under demo/* -> 403
        let (token, _) = mint_token(&test_secret(), "attacker", "publish:attacker");
        let (s, body) = post_json(&app, "/api/publish", &publish_body(), Some(&token)).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        assert!(body.contains("cannot publish"));
        // nothing was written
        let (s, _) = get(&app, "/api/packages/demo/acme-skill").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn http_token_revocation_end_to_end() {
        let app = build_app(test_state());
        // register owner -> minted + recorded capability
        let (s, body) = post_json(&app, "/api/owners/register", r#"{"owner":"rev"}"#, None).await;
        assert_eq!(s, StatusCode::CREATED);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let token = v["token"].as_str().unwrap().to_string();
        let key = signing_key_from_b64(v["signing_key"].as_str().unwrap()).unwrap();
        // publish succeeds with the recorded token + valid signature
        let rev_manifest = serde_json::json!({"name": "rev/pkg", "version": "1.0.0", "description": "revocable",
            "license": "MIT", "repo": "x", "harnesses": ["pi"]});
        let rev_files = HashMap::from([("SKILL.md".to_string(), "# r".to_string())]);
        let publish_rev = signed_body(&key, &rev_manifest, &rev_files);
        let (s, _) = post_json(&app, "/api/publish", &publish_rev, Some(&token)).await;
        assert_eq!(s, StatusCode::CREATED);
        // revoke the token
        let (s, body) = post_json(&app, "/api/owners/revoke", &serde_json::json!({"token": token}).to_string(), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("revoked"));
        // publish with the revoked token is now rejected
        let (s, _) = post_json(&app, "/api/publish", &publish_rev, Some(&token)).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        // reads remain anonymous after revocation
        let (s, _) = get(&app, "/api/packages/rev/pkg").await;
        assert_eq!(s, StatusCode::OK);
    }

    #[test]
    fn rate_limiter_bucket_window_resets() {
        let limits = RateLimits { publish_ip: 2, window_secs: 60, ..RateLimits::default() };
        let lim = limits.publish_ip;
        let rl = RateLimiter::new(limits);
        assert!(rl.allow("k", lim));
        assert!(rl.allow("k", lim));
        assert!(!rl.allow("k", lim)); // over limit
        // a different key is independent
        assert!(rl.allow("other", lim));
    }

    #[tokio::test]
    async fn http_publish_rate_limited_by_ip() {
        let mut limits = RateLimits::default();
        limits.publish_ip = 2;
        let state = test_state_with_limits(limits);
        let key = register_key(&state, "demo");
        let app = build_app(state);
        let (token, _) = mint_token(&test_secret(), "demo", "publish:demo");
        // two distinct publish requests consume the per-IP bucket, third is rejected
        let req = |v: &str| {
            let manifest = serde_json::json!({"name": "demo/rl", "version": v, "description": "rl",
                "license": "MIT", "repo": "x", "harnesses": ["pi"]});
            let files = HashMap::from([("SKILL.md".to_string(), "# rl".to_string())]);
            signed_body(&key, &manifest, &files)
        };
        let (s, _) = post_json(&app, "/api/publish", &req("1.0.0"), Some(&token)).await;
        assert_eq!(s, StatusCode::CREATED);
        let (s, _) = post_json(&app, "/api/publish", &req("1.0.1"), Some(&token)).await;
        assert_eq!(s, StatusCode::CREATED);
        let (s, _) = post_json(&app, "/api/publish", &req("1.0.2"), Some(&token)).await;
        assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn http_reads_rate_limited_globally() {
        let mut limits = RateLimits::default();
        limits.read_global = 3;
        let app = build_app(test_state_with_limits(limits));
        // reads share a global bucket; after the cap, further reads are rejected
        for _ in 0..3 {
            let (s, _) = get(&app, "/api/search?q=x").await;
            assert_eq!(s, StatusCode::OK);
        }
        let (s, _) = get(&app, "/api/search?q=x").await;
        assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn http_publish_rejects_non_canonical_grammar() {
        let app = build_app(test_state());
        let (token, _) = mint_token(&test_secret(), "demo", "publish:demo");
        for bad in ["no_slash", "A/b", "a/b/c", "demo/under_score"] {
            let body = serde_json::json!({
                "manifest": {"name": bad, "version": "1.0.0", "description": "bad",
                             "license": "MIT", "repo": "x", "harnesses": ["pi"]},
                "files": {},
                "scan": {"verified": true}
            })
            .to_string();
            let (s, _) = post_json(&app, "/api/publish", &body, Some(&token)).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "expected 400 for name '{bad}'");
        }
    }

    async fn publish_json(app: &Router, key: &ed25519_dalek::SigningKey, manifest: serde_json::Value, files: serde_json::Value) -> StatusCode {
        let files_map: HashMap<String, String> = serde_json::from_value(files).unwrap();
        let body = signed_body(key, &manifest, &files_map);
        let token = mint_token(&test_secret(), "demo", "publish:demo").0;
        post_json(app, "/api/publish", &body, Some(&token)).await.0
    }

    fn base_manifest() -> serde_json::Value {
        serde_json::json!({
            "name": "demo/skill", "version": "1.0.0", "description": "ok",
            "license": "MIT", "repo": "https://example.com", "harnesses": ["pi"]
        })
    }

    #[tokio::test]
    async fn http_key_rotation_invalidates_old_key() {
        let state = test_state();
        let app = build_app(state);
        // register owner -> CA issues a signing key
        let (s, body) = post_json(&app, "/api/owners/register", r#"{"owner":"rot"}"#, None).await;
        assert_eq!(s, StatusCode::CREATED);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let token = v["token"].as_str().unwrap().to_string();
        let key1 = signing_key_from_b64(v["signing_key"].as_str().unwrap()).unwrap();
        let manifest = serde_json::json!({"name": "rot/s", "version": "1.0.0", "description": "d",
            "license": "MIT", "repo": "x", "harnesses": ["pi"]});
        let files = HashMap::from([("SKILL.md".to_string(), "# s".to_string())]);
        // publish with key1 -> 201
        let (s, _) = post_json(&app, "/api/publish", &signed_body(&key1, &manifest, &files), Some(&token)).await;
        assert_eq!(s, StatusCode::CREATED);
        // rotate -> new signing key
        let (s, body) = post_json(&app, "/api/owners/rotate", &serde_json::json!({"token": token}).to_string(), None).await;
        assert_eq!(s, StatusCode::OK);
        let key2 = signing_key_from_b64(serde_json::from_str::<serde_json::Value>(&body).unwrap()["signing_key"].as_str().unwrap()).unwrap();
        // old key no longer verifies -> 403
        let (s, _) = post_json(&app, "/api/publish", &signed_body(&key1, &manifest, &files), Some(&token)).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        // new key verifies -> 409 (dup version) means signature passed
        let (s, _) = post_json(&app, "/api/publish", &signed_body(&key2, &manifest, &files), Some(&token)).await;
        assert_eq!(s, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn http_owner_revocation_blocks_publish() {
        let state = test_state();
        let app = build_app(state);
        let (s, body) = post_json(&app, "/api/owners/register", r#"{"owner":"doomed"}"#, None).await;
        assert_eq!(s, StatusCode::CREATED);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let token = v["token"].as_str().unwrap().to_string();
        let key = signing_key_from_b64(v["signing_key"].as_str().unwrap()).unwrap();
        let manifest = serde_json::json!({"name": "doomed/s", "version": "1.0.0", "description": "d",
            "license": "MIT", "repo": "x", "harnesses": ["pi"]});
        let files = HashMap::from([("SKILL.md".to_string(), "# s".to_string())]);
        let (s, _) = post_json(&app, "/api/publish", &signed_body(&key, &manifest, &files), Some(&token)).await;
        assert_eq!(s, StatusCode::CREATED);
        // revoke the owner namespace
        let (s, _) = post_json(&app, "/api/owners/revoke-owner", &serde_json::json!({"token": token}).to_string(), None).await;
        assert_eq!(s, StatusCode::OK);
        // further publishes rejected
        let (s, _) = post_json(&app, "/api/publish", &signed_body(&key, &manifest, &files), Some(&token)).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        // reads still anonymous
        let (s, _) = get(&app, "/api/packages/doomed/s").await;
        assert_eq!(s, StatusCode::OK);
    }

    #[tokio::test]
    async fn http_publish_rejects_bad_semver() {
        let state = test_state();
        let key = register_key(&state, "demo");
        let app = build_app(state);
        for bad in ["1.0", "1.0.0.0", "v1.0.0", "1.0.a", ""] {
            let mut m = base_manifest();
            m["version"] = serde_json::json!(bad);
            let s = publish_json(&app, &key, m, serde_json::json!({})).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "version '{bad}' must be rejected");
        }
    }

    #[tokio::test]
    async fn http_publish_rejects_path_traversal() {
        let state = test_state();
        let key = register_key(&state, "demo");
        let app = build_app(state);
        for bad_path in ["../evil.sh", "/etc/passwd", "a/../b", "..", "a//b"] {
            let mut files = serde_json::json!({});
            files[bad_path] = serde_json::json!("#!/bin/sh");
            let s = publish_json(&app, &key, base_manifest(), files).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "path '{bad_path}' must be rejected");
        }
    }

    #[tokio::test]
    async fn http_publish_rejects_extra_manifest_field() {
        let state = test_state();
        let key = register_key(&state, "demo");
        let app = build_app(state);
        let mut m = base_manifest();
        m["author"] = serde_json::json!("attacker"); // additionalProperties: false
        let s = publish_json(&app, &key, m, serde_json::json!({})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn http_publish_rejects_wrong_content_type() {
        let app = build_app(test_state());
        let token = mint_token(&test_secret(), "demo", "publish:demo").0;
        // axum's Json extractor rejects non-application/json with 415
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/publish")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "text/plain")
                    .body(Body::from("hello"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[test]
    fn validate_files_enforces_size_caps() {
        let mut files = HashMap::new();
        files.insert("SKILL.md".to_string(), "x".repeat(MAX_FILE_SIZE + 1));
        assert!(validate_files(&files).is_err()); // single file over cap
        let mut many = HashMap::new();
        for i in 0..(MAX_FILES + 1) {
            many.insert(format!("f{i}.md"), "x".to_string());
        }
        assert!(validate_files(&many).is_err()); // too many files
        let mut big_total = HashMap::new();
        big_total.insert("a".to_string(), "x".repeat(MAX_TOTAL_SIZE));
        assert!(validate_files(&big_total).is_err()); // total over cap
        let ok = HashMap::from([("SKILL.md".to_string(), "hi".to_string())]);
        assert!(validate_files(&ok).is_ok());
    }

    #[tokio::test]
    async fn http_reads_reject_non_canonical_url() {
        let app = build_app(test_state());
        let (s, _) = get(&app, "/api/packages/Bad/name").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, _) = get(&app, "/api/packages/Bad/name/1.0.0/files").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // valid grammar but unknown package -> 404, not 400
        let (s, _) = get(&app, "/api/packages/valid/unknown").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }
}
