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

    if name.is_empty() || version.is_empty() || description.is_empty() {
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
    let db = state.db.lock().unwrap();
    if let Err(e) = db.execute(
        "INSERT OR IGNORE INTO owners (owner, created_at) VALUES (?1, ?2)",
        rusqlite::params![payload.owner, now_iso()],
    ) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }
    let (token, claims) = mint_token(&state.secret, &payload.owner, &format!("publish:{}", payload.owner));
    if let Err(e) = record_capability(&db, &claims) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }
    (StatusCode::CREATED, Json(serde_json::json!({"owner": payload.owner, "token": token}))).into_response()
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
    match publish_db(&mut db, &payload) {
        Ok(201) => (StatusCode::CREATED, Json(serde_json::json!({"status": "published"}))).into_response(),
        Ok(409) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": "version already exists (immutable)"}))).into_response(),
        Ok(400) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid manifest: name (owner/name grammar), version, description required"}))).into_response(),
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
        .route("/api/owners/revoke", post(revoke_token))
        .route("/api/publish", post(publish))
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
        let app = build_app(test_state());
        let (token, _) = mint_token(&test_secret(), "demo", "publish:demo");
        // publish valid owner/name with the owner's token -> 201
        let (s, _) = post_json(&app, "/api/publish", &publish_body(), Some(&token)).await;
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
        let token = serde_json::from_str::<serde_json::Value>(&body).unwrap()["token"]
            .as_str().unwrap().to_string();
        // publish succeeds with the recorded token
        let publish_rev = serde_json::json!({
            "manifest": {"name": "rev/pkg", "version": "1.0.0", "description": "revocable",
                         "license": "MIT", "repo": "x", "harnesses": ["pi"]},
            "files": {"SKILL.md": "# r"},
            "scan": {"verified": true}
        }).to_string();
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
        let app = build_app(test_state_with_limits(limits));
        let (token, _) = mint_token(&test_secret(), "demo", "publish:demo");
        // two distinct publish requests consume the per-IP bucket, third is rejected
        let req = |v: &str| {
            let body2 = serde_json::json!({
                "manifest": {"name": "demo/rl", "version": v, "description": "rl",
                             "license": "MIT", "repo": "x", "harnesses": ["pi"]},
                "files": {"SKILL.md": "# rl"},
                "scan": {"verified": true}
            }).to_string();
            body2
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
