mod harness;
mod lockfile;
mod manifest;
mod registry;
mod scan;

use clap::{Parser, Subcommand};
use ed25519_dalek::Signer;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn b64url(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).ok()
}

/// Canonical byte string signed by the publisher — must match the registry's package_digest_input.
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

fn sign_package(signing_key_b64: &str, manifest: &serde_json::Value, files: &HashMap<String, String>) -> anyhow::Result<String> {
    let bytes = b64url_decode(signing_key_b64).ok_or_else(|| anyhow::anyhow!("invalid base64url signing key"))?;
    let arr: [u8; 32] = bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("signing key must be 32 bytes"))?;
    let key = ed25519_dalek::SigningKey::from_bytes(&arr);
    let digest = package_digest_input(manifest, files);
    Ok(b64url(&key.sign(&digest).to_bytes()))
}

#[derive(Parser)]
#[command(name = "skillhub", version, about = "SkillHub — cross-harness skill package manager")]
struct Cli {
    /// Registry base URL
    #[arg(long, global = true, default_value = "http://127.0.0.1:8787")]
    registry: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search the registry
    Search { query: String },
    /// Show package info and versions
    Info { name: String },
    /// Install a skill into a harness skills directory
    Install {
        name: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        harness: Option<String>,
        #[arg(long)]
        dir: Option<String>,
    },
    /// Update an installed skill to the latest version
    Update {
        name: String,
        #[arg(long)]
        dir: Option<String>,
    },
    /// Remove an installed skill
    Remove {
        name: String,
        #[arg(long)]
        dir: Option<String>,
    },
    /// Download a package and scan it (or scan a local directory with --dir)
    Verify {
        name: Option<String>,
        #[arg(long)]
        dir: Option<String>,
        /// Also run the SlopGate quality scanner (node + apps/slopgate required)
        #[arg(long)]
        quality: bool,
    },
    /// Scan a local skill directory with the security scanner
    Scan { dir: String },
    /// List detected harnesses on this machine
    Harnesses,
    /// Register an owner and mint its first publish capability token
    Register {
        /// Owner namespace to register (lowercase, matches [a-z0-9][a-z0-9-]*)
        owner: String,
    },
    /// Revoke a capability token (pass the token string as an argument)
    Revoke {
        /// The token to revoke
        token: String,
    },
    /// Roll over the owner's signing key (present a valid capability token)
    Rotate {
        /// Publish capability token (else $SKILLHUB_TOKEN)
        #[arg(long)]
        token: Option<String>,
    },
    /// Publish a local skill directory to the registry (runs the scanner first)
    Publish {
        /// Path to skillhub.json
        manifest: String,
        /// Directory containing the skill files
        #[arg(long)]
        files_dir: String,
        /// Publish capability token (else $SKILLHUB_TOKEN)
        #[arg(long)]
        token: Option<String>,
        /// Ed25519 signing key (base64url) to sign the package (else $SKILLHUB_SIGNING_KEY)
        #[arg(long)]
        signing_key: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = registry::Client::new(&cli.registry);
    match &cli.command {
        Command::Search { query } => cmd_search(&client, query),
        Command::Info { name } => cmd_info(&client, name),
        Command::Install { name, version, harness, dir } => cmd_install(&client, name, version.as_deref(), harness.as_deref(), dir.as_deref()),
        Command::Update { name, dir } => cmd_update(&client, name, dir.as_deref()),
        Command::Remove { name, dir } => cmd_remove(name, dir.as_deref()),
        Command::Verify { name, dir, quality } => cmd_verify(&client, name.as_deref(), dir.as_deref(), *quality),
        Command::Scan { dir } => cmd_scan(Path::new(dir)),
        Command::Harnesses => cmd_harnesses(),
        Command::Register { owner } => cmd_register(&client, owner),
        Command::Revoke { token } => cmd_revoke(&client, token),
        Command::Rotate { token } => cmd_rotate(&client, token.as_deref()),
        Command::Publish { manifest, files_dir, token, signing_key } => cmd_publish(&client, manifest, Path::new(files_dir), token.as_deref(), signing_key.as_deref()),
    }
}

fn cmd_search(client: &registry::Client, q: &str) -> anyhow::Result<()> {
    let results = client.search(q)?;
    if results.is_empty() {
        println!("no packages match '{}'", q);
        return Ok(());
    }
    println!("{:<40} {:>8} {:>10}  {}", "name", "downloads", "verified", "description");
    for p in results {
        let v = if p.verified { "VERIFIED" } else { "unverified" };
        let hr = if p.high_risk { " [high-risk]" } else { "" };
        println!("{:<40} {:>8} {:>10}  {}{}", p.name, p.downloads, v, p.description, hr);
    }
    Ok(())
}

fn cmd_info(client: &registry::Client, name: &str) -> anyhow::Result<()> {
    let d = client.info(name)?;
    println!("name:        {}", d.name);
    println!("description: {}", d.description);
    println!("license:     {}", d.license);
    println!("repo:        {}", d.repo);
    println!("verified:    {}", if d.verified { "YES" } else { "no" });
    println!("high-risk:   {}", if d.high_risk { "yes" } else { "no" });
    println!("downloads:   {}", d.downloads);
    println!("versions:");
    for v in &d.versions {
        println!("  {}  ({} harnesses, published {})", v.version, v.harnesses.len(), v.published_at);
    }
    Ok(())
}

/// Reject path traversal in registry-supplied relative file paths.
fn safe_rel(path: &str) -> anyhow::Result<PathBuf> {
    let p = Path::new(path);
    anyhow::ensure!(!p.is_absolute(), "absolute path not allowed: {}", path);
    anyhow::ensure!(
        !p.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))),
        "unsafe path: {}",
        path
    );
    Ok(p.to_path_buf())
}

fn write_files(target: &Path, files: &std::collections::HashMap<String, String>) -> anyhow::Result<()> {
    fs::create_dir_all(target)?;
    let mut paths: Vec<&String> = files.keys().collect();
    paths.sort();
    for p in paths {
        let rel = safe_rel(p)?;
        let dest = target.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, &files[p])?;
    }
    Ok(())
}

fn cmd_install(
    client: &registry::Client,
    name: &str,
    version: Option<&str>,
    harness: Option<&str>,
    dir: Option<&str>,
) -> anyhow::Result<()> {
    let info = client.info(name)?;
    let resolved = match version {
        Some(v) => v.to_string(),
        None => info
            .versions
            .iter()
            .map(|v| v.version.clone())
            .max_by(|a, b| manifest::cmp_versions(a, b))
            .ok_or_else(|| anyhow::anyhow!("no versions published for {}", name))?,
    };
    anyhow::ensure!(
        info.versions.iter().any(|v| v.version == resolved),
        "version {} not found for {}",
        resolved,
        name
    );

    let dest = harness::resolve(harness, dir)?;
    let files = client.files(name, &resolved)?;
    let target = dest.join(name);
    write_files(&target, &files.files)?;
    let sum = scan::checksum(&target)?;
    let report = scan::scan_skill(&target)?;
    if !report.findings.is_empty() {
        println!("WARNING: scanner found {} issue(s):", report.findings.len());
        for f in report.findings.iter().take(10) {
            println!("  [{}] {} {}:{} {}", f.severity.to_uppercase(), f.rule, f.file, f.line, f.snippet);
        }
    }

    let entry = lockfile::LockEntry {
        name: name.to_string(),
        version: resolved.clone(),
        source: client.base.clone(),
        harness: harness.unwrap_or("auto").to_string(),
        installed_at: now_iso(),
        checksum: sum,
    };
    lockfile::upsert(&dest, entry)?;

    println!(
        "installed {} v{} -> {} (harness dir: {})",
        name,
        resolved,
        target.display(),
        dest.display()
    );
    println!("lockfile: {}", lockfile::lock_path(&dest).display());
    Ok(())
}

fn cmd_update(client: &registry::Client, name: &str, dir: Option<&str>) -> anyhow::Result<()> {
    let dest = harness::resolve(None, dir)?;
    let current = lockfile::find(&dest, name).ok_or_else(|| anyhow::anyhow!("{} is not installed (no lockfile entry)", name))?;
    let info = client.info(name)?;
    let latest = info
        .versions
        .iter()
        .map(|v| v.version.clone())
        .max_by(|a, b| manifest::cmp_versions(a, b))
        .ok_or_else(|| anyhow::anyhow!("no versions published for {}", name))?;
    if manifest::cmp_versions(&current.version, &latest) != std::cmp::Ordering::Less {
        println!("{} is already at the latest version ({})", name, current.version);
        return Ok(());
    }
    let files = client.files(name, &latest)?;
    let target = dest.join(name);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    write_files(&target, &files.files)?;
    let entry = lockfile::LockEntry {
        name: name.to_string(),
        version: latest.clone(),
        source: client.base.clone(),
        harness: current.harness,
        installed_at: now_iso(),
        checksum: scan::checksum(&target)?,
    };
    lockfile::upsert(&dest, entry)?;
    println!("updated {} {} -> {}", name, current.version, latest);
    Ok(())
}

fn cmd_remove(name: &str, dir: Option<&str>) -> anyhow::Result<()> {
    let dest = harness::resolve(None, dir)?;
    let removed = lockfile::remove(&dest, name)?;
    let target = dest.join(name);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    if removed {
        println!("removed {} from {}", name, dest.display());
    } else if !target.exists() {
        println!("{} not installed in {}", name, dest.display());
    }
    Ok(())
}

fn cmd_verify(client: &registry::Client, name: Option<&str>, dir: Option<&str>, quality: bool) -> anyhow::Result<()> {
    let tmp = std::env::temp_dir().join(format!("skillhub-verify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let target;
    if let Some(n) = name {
        let info = client.info(n)?;
        let latest = info
            .versions
            .iter()
            .map(|v| v.version.clone())
            .max_by(|a, b| manifest::cmp_versions(a, b))
            .ok_or_else(|| anyhow::anyhow!("no versions published for {}", n))?;
        let files = client.files(n, &latest)?;
        write_files(&tmp, &files.files)?;
        target = tmp;
        let report = scan::scan_skill(&target)?;
        print_report(n, &report);
        if quality {
            print_quality(&target, run_quality_check(&target));
        }
        if !report.verified {
            std::process::exit(1);
        }
        Ok(())
    } else if let Some(d) = dir {
        target = std::path::PathBuf::from(d);
        let report = scan::scan_skill(&target)?;
        print_report(d, &report);
        if quality {
            print_quality(&target, run_quality_check(&target));
        }
        if !report.verified {
            std::process::exit(1);
        }
        Ok(())
    } else {
        anyhow::bail!("verify requires a package name or --dir")
    }
}

/// SlopGate quality check: shells out to the slopgate CLI (node type-stripping).
/// Returns None when the scanner is unavailable (no node, no repo checkout).
fn run_quality_check(dir: &std::path::Path) -> Option<serde_json::Value> {
    let cli = std::env::var("SKILLHUB_SLOPGATE_CLI").ok().map(std::path::PathBuf::from).or_else(|| {
        // repo-root relative: works when run from the ecosystem checkout
        let guess = std::path::Path::new("apps/slopgate/src/cli.ts");
        guess.exists().then(|| guess.to_path_buf())
    })?;
    let out = std::process::Command::new("node")
        .arg("--experimental-strip-types")
        .arg(&cli)
        .arg("score")
        .arg(dir)
        .arg("--json")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

fn print_quality(dir: &std::path::Path, report: Option<serde_json::Value>) {
    match report {
        Some(r) => {
            // `slop score --json` emits score as a number and totalFindings as a count.
            let score = r["score"].as_f64().or_else(|| {
                r["score"].as_object().and_then(|o| o.get("score")).and_then(|v| v.as_f64())
            }).unwrap_or(0.0);
            let findings = r["totalFindings"]
                .as_i64()
                .or_else(|| r["findings"].as_array().map(|a| a.len() as i64))
                .unwrap_or(0);
            println!("QUALITY: {} — slop score {:.0}/100 ({} finding(s))", dir.display(), score, findings);
        }
        None => println!("QUALITY: skipped — slopgate scanner unavailable (SKILLHUB_SLOPGATE_CLI or repo checkout + node required)"),
    }
}

fn print_report(target: &str, report: &scan::ScanReport) {
    if report.findings.is_empty() {
        println!("SCAN-CLEAN: {} (verified)", target);
        return;
    }
    println!("SCAN-FAIL: {} — {} finding(s):", target, report.findings.len());
    for f in &report.findings {
        println!(
            "  [{}] {} {}:{}  {}",
            f.severity.to_uppercase(),
            f.rule,
            f.file,
            f.line,
            f.snippet
        );
    }
}

fn cmd_scan(dir: &Path) -> anyhow::Result<()> {
    let report = scan::scan_skill(dir)?;
    print_report(&dir.display().to_string(), &report);
    if !report.verified {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_harnesses() -> anyhow::Result<()> {
    let det = harness::detected();
    if det.is_empty() {
        println!("no harness skills directories detected");
        return Ok(());
    }
    for h in det {
        println!("{}  {}", h.id, h.dir.display());
    }
    Ok(())
}

fn cmd_revoke(client: &registry::Client, token: &str) -> anyhow::Result<()> {
    match client.revoke_token(token)? {
        200 => println!("token revoked"),
        401 => println!("ERROR: token invalid or expired"),
        other => println!("ERROR: registry returned HTTP {}", other),
    }
    Ok(())
}

fn cmd_register(client: &registry::Client, owner: &str) -> anyhow::Result<()> {
    let v = client.register_owner(owner)?;
    let registered = v["owner"].as_str().unwrap_or(owner);
    let token = v["token"].as_str().unwrap_or_default();
    let signing_key = v["signing_key"].as_str().unwrap_or_default();
    println!("owner registered: {registered}");
    println!("export SKILLHUB_TOKEN={token}");
    println!("export SKILLHUB_SIGNING_KEY={signing_key}");
    println!("  (keep both secret: token grants publish scope, signing key authenticates your packages for {registered}/*)");
    Ok(())
}

fn cmd_rotate(client: &registry::Client, token: Option<&str>) -> anyhow::Result<()> {
    let token = token.map(|t| t.to_string()).or_else(|| std::env::var("SKILLHUB_TOKEN").ok())
        .ok_or_else(|| anyhow::anyhow!("rotate needs a capability token (--token or $SKILLHUB_TOKEN)"))?;
    let v = client.rotate_key(&token)?;
    let signing_key = v["signing_key"].as_str().unwrap_or_default();
    println!("owner {} signing key rotated", v["owner"].as_str().unwrap_or(""));
    println!("export SKILLHUB_SIGNING_KEY={signing_key}");
    Ok(())
}

fn cmd_publish(client: &registry::Client, manifest_path: &str, files_dir: &Path, token: Option<&str>, signing_key: Option<&str>) -> anyhow::Result<()> {
    let raw = fs::read_to_string(manifest_path)?;
    let m = manifest::Manifest::from_json(&raw)?;
    anyhow::ensure!(files_dir.is_dir(), "--files-dir must be a directory");

    // collect files (relative paths), excluding the manifest itself
    let mut files = std::collections::HashMap::new();
    for entry in walkdir::WalkDir::new(files_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(files_dir).unwrap().to_string_lossy().replace('\\', "/");
        if rel == "skillhub.json" {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let content = String::from_utf8_lossy(&bytes).to_string();
        files.insert(rel, content);
    }

    // run the scanner on the publish directory
    let report = scan::scan_skill(files_dir)?;
    if !report.verified {
        println!("WARNING: scanner found high-severity issues — package will be published unverified:");
        for f in report.findings.iter().filter(|f| f.severity == "high").take(10) {
            println!("  [{}] {} {}:{} {}", f.severity.to_uppercase(), f.rule, f.file, f.line, f.snippet);
        }
    }

    let manifest_value = serde_json::to_value(&m)?;
    // signing key: --signing-key flag, else $SKILLHUB_SIGNING_KEY
    let signing_key = signing_key.map(|k| k.to_string()).or_else(|| std::env::var("SKILLHUB_SIGNING_KEY").ok());
    let signature = match signing_key.as_deref() {
        Some(k) if !k.is_empty() => Some(sign_package(k, &manifest_value, &files)?),
        _ => None,
    };
    let mut payload = serde_json::json!({
        "manifest": manifest_value,
        "files": files,
        "scan": serde_json::to_value(&report)?,
    });
    if let Some(sig) = &signature {
        payload["signature"] = serde_json::json!(sig);
    }
    // publish token: --token flag, else $SKILLHUB_TOKEN
    let token = token.map(|t| t.to_string()).or_else(|| std::env::var("SKILLHUB_TOKEN").ok());
    if signature.is_none() {
        println!("NOTE: no signing key set (--signing-key or $SKILLHUB_SIGNING_KEY) — registry will reject unsigned publishes");
    }
    let status = client.publish(&payload, token.as_deref())?;
    match status {
        201 => println!("published {} v{} (verified: {})", m.name, m.version, report.verified),
        401 => println!("ERROR: publish requires a capability token — run `skillhub register {}` or set $SKILLHUB_TOKEN", m.name.split('/').next().unwrap_or("")),
        403 => println!("ERROR: your token cannot publish under {}/* (per-owner scope)", m.name.split('/').next().unwrap_or("")),
        409 => println!("ERROR: version {} of {} already exists (versions are immutable)", m.version, m.name),
        other => println!("ERROR: registry returned HTTP {}", other),
    }
    Ok(())
}

fn now_iso() -> String {
    // Simple UTC ISO8601 without chrono.
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    // Howard Hinnant's algorithm
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
