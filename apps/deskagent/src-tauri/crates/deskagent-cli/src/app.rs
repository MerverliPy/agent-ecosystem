//! TUI application state: four tabs mirroring the web (Chat / Memory+Approvals /
//! Models / Tasks), key handling, and the inline Y/n approval-card flow (Task 5).
//! Rendering lives in [`crate::ui`] as a pure `draw(f, &App)` so tests drive it
//! through a `ratatui::backend::TestBackend`.

use std::io;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use deskagent_core::approvals::{decide, list_cards, ApprovalCard};
use deskagent_core::consolidation::{get_persona, Persona};
use deskagent_core::runtime::registry::{BackendKind, ModelRegistry};
use deskagent_core::runtime::ModelInfo;
use deskagent_core::sessions::{create_session, list_sessions, Session};
use deskagent_core::store::{MemoryEvent, MemoryStore};

use crate::chat;
use crate::data;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    Memory,
    Models,
    Tasks,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Chat, Tab::Memory, Tab::Models, Tab::Tasks];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Chat => "Chat",
            Tab::Memory => "Memory+Approvals",
            Tab::Models => "Models",
            Tab::Tasks => "Tasks",
        }
    }

    pub fn next(self) -> Tab {
        match self {
            Tab::Chat => Tab::Memory,
            Tab::Memory => Tab::Models,
            Tab::Models => Tab::Tasks,
            Tab::Tasks => Tab::Chat,
        }
    }

    pub fn prev(self) -> Tab {
        match self {
            Tab::Chat => Tab::Tasks,
            Tab::Memory => Tab::Chat,
            Tab::Models => Tab::Memory,
            Tab::Tasks => Tab::Models,
        }
    }
}

/// One rendered chat line: role + text + how many memories the assistant turn cited.
#[derive(Debug, Clone)]
pub struct ChatLine {
    pub role: String,
    pub text: String,
    pub citations: usize,
}

pub struct App {
    pub store: MemoryStore,
    pub dir: PathBuf,
    pub tab: Tab,
    pub sessions: Vec<Session>,
    pub active_id: Option<String>,
    pub chat_lines: Vec<ChatLine>,
    pub input: String,
    pub input_cursor: usize,
    pub chat_scroll: usize,
    pub auto_scroll: bool,
    pub memory_scroll: usize,
    pub memories: Vec<MemoryEvent>,
    pub approvals: Vec<ApprovalCard>,
    pub pending: Vec<ApprovalCard>,
    pub focus_approval: usize,
    pub persona: Option<Persona>,
    pub models: Vec<ModelInfo>,
    pub model_sel: usize,
    pub remembered: Option<(BackendKind, String)>,
    pub backend_override: Option<BackendKind>,
    pub base_url: Option<String>,
    pub encryption: String,
    pub status: String,
    pub quit: bool,
}

pub fn run_tui(
    store: MemoryStore,
    backend_override: Option<BackendKind>,
    base_url: Option<String>,
    dir: PathBuf,
) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new(store, dir, backend_override, base_url);
    app.refresh();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

impl App {
    pub fn new(
        store: MemoryStore,
        dir: PathBuf,
        backend_override: Option<BackendKind>,
        base_url: Option<String>,
    ) -> Self {
        let encryption = data::encryption_label(&dir);
        Self {
            store,
            dir,
            tab: Tab::Chat,
            sessions: vec![],
            active_id: None,
            chat_lines: vec![],
            input: String::new(),
            input_cursor: 0,
            chat_scroll: usize::MAX,
            auto_scroll: true,
            memory_scroll: 0,
            memories: vec![],
            approvals: vec![],
            pending: vec![],
            focus_approval: 0,
            persona: None,
            models: vec![],
            model_sel: 0,
            remembered: None,
            backend_override,
            base_url,
            encryption,
            status: "loading…".to_string(),
            quit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|f| crate::ui::draw(f, self))?;
            if self.quit {
                break;
            }
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                    self.on_key(key)?;
                }
                crossterm::event::Event::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    }

    // ---- key handling -------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) -> io::Result<()> {
        match key.code {
            KeyCode::Tab => self.tab = self.tab.next(),
            KeyCode::BackTab => self.tab = self.tab.prev(),
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('q') if self.tab != Tab::Chat => self.quit = true,
            _ => match self.tab {
                Tab::Chat => self.on_chat_key(key),
                Tab::Memory => self.on_memory_key(key),
                Tab::Models => self.on_models_key(key),
                Tab::Tasks => {}
            },
        }
        Ok(())
    }

    fn on_chat_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.chat_submit(),
            KeyCode::Backspace => self.edit_backspace(),
            KeyCode::Left => self.edit_left(),
            KeyCode::Right => self.edit_right(),
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input.len(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => self.edit_insert(c),
            _ => {}
        }
    }

    fn on_memory_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') => self.approve_focused(),
            KeyCode::Char('n') => self.reject_focused(),
            KeyCode::Char('k') => self.scroll_memories(-1),
            KeyCode::Char('j') => self.scroll_memories(1),
            KeyCode::Char('e') => self.export_memories(),
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
    }

    fn on_models_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('k') => self.model_sel = self.model_sel.saturating_sub(1),
            KeyCode::Char('j') => self.model_sel = (self.model_sel + 1).min(self.models.len().saturating_sub(1)),
            KeyCode::Enter => self.pick_model(),
            KeyCode::Char('m') => self.pick_model(),
            KeyCode::Char('r') => self.reload_models(),
            _ => {}
        }
    }

    // ---- chat input editing ------------------------------------------------

    fn edit_insert(&mut self, c: char) {
        if c == '\n' || c == '\r' {
            return;
        }
        self.input.insert(self.input_cursor, c);
        self.input_cursor += c.len_utf8();
    }

    fn edit_backspace(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let before = self.input[..self.input_cursor].chars().next_back();
        if let Some(c) = before {
            let start = self.input_cursor - c.len_utf8();
            self.input.remove(start);
            self.input_cursor = start;
        }
    }

    fn edit_left(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let before = self.input[..self.input_cursor].chars().next_back();
        if let Some(c) = before {
            self.input_cursor -= c.len_utf8();
        }
    }

    fn edit_right(&mut self) {
        let after = self.input[self.input_cursor..].chars().next();
        if let Some(c) = after {
            self.input_cursor += c.len_utf8();
        }
    }

    // ---- actions ------------------------------------------------------------

    /// Submit the chat input: capture + extract + complete (offline fallback per
    /// DEC-0005), then reload every pane. Pending extraction proposals appear in the
    /// Memory+Approvals tab for the inline Y/n flow.
    pub fn chat_submit(&mut self) {
        let content = self.input.trim().to_string();
        if content.is_empty() {
            return;
        }
        self.input.clear();
        self.input_cursor = 0;
        let Some(session_id) = self.active_id.clone() else {
            self.status = "no active session".to_string();
            return;
        };
        match chat::chat_turn(
            &self.store,
            &session_id,
            &content,
            self.backend_override,
            self.base_url.clone(),
            None,
        ) {
            Ok(t) => {
                self.status = if t.offline {
                    format!("offline fallback · {}", t.model)
                } else {
                    format!("{} · {} citations · {} proposal(s)", t.model, t.citations, t.extraction_proposals)
                };
            }
            Err(e) => self.status = format!("error: {e}"),
        }
        self.refresh();
    }

    pub fn approve_focused(&mut self) {
        let Some(card) = self.pending.get(self.focus_approval).cloned() else {
            self.status = "no pending approval card".to_string();
            return;
        };
        match decide(&self.store, &card.id, true) {
            Ok(d) => self.status = format!("approved {} (+{:.1} confidence)", short_id(&card.id), d.confidence_delta),
            Err(e) => self.status = format!("approve: {e}"),
        }
        self.refresh();
    }

    pub fn reject_focused(&mut self) {
        let Some(card) = self.pending.get(self.focus_approval).cloned() else {
            self.status = "no pending approval card".to_string();
            return;
        };
        match decide(&self.store, &card.id, false) {
            Ok(d) => self.status = format!("rejected {} ({:.1} confidence)", short_id(&card.id), d.confidence_delta),
            Err(e) => self.status = format!("reject: {e}"),
        }
        self.refresh();
    }

    pub fn scroll_memories(&mut self, delta: i32) {
        let max = self.memories.len().saturating_sub(1) as i32;
        let next = self.memory_scroll as i32 + delta;
        self.memory_scroll = next.clamp(0, max.max(0)) as usize;
    }

    pub fn export_memories(&mut self) {
        match self.store.export_all() {
            Ok(json) => {
                let path = self.dir.join(format!(
                    "deskagent-export-{}.json",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                ));
                match std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".into())) {
                    Ok(()) => {
                        let count = json.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
                        self.status = format!("exported {count} memories → {}", path.display());
                    }
                    Err(e) => self.status = format!("export write: {e}"),
                }
            }
            Err(e) => self.status = format!("export: {e}"),
        }
    }

    pub fn reload_models(&mut self) {
        let kind = self
            .backend_override
            .or_else(|| self.remembered.as_ref().map(|(k, _)| *k))
            .unwrap_or(BackendKind::Ollama);
        let base = self
            .base_url
            .clone()
            .or_else(|| self.store.meta("runtime.base_url").ok().flatten());
        let reg = ModelRegistry::new(kind, base);
        match reg.list() {
            Ok(models) => {
                self.models = models;
                self.model_sel = 0;
                self.status = format!("{} reachable · {} model(s)", kind.as_str(), self.models.len());
            }
            Err(e) => {
                self.models.clear();
                self.status = format!("list models: {e}");
            }
        }
    }

    pub fn pick_model(&mut self) {
        let Some(info) = self.models.get(self.model_sel).cloned() else {
            self.status = "no models loaded — press r".to_string();
            return;
        };
        let kind = self.backend_override.unwrap_or(BackendKind::Ollama);
        let reg = ModelRegistry::new(kind, self.base_url.clone());
        match reg.remember_choice(&self.store, &info.name) {
            Ok(()) => {
                self.status = format!("remembered {} / {}", kind.as_str(), info.name);
            }
            Err(e) => self.status = format!("remember: {e}"),
        }
        self.refresh();
    }

    // ---- state loading ------------------------------------------------------

    pub fn refresh(&mut self) {
        match list_sessions(&self.store) {
            Ok(sessions) => {
                self.sessions = sessions;
                if self.active_id.is_none() {
                    if let Some(first) = self.sessions.first() {
                        self.active_id = Some(first.id.clone());
                    } else {
                        // fresh data dir: start a conversation
                        match create_session(&self.store, None) {
                            Ok(sess) => {
                                self.sessions.insert(0, sess.clone());
                                self.active_id = Some(sess.id);
                            }
                            Err(e) => self.status = format!("create session: {e}"),
                        }
                    }
                }
                self.rebuild_chat();
            }
            Err(e) => self.status = format!("sessions: {e}"),
        }
        self.memories = self.store.list_memories().unwrap_or_default();
        self.approvals = list_cards(&self.store).unwrap_or_default();
        self.pending = self
            .approvals
            .iter()
            .filter(|c| c.status == "pending")
            .cloned()
            .collect();
        // keep the newest pending card in focus; clamp on refresh
        let pending_len = self.pending.len();
        if pending_len > 0 && self.focus_approval > pending_len - 1 {
            self.focus_approval = pending_len - 1;
        }
        self.persona = get_persona(&self.store).ok().flatten();
        self.remembered = ModelRegistry::remembered_choice(&self.store);
        if self.auto_scroll {
            self.chat_scroll = usize::MAX;
        }
    }

    fn rebuild_chat(&mut self) {
        self.chat_lines.clear();
        let Some(id) = &self.active_id else {
            return;
        };
        if let Some(session) = self.sessions.iter().find(|s| &s.id == id) {
            for m in &session.messages {
                self.chat_lines.push(ChatLine {
                    role: m.role.clone(),
                    text: m.content.clone(),
                    citations: m.citations.as_ref().map(|c| c.len()).unwrap_or(0),
                });
            }
        }
    }

    pub fn approval_status_counts(&self) -> (usize, usize) {
        (
            self.pending.len(),
            self.approvals
                .iter()
                .filter(|c| c.status == "approved")
                .count(),
        )
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat;
    use deskagent_core::store::StoreConfig;

    fn app() -> App {
        let store = MemoryStore::open(StoreConfig {
            path: ":memory:".into(),
            encrypt: false,
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("deskagent-cli-app-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = App::new(store, dir, None, Some("http://127.0.0.1:1".into()));
        a.refresh();
        a
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn fresh_app_creates_a_session_in_chat_tab() {
        let app = app();
        assert_eq!(app.tab, Tab::Chat);
        assert!(app.active_id.is_some());
        assert_eq!(app.sessions.len(), 1);
        assert!(app.encryption.contains("encrypted"));
    }

    #[test]
    fn tabs_cycle_forward_and_back() {
        let mut app = app();
        assert_eq!(app.tab.next(), Tab::Memory);
        assert_eq!(app.tab.next().next().next().next(), Tab::Chat);
        assert_eq!(app.tab.prev(), Tab::Tasks);
        app.on_key(key(KeyCode::Tab)).unwrap();
        assert_eq!(app.tab, Tab::Memory);
        app.on_key(key(KeyCode::BackTab)).unwrap();
        assert_eq!(app.tab, Tab::Chat);
    }

    #[test]
    fn chat_submit_offline_appends_fallback_and_enqueues_proposals_eventually() {
        let mut app = app();
        app.input = "I prefer Rust for CLIs.".to_string();
        app.input_cursor = app.input.len();
        app.chat_submit();
        assert!(app.chat_lines.len() >= 2, "user + assistant lines");
        assert_eq!(app.chat_lines[0].role, "user");
        assert_eq!(app.chat_lines[1].role, "assistant");
        assert!(app.chat_lines[1].text.contains("Deterministic fallback"));
        assert!(app.status.contains("offline"));
        // input cleared
        assert!(app.input.is_empty());
    }

    #[test]
    fn input_editing_moves_and_deletes_by_char() {
        let mut app = app();
        app.edit_insert('h');
        app.edit_insert('i');
        assert_eq!(app.input, "hi");
        app.edit_backspace();
        assert_eq!(app.input, "h");
        // multi-byte char handling
        app.input_cursor = 0;
        app.input.clear();
        app.edit_insert('日');
        app.edit_insert('a');
        app.edit_left();
        app.edit_backspace();
        assert_eq!(app.input, "a");
    }

    #[test]
    fn inline_approval_flow_decides_the_focused_card() {
        let mut app = app();
        // seed a pending card by running 5 preference turns (extraction fires)
        let session_id = app.active_id.clone().unwrap();
        for t in [
            "I prefer TypeScript for new services.",
            "Please remember: my favorite editor is Neovim.",
            "I always run cargo test before pushing.",
            "To deploy staging, run `bash scripts/deploy.sh staging`.",
            "I like dark mode and coffee.",
        ] {
            chat::chat_turn(&app.store, &session_id, t, None, Some("http://127.0.0.1:1".into()), None).unwrap();
        }
        app.refresh();
        assert!(!app.pending.is_empty(), "pending cards expected after 5 turns");
        let before = app.pending.len();
        app.focus_approval = 0;
        app.approve_focused();
        assert_eq!(app.pending.len(), before - 1, "approved card leaves the pending list");
        assert!(app.status.contains("approved"));
        // decided card history grows
        let (pending, decided) = app.approval_status_counts();
        assert_eq!(pending, before - 1);
        assert!(decided >= 1);
    }

    #[test]
    fn reject_focused_records_negative_signal() {
        let mut app = app();
        let session_id = app.active_id.clone().unwrap();
        chat::chat_turn(&app.store, &session_id, "Please remember: user dislikes pineapple.", None, Some("http://127.0.0.1:1".into()), None).unwrap();
        for _ in 0..4 {
            chat::chat_turn(&app.store, &session_id, "I like plain water.", None, Some("http://127.0.0.1:1".into()), None).unwrap();
        }
        app.refresh();
        assert!(!app.pending.is_empty());
        let card_id = app.pending[0].id.clone();
        app.reject_focused();
        assert!(!app.pending.iter().any(|c| c.id == card_id));
        // verify via approvals history (card ids ≠ memory ids)
        let cards = list_cards(&app.store).unwrap();
        let decided_card = cards.iter().find(|c| c.id == card_id).unwrap();
        assert_eq!(decided_card.status, "rejected");
        assert!(app.status.contains("rejected"));
    }

    #[test]
    fn pick_model_remembers_choice() {
        let mut app = app();
        app.models = vec![ModelInfo {
            name: "qwen2.5:7b".into(),
            size_bytes: Some(4_700_000_000),
            family: None,
            parameter_size: Some("7B".into()),
        }];
        app.model_sel = 0;
        app.pick_model();
        let (kind, model) = ModelRegistry::remembered_choice(&app.store).unwrap();
        assert_eq!(kind, BackendKind::Ollama);
        assert_eq!(model, "qwen2.5:7b");
        assert!(app.status.contains("remembered"));
    }

    #[test]
    fn export_memories_writes_json_file() {
        let mut app = app();
        app.input = "I prefer Rust for CLIs.".to_string();
        app.input_cursor = app.input.len();
        app.chat_submit();
        app.export_memories();
        assert!(app.status.starts_with("exported"));
        assert!(app.status.contains("deskagent-export-"));
        let path = app
            .status
            .split("→ ")
            .nth(1)
            .map(PathBuf::from)
            .expect("report includes the path");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("memories"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn quit_keys_exit_only_outside_chat_input() {
        let mut app = app();
        app.on_key(key(KeyCode::Char('q'))).unwrap();
        assert!(!app.quit, "typing q in chat is input, not quit");
        app.tab = Tab::Memory;
        app.on_key(key(KeyCode::Char('q'))).unwrap();
        assert!(app.quit, "q outside the chat tab quits");
    }

    #[test]
    fn memory_scroll_clamps() {
        let mut app = app();
        app.memories = vec![
            MemoryEvent {
                id: "m1".into(),
                kind: deskagent_core::store::MemoryKind::Semantic,
                content: "a".into(),
                summary: None,
                source: deskagent_core::store::MemorySource::User,
                confidence: 0.9,
                created_at: "t".into(),
                updated_at: None,
                episode_id: None,
                scope: deskagent_core::store::MemoryScope {
                    scope_type: deskagent_core::store::ScopeType::Companion,
                    project_id: None,
                    project_path: None,
                },
                approval: deskagent_core::store::ApprovalStatus::Approved,
                tags: None,
                embedding: None,
            },
            MemoryEvent {
                id: "m2".into(),
                kind: deskagent_core::store::MemoryKind::Semantic,
                content: "b".into(),
                summary: None,
                source: deskagent_core::store::MemorySource::User,
                confidence: 0.9,
                created_at: "t".into(),
                updated_at: None,
                episode_id: None,
                scope: deskagent_core::store::MemoryScope {
                    scope_type: deskagent_core::store::ScopeType::Companion,
                    project_id: None,
                    project_path: None,
                },
                approval: deskagent_core::store::ApprovalStatus::Approved,
                tags: None,
                embedding: None,
            },
        ];
        app.scroll_memories(99);
        assert_eq!(app.memory_scroll, 1);
        app.scroll_memories(-99);
        assert_eq!(app.memory_scroll, 0);
    }
}