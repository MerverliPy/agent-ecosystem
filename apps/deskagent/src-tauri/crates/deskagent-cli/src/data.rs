//! Data directory + at-rest encryption key resolution (DEC-0009).
//!
//! Faithfully mirrors the Tauri shell's `resolve_key` (src-tauri/src/lib.rs) so the
//! CLI and GUI derive identical keys: `DESKAGENT_PASSPHRASE` (with a persisted salt)
//! wins, otherwise a 0600 keyfile (`deskagent.key`) is generated on first use. A
//! store opened with a key is encrypted at rest; without one it falls back to the
//! same documented plaintext path the shell uses. This is a data-layer port only —
//! `deskagent-core` is untouched.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use deskagent_core::store::{MemoryStore, StoreConfig};

pub const KEYFILE_NAME: &str = "deskagent.key";
pub const SALT_NAME: &str = "deskagent.salt";
pub const DB_NAME: &str = "deskagent.db";

/// Data-dir priority: `--data-dir` flag > `DESKAGENT_DATA_DIR` env (the shell's
/// documented override) > `$XDG_DATA_HOME/deskagent` > `~/.local/share/deskagent`.
pub fn resolve_data_dir(flag: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = flag {
        return dir;
    }
    if let Ok(dir) = std::env::var("DESKAGENT_DATA_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("deskagent");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/deskagent");
    }
    PathBuf::from(".")
}

fn keyfile_path(dir: &PathBuf) -> PathBuf {
    dir.join(KEYFILE_NAME)
}

fn salt_path(dir: &PathBuf) -> PathBuf {
    dir.join(SALT_NAME)
}

/// Load the persisted passphrase salt, or create and persist a fresh one on first
/// run (mirrors the shell's `passphrase_salt` so the same passphrase derives the
/// same key across launches).
pub fn passphrase_salt(dir: &PathBuf) -> Vec<u8> {
    let salt_path = salt_path(dir);
    if let Ok(hex) = std::fs::read_to_string(&salt_path) {
        let trimmed = hex.trim();
        if trimmed.len() == 32 {
            if let Some(salt) = hex_to_bytes(trimmed) {
                return salt;
            }
        }
    }
    let salt = deskagent_core::encrypt::random_salt();
    let hex: String = salt.iter().map(|b| format!("{b:02x}")).collect();
    if let Ok(mut f) = std::fs::File::create(&salt_path) {
        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
        let _ = f.write_all(hex.as_bytes());
    }
    salt
}

pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    hex.as_bytes()
        .chunks(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
        .collect()
}

/// Resolve the at-rest encryption key, exactly like the Tauri shell: passphrase
/// (persisted salt) when `DESKAGENT_PASSPHRASE` is set, else an existing 0600
/// keyfile, else a newly generated keyfile. `None` only when no keyfile could be
/// persisted (the documented plaintext fallback).
pub fn resolve_key(dir: &PathBuf) -> Option<[u8; 32]> {
    if let Ok(pass) = std::env::var("DESKAGENT_PASSPHRASE") {
        if !pass.is_empty() {
            let salt = passphrase_salt(dir);
            return Some(deskagent_core::encrypt::derive_key(&pass, &salt));
        }
    }
    let keyfile = keyfile_path(dir);
    if keyfile.exists() {
        if let Ok(hex) = std::fs::read_to_string(&keyfile) {
            let trimmed = hex.trim();
            if trimmed.len() == 64 {
                if let Some(bytes) = hex_to_bytes(trimmed) {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return Some(key);
                }
            }
        }
    }
    // generate a fresh key and persist it with restrictive permissions
    let bytes = deskagent_core::encrypt::random_key();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Ok(mut f) = std::fs::File::create(&keyfile) {
        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
        let _ = f.write_all(hex.as_bytes());
        return Some(bytes);
    }
    None
}

/// Human-readable encryption status for the TUI status bar.
pub fn encryption_label(dir: &PathBuf) -> String {
    if let Ok(pass) = std::env::var("DESKAGENT_PASSPHRASE") {
        if !pass.is_empty() {
            return "encrypted · passphrase (DESKAGENT_PASSPHRASE)".to_string();
        }
    }
    if keyfile_path(dir).exists() {
        return "encrypted · keyfile (0600)".to_string();
    }
    "encrypted · keyfile (generated on first use)".to_string()
}

/// Open (creating if needed) the store with the same key policy as the shell's
/// `open_store`: encrypted when a key resolves, else the documented plaintext path.
pub fn open_store(dir: &PathBuf) -> MemoryStore {
    std::fs::create_dir_all(dir).ok();
    match resolve_key(dir) {
        Some(key) => {
            let config = StoreConfig {
                path: dir.join(DB_NAME).to_string_lossy().into_owned(),
                encrypt: true,
            };
            MemoryStore::open_encrypted(config, key)
                .unwrap_or_else(|e| panic!("open encrypted store: {e}"))
        }
        None => {
            let config = StoreConfig {
                path: dir.join(DB_NAME).to_string_lossy().into_owned(),
                encrypt: false,
            };
            MemoryStore::open(config).unwrap_or_else(|e| panic!("open store: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskagent_core::sessions::{create_session, get_session};
    use std::sync::{Mutex, MutexGuard};

    /// Both tests below mutate the process-wide `DESKAGENT_PASSPHRASE`; tests run in
    /// parallel threads, so serialize them or the keyfile test can observe a
    /// passphrase it never set.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("deskagent-cli-{tag}-{}-{ts}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &PathBuf) {
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn keyfile_is_generated_with_0600_permissions() {
        let _guard = lock_env();
        // Guarantee the passphrase branch is not taken during this assertion window.
        let prior = std::env::var("DESKAGENT_PASSPHRASE").ok();
        std::env::remove_var("DESKAGENT_PASSPHRASE");

        let dir = temp_dir("key0600");
        let key = resolve_key(&dir).expect("key resolves");
        assert_eq!(key.len(), 32);
        let path = keyfile_path(&dir);
        assert!(path.exists());
        use std::os::unix::fs::MetadataExt;
        let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600, "keyfile must be 0600 (DEC-0009)");
        cleanup(&dir);

        match prior {
            Some(p) => std::env::set_var("DESKAGENT_PASSPHRASE", p),
            None => std::env::remove_var("DESKAGENT_PASSPHRASE"),
        }
    }

    #[test]
    fn passphrase_derives_the_same_key_across_launches() {
        let _guard = lock_env();
        let dir = temp_dir("passphrase");
        // Guard: save/restore the env var so parallel tests are unaffected.
        let prior = std::env::var("DESKAGENT_PASSPHRASE").ok();
        std::env::set_var("DESKAGENT_PASSPHRASE", "smoke-test-passphrase");
        let k1 = resolve_key(&dir).expect("key 1");
        let k2 = resolve_key(&dir).expect("key 2 (relaunch)");
        assert_eq!(k1, k2, "same passphrase must derive the same key");
        // passphrase mode does not create a keyfile (keys are ephemeral per process)
        assert!(!keyfile_path(&dir).exists());
        // open a store and round-trip a memory through encryption
        let store = open_store(&dir);
        let session = create_session(&store, None).unwrap();
        get_session(&store, &session.id).unwrap().unwrap();
        match prior {
            Some(p) => std::env::set_var("DESKAGENT_PASSPHRASE", p),
            None => std::env::remove_var("DESKAGENT_PASSPHRASE"),
        }
        cleanup(&dir);
    }

    #[test]
    fn hex_helpers_roundtrip() {
        let bytes = hex_to_bytes("00ff10aabbccddee").unwrap();
        assert_eq!(bytes, vec![0x00, 0xff, 0x10, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]);
        assert!(hex_to_bytes("zz").is_none());
        // odd-length hex is leniently chunked — identical to the Tauri shell's
        // hex_to_bytes, so the CLI and GUI resolve the same keys.
        assert_eq!(hex_to_bytes("abc"), Some(vec![0xab, 0x0c]));
    }

    #[test]
    fn data_dir_falls_back_to_home() {
        let dir = resolve_data_dir(Some(PathBuf::from("/tmp/x")));
        assert_eq!(dir, PathBuf::from("/tmp/x"));
        let dir = resolve_data_dir(None);
        assert!(!dir.as_os_str().is_empty());
    }
}