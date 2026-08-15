//! Tauri shell: wires the deskagent-core memory system to the React UI.
//! The store lives in the platform app-data dir; content is encrypted at rest when a
//! passphrase is available (DESKAGENT_PASSPHRASE env or an auto-generated 0600 keyfile).

use std::path::PathBuf;
use std::sync::Mutex;

use deskagent_core::approvals::{ApprovalCard, ApprovalDecision};
use deskagent_core::capture;
use deskagent_core::consolidation::{ConsolidationReport, Persona};
use deskagent_core::retrieval::{RetrievalQuery, RetrievalResult};
use deskagent_core::sessions::Session;
use deskagent_core::store::{MemoryEvent, MemoryKind, MemoryScope, MemoryStore, ScopeType, StoreConfig};
use tauri::{Manager, State};

pub struct AppState {
    pub store: Mutex<MemoryStore>,
}

fn keyfile_path(app_data: &PathBuf) -> PathBuf {
    app_data.join("deskagent.key")
}

/// Determine the at-rest encryption key: DESKAGENT_PASSPHRASE env, else a generated
/// keyfile (0600) in app data. Returns None to run unencrypted (documented fallback).
fn resolve_key(app_data: &PathBuf) -> Option<[u8; 32]> {
    if let Ok(pass) = std::env::var("DESKAGENT_PASSPHRASE") {
        if !pass.is_empty() {
            let salt = deskagent_core::encrypt::random_salt();
            return Some(deskagent_core::encrypt::derive_key(&pass, &salt));
        }
    }
    let keyfile = keyfile_path(app_data);
    if keyfile.exists() {
        if let Ok(hex) = std::fs::read_to_string(&keyfile) {
            let trimmed = hex.trim();
            if trimmed.len() == 64 {
                let mut key = [0u8; 32];
                for (i, b) in trimmed.as_bytes().chunks(2).enumerate() {
                    if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(b).ok()?, 16) {
                        key[i] = byte;
                    }
                }
                return Some(key);
            }
        }
    }
    // generate a fresh key and persist it with restrictive permissions
    let bytes = deskagent_core::encrypt::random_key();
    use std::io::Write;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Ok(mut f) = std::fs::File::create(&keyfile) {
        use std::os::unix::fs::PermissionsExt;
        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
        let _ = f.write_all(hex.as_bytes());
        return Some(bytes);
    }
    None
}

fn app_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn open_store(app_data: &PathBuf) -> MemoryStore {
    let db_path = app_data.join("deskagent.db");
    let config = StoreConfig {
        path: db_path.to_string_lossy().into_owned(),
        encrypt: true,
    };
    match resolve_key(app_data) {
        Some(key) => MemoryStore::open_encrypted(config, key).expect("open encrypted store"),
        None => MemoryStore::open(config).expect("open store"),
    }
}

#[tauri::command]
fn session_list(state: State<AppState>) -> Result<Vec<Session>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sessions::list_sessions(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn session_create(state: State<AppState>, project_id: Option<String>) -> Result<Session, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sessions::create_session(&store, project_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn session_append(
    state: State<AppState>,
    session_id: String,
    role: String,
    content: String,
) -> Result<Session, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let scope = scope_for_session(&store, &session_id)?;
    let session = capture::capture_turn(&store, &session_id, &role, &content, scope)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    // extraction pass every N turns (default 5), max 20 proposals per pass
    let turns = capture::turns_since_pass(&store, &session_id).map_err(|e| e.to_string())?;
    if turns as usize >= capture::DEFAULT_TURNS_PER_PASS {
        let _ = capture::run_extraction_pass(
            &store,
            &session_id,
            capture::DEFAULT_TURNS_PER_PASS,
            capture::MAX_MEMORIES_PER_PASS,
        );
    }
    Ok(session)
}

fn scope_for_session(store: &MemoryStore, session_id: &str) -> Result<MemoryScope, String> {
    let session = deskagent_core::sessions::get_session(store, session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    Ok(match session.project_id {
        Some(pid) => MemoryScope {
            scope_type: ScopeType::Project,
            project_id: Some(pid),
            project_path: None,
        },
        None => MemoryScope {
            scope_type: ScopeType::Companion,
            project_id: None,
            project_path: None,
        },
    })
}

#[tauri::command]
fn memory_list(state: State<AppState>) -> Result<Vec<MemoryEvent>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.list_memories().map_err(|e| e.to_string())
}

#[tauri::command]
fn memory_retrieve(
    state: State<AppState>,
    text: String,
    project_id: Option<String>,
    limit: Option<usize>,
) -> Result<RetrievalResult, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::retrieval::retrieve(
        &store,
        &RetrievalQuery {
            text,
            project_id,
            kinds: None,
            limit: limit.unwrap_or(8),
            budget_chars: deskagent_core::retrieval::DEFAULT_INJECTION_BUDGET_CHARS,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn memory_propose(
    state: State<AppState>,
    kind: String,
    content: String,
    scope_type: String,
    project_id: Option<String>,
) -> Result<ApprovalCard, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let kind = match kind.as_str() {
        "episodic" => MemoryKind::Episodic,
        "semantic" => MemoryKind::Semantic,
        "procedural" => MemoryKind::Procedural,
        "working" => MemoryKind::Working,
        _ => return Err("unknown memory kind".into()),
    };
    let scope = MemoryScope {
        scope_type: if scope_type == "project" { ScopeType::Project } else { ScopeType::Companion },
        project_id,
        project_path: None,
    };
    let event = deskagent_core::store::MemoryEvent {
        id: deskagent_core::store::new_id("mem"),
        kind,
        content: content.clone(),
        summary: None,
        source: deskagent_core::store::MemorySource::User,
        confidence: 0.8,
        created_at: chrono_now(),
        updated_at: None,
        episode_id: None,
        scope,
        approval: deskagent_core::store::ApprovalStatus::Pending,
        tags: None,
        embedding: None,
    };
    deskagent_core::approvals::propose(&store, &event, format!("Manual memory: {content}"))
        .map_err(|e| e.to_string())
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[tauri::command]
fn approval_list(state: State<AppState>) -> Result<Vec<ApprovalCard>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::approvals::list_cards(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn approval_decide(state: State<AppState>, card_id: String, approved: bool) -> Result<ApprovalDecision, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::approvals::decide(&store, &card_id, approved).map_err(|e| e.to_string())
}

#[tauri::command]
fn persona_get(state: State<AppState>) -> Result<Option<Persona>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::consolidation::get_persona(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn persona_regenerate(state: State<AppState>) -> Result<Persona, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::consolidation::regenerate_persona(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn memory_consolidate(state: State<AppState>) -> Result<ConsolidationReport, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let now = chrono_now();
    deskagent_core::consolidation::consolidate(&store, &now, 30.0).map_err(|e| e.to_string())
}

#[tauri::command]
fn memory_export(state: State<AppState>) -> Result<serde_json::Value, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.export_all().map_err(|e| e.to_string())
}

#[tauri::command]
fn memory_wipe(state: State<AppState>) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.wipe_all().map_err(|e| e.to_string())
}

// ---- Phase 6: runtime / skills / sandbox / conversation ----------------------

#[tauri::command]
fn runtime_list_models(
    _state: State<AppState>,
    backend: String,
    base_url: Option<String>,
) -> Result<Vec<deskagent_core::ModelInfo>, String> {
    let kind = match backend.as_str() {
        "llama.cpp" => deskagent_core::BackendKind::LlamaCpp,
        _ => deskagent_core::BackendKind::Ollama,
    };
    let reg = deskagent_core::runtime::registry::ModelRegistry::new(kind, base_url);
    reg.list().map_err(|e| e.to_string())
}

#[tauri::command]
fn runtime_pick(
    state: State<AppState>,
    backend: String,
    base_url: Option<String>,
    model: String,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let kind = if backend == "llama.cpp" { deskagent_core::BackendKind::LlamaCpp } else { deskagent_core::BackendKind::Ollama };
    let reg = deskagent_core::runtime::registry::ModelRegistry::new(kind, base_url);
    reg.remember_choice(&store, &model).map_err(|e| e.to_string())
}

#[tauri::command]
fn chat_complete(
    state: State<AppState>,
    session_id: String,
    user_turn: String,
) -> Result<deskagent_core::sessions::Session, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    // Use the remembered backend/model when configured; otherwise reply with the
    // deterministic fallback so the UI always works offline (DEC-0005).
    let (kind, model) = deskagent_core::runtime::registry::ModelRegistry::remembered_choice(&store)
        .unwrap_or((deskagent_core::BackendKind::Ollama, "unknown".to_string()));
    let base = store.meta("runtime.base_url").ok().flatten();
    let reg = deskagent_core::runtime::registry::ModelRegistry::new(kind, base);

    let ctx = deskagent_core::conversation::build_chat_context(&store, &session_id, &user_turn, None)
        .map_err(|e| e.to_string())?;
    let history = deskagent_core::sessions::get_session(&store, &session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    let msgs: Vec<deskagent_core::runtime::ChatMsg> = history
        .messages
        .iter()
        .take(deskagent_core::conversation::CONTEXT_HISTORY_KEEP)
        .map(|m| deskagent_core::runtime::ChatMsg {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let text = match reg.chat(&model, &ctx.system, &msgs) {
        Ok(gen) => gen.text,
        Err(err) => format!("[runtime offline: {err}]\n\nDeterministic fallback:\n\n{user_turn}"),
    };
    deskagent_core::conversation::attach_assistant_with_citations(&store, &session_id, &text, ctx.citations)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())
}

#[tauri::command]
fn skill_install(
    state: State<AppState>,
    registry: String,
    owner: String,
    name: String,
) -> Result<deskagent_core::InstalledSkill, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let skills_dir = deskagent_core::skills::skills_dir_from(&app_skills_base());
    deskagent_core::skills::install_skill(&store, &registry, &owner, &name, None, &skills_dir)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn skill_list(_state: State<AppState>) -> Result<Vec<deskagent_core::SkillLock>, String> {
    let skills_dir = deskagent_core::skills::skills_dir_from(&app_skills_base());
    deskagent_core::skills::installed_skills(&skills_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn skill_remove(_state: State<AppState>, owner: String, name: String) -> Result<(), String> {
    let skills_dir = deskagent_core::skills::skills_dir_from(&app_skills_base());
    deskagent_core::skills::remove_skill(&skills_dir, &owner, &name).map_err(|e| e.to_string())
}

fn app_skills_base() -> std::path::PathBuf {
    // Same app-data dir used for the DB; skills live in <appdata>/skills.
    let dirs = std::path::PathBuf::from(
        std::env::var("DESKAGENT_DATA_DIR").unwrap_or_else(|_| "./deskagent-data".to_string()),
    );
    dirs
}

#[tauri::command]
fn action_propose(
    state: State<AppState>,
    kind: String,
    description: String,
    risk: String,
    undo_description: String,
) -> Result<deskagent_core::ActionProposal, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sandbox::propose_action(&store, &kind, &description, &risk, &undo_description)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn action_decide(
    state: State<AppState>,
    id: String,
    approved: bool,
) -> Result<Option<deskagent_core::UndoEntry>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sandbox::decide_action(&store, &id, approved).map_err(|e| e.to_string())
}

#[tauri::command]
fn action_list(state: State<AppState>) -> Result<Vec<deskagent_core::ActionProposal>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sandbox::list_actions(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn undo_list(state: State<AppState>) -> Result<Vec<deskagent_core::UndoEntry>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sandbox::list_undo(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn undo_revert(state: State<AppState>, id: String) -> Result<Option<deskagent_core::UndoEntry>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    deskagent_core::sandbox::revert_undo(&store, &id).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app_data_dir(&app.handle());
            std::fs::create_dir_all(&data_dir).ok();
            let store = open_store(&data_dir);
            app.manage(AppState {
                store: Mutex::new(store),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session_list,
            session_create,
            session_append,
            memory_list,
            memory_retrieve,
            memory_propose,
            approval_list,
            approval_decide,
            persona_get,
            persona_regenerate,
            memory_consolidate,
            memory_export,
            memory_wipe,
            runtime_list_models,
            runtime_pick,
            chat_complete,
            skill_install,
            skill_list,
            skill_remove,
            action_propose,
            action_decide,
            action_list,
            undo_list,
            undo_revert
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
