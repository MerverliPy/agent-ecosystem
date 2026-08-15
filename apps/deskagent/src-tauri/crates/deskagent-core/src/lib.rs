//! DeskAgent self-memory core (DEC-0009): local-first, user-owned memory.
//!
//! Modules:
//! - [`store`]    SQLite store (rusqlite bundled): episodic/semantic/procedural/working.
//! - [`encrypt`]  AES-256-GCM encryption at rest, PBKDF2 key derivation.
//! - [`embed`]    Local embeddings: deterministic HashEmbedder (default) or fastembed-rs (feature).
//! - [`capture`]  Capture pipeline: raw episodes + extraction pass distilling facts/preferences.
//! - [`consolidation`] Persona regeneration, dedupe, conflict detection, decay.
//! - [`retrieval`] Hybrid keyword + embedding retrieval with RRF fusion and a strict injection budget.
//! - [`approvals`] Propose-to-remember approval cards (every distilled write is gated).
//! - [`sessions`] Session persistence shared with the chat UI.

pub mod approvals;
pub mod capture;
pub mod consolidation;
pub mod conversation;
pub mod embed;
pub mod encrypt;
pub mod retrieval;
pub mod runtime;
pub mod sandbox;
pub mod sessions;
pub mod skills;
pub mod store;

pub use approvals::{ApprovalCard, ApprovalDecision};
pub use runtime::{Backend, Generation, ModelInfo, RuntimeError};
pub use runtime::registry::BackendKind;
pub use sandbox::{ActionProposal, UndoEntry};
pub use skills::{InstalledSkill, SkillLock, SkillManifest};
pub use store::{MemoryEvent, MemoryKind, MemoryScope, MemorySource, MemoryStore, ScopeType};

pub const VERSION: &str = "0.1.0";
