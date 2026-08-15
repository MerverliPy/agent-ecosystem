// SkillHub registry — axum + SQLite. Versioned, immutable package storage with search + download counts.
// Endpoints:
//   GET  /health
//   GET  /api/search?q=<text>                          -> Vec<Summary>
//   GET  /api/packages/{name}                          -> Detail (latest verified flags + versions)
//   GET  /api/packages/{name}/{version}/files          -> {files: {path: content}} (increments download count)
//   POST /api/publish                                  -> 201 | 409 | 400
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
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
struct AppState {
    db: Db,
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
    let full = format!("{owner}/{name}");
    let db = state.db.lock().unwrap();
    match detail_db(&db, &full) {
        Ok(Some(d)) => (StatusCode::OK, Json(d)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn pkg_files(State(state): State<AppState>, Path((owner, name, version)): Path<(String, String, String)>) -> impl IntoResponse {
    let full = format!("{owner}/{name}");
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
    let name = m["name"].as_str().ok_or_else(|| anyhow::anyhow!("manifest.name required"))?.to_string();
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

async fn publish(State(state): State<AppState>, Json(payload): Json<PublishPayload>) -> impl IntoResponse {
    let mut db = state.db.lock().unwrap();
    match publish_db(&mut db, &payload) {
        Ok(201) => (StatusCode::CREATED, Json(serde_json::json!({"status": "published"}))).into_response(),
        Ok(409) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": "version already exists (immutable)"}))).into_response(),
        Ok(400) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid manifest: name/version/description required"}))).into_response(),
        Ok(other) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("unexpected status {other}")}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

// ---------- main ----------

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
    let state = AppState { db: Arc::new(Mutex::new(conn)) };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/search", get(search))
        .route("/api/packages/{owner}/{name}", get(pkg_detail))
        .route("/api/packages/{owner}/{name}/{version}/files", get(pkg_files))
        .route("/api/publish", post(publish))
        .with_state(state);

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
}
