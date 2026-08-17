//! deskagent-cli — terminal UI (ratatui) + headless commands over the
//! deskagent-core memory system (DEC-0009). The four-pane TUI mirrors the web tabs;
//! every command reuses the core exactly as the Tauri shell does. `deskagent-core`
//! is untouched — this crate is pure CLI wiring.

mod app;
mod chat;
mod data;
mod ui;

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use deskagent_core::runtime::registry::BackendKind;
use deskagent_core::store::MemoryStore;

#[derive(Parser)]
#[command(
    name = "deskagent",
    version,
    about = "DeskAgent CLI — local-first personal agent with self-memory (DEC-0009). Terminal UI (default) plus headless chat, models, approvals, memory, persona, export, and wipe."
)]
struct Cli {
    /// Data directory (default: $DESKAGENT_DATA_DIR, else ~/.local/share/deskagent).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Interactive four-pane terminal UI (Chat / Memory+Approvals / Models / Tasks).
    Tui {
        /// Backend: ollama | llama.cpp (default: remembered choice, else ollama).
        #[arg(long)]
        backend: Option<String>,
        /// Backend base URL override.
        #[arg(long)]
        base_url: Option<String>,
    },
    /// One-shot chat: capture the turn, complete it (runtime or deterministic
    /// fallback, DEC-0005), print the reply with citations.
    Chat {
        /// The message to send.
        message: String,
        /// Reuse a session id (a new one is created when absent).
        #[arg(long)]
        session: Option<String>,
        /// Backend: ollama | llama.cpp (default: remembered choice, else ollama).
        #[arg(long)]
        backend: Option<String>,
        /// Backend base URL override (also remembered for later turns).
        #[arg(long)]
        base_url: Option<String>,
        /// Model override; also remembered as the default choice.
        #[arg(long)]
        model: Option<String>,
    },
    /// List models from the backend; --pick remembers one as the default choice.
    Models {
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        pick: Option<String>,
    },
    /// List approval cards (pending + decided history).
    Approvals {},
    /// Approve a pending approval card (the TUI's inline Y/n in headless form).
    Approve {
        /// Card id (accepts the 16-char short prefix shown in the TUI).
        id: String,
    },
    /// Reject a pending approval card.
    Reject {
        /// Card id (accepts the 16-char short prefix shown in the TUI).
        id: String,
    },
    /// List memories (all by default; --approved only approved ones).
    Memory {
        #[arg(long)]
        approved: bool,
        /// Filter by kind: episodic | semantic | procedural | working.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Print the persona card (display only; regeneration is automatic).
    Persona {},
    /// Export every memory as JSON (DEC-0009) to stdout or --out <file>.
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Delete every memory, approval, and transcript (DEC-0009). Requires --yes.
    Wipe {
        #[arg(long)]
        yes: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let data_dir = data::resolve_data_dir(cli.data_dir.clone());
    let code = match cli.command {
        None => cmd_default(&data_dir),
        Some(Command::Tui { backend, base_url }) => cmd_tui(&data_dir, backend, base_url),
        Some(Command::Chat { message, session, backend, base_url, model }) => {
            cmd_chat(&data_dir, &message, session, backend, base_url, model)
        }
        Some(Command::Models { backend, base_url, pick }) => cmd_models(&data_dir, backend, base_url, pick),
        Some(Command::Approvals {}) => cmd_approvals(&data_dir),
        Some(Command::Approve { id }) => cmd_decide(&data_dir, &id, true),
        Some(Command::Reject { id }) => cmd_decide(&data_dir, &id, false),
        Some(Command::Memory { approved, kind }) => cmd_memory(&data_dir, approved, kind),
        Some(Command::Persona {}) => cmd_persona(&data_dir),
        Some(Command::Export { out }) => cmd_export(&data_dir, out),
        Some(Command::Wipe { yes }) => cmd_wipe(&data_dir, yes),
    };
    std::process::exit(code);
}

/// Parse `--backend`: `None` (no flag) means "use the remembered/default backend",
/// while an unknown value is an error.
fn parse_backend(s: Option<String>) -> Result<Option<BackendKind>, String> {
    match s.as_deref() {
        Some("llama.cpp") | Some("llama-cpp") => Ok(Some(BackendKind::LlamaCpp)),
        Some("ollama") => Ok(Some(BackendKind::Ollama)),
        Some(other) => Err(format!("unknown backend \"{other}\" (expected ollama | llama.cpp)")),
        None => Ok(None),
    }
}

fn cmd_default(data_dir: &PathBuf) -> i32 {
    if !std::io::stdout().is_terminal() {
        eprintln!(
            "deskagent: interactive TUI needs a terminal.\n\
             Use a subcommand instead, e.g. `deskagent chat \"hello\"` (see `deskagent --help`)."
        );
        return 2;
    }
    cmd_tui(data_dir, None, None)
}

fn cmd_tui(data_dir: &PathBuf, backend: Option<String>, base_url: Option<String>) -> i32 {
    let kind = match parse_backend(backend) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("deskagent: {e}");
            return 1;
        }
    };
    let store = data::open_store(data_dir);
    match app::run_tui(store, kind, base_url, data_dir.clone()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("deskagent: TUI error: {e}");
            1
        }
    }
}

fn cmd_chat(
    data_dir: &PathBuf,
    message: &str,
    session: Option<String>,
    backend: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
) -> i32 {
    let store = data::open_store(data_dir);
    let session_id = match session {
        Some(id) => id,
        None => match deskagent_core::sessions::create_session(&store, None) {
            Ok(s) => s.id,
            Err(e) => {
                eprintln!("deskagent: failed to create session: {e}");
                return 1;
            }
        },
    };
    let kind = match parse_backend(backend) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("deskagent: {e}");
            return 1;
        }
    };
    match chat::chat_turn(&store, &session_id, message, kind, base_url, model) {
        Ok(t) => {
            println!("session: {}", t.session_id);
            println!("model:   {} ({})", t.model, if t.offline { "offline fallback" } else { "runtime" });
            println!("citations: {}", t.citations);
            println!("context: {} chars{}", t.used_chars, if t.truncated { " (truncated)" } else { "" });
            println!();
            println!("{}", t.reply);
            0
        }
        Err(e) => {
            eprintln!("deskagent: {e}");
            1
        }
    }
}

fn cmd_models(
    data_dir: &PathBuf,
    backend: Option<String>,
    base_url: Option<String>,
    pick: Option<String>,
) -> i32 {
    let store = data::open_store(data_dir);
    let kind = match parse_backend(backend) {
        Ok(Some(k)) => k,
        Ok(None) => deskagent_core::runtime::registry::ModelRegistry::remembered_choice(&store)
            .map(|(k, _)| k)
            .unwrap_or(BackendKind::Ollama),
        Err(e) => {
            eprintln!("deskagent: {e}");
            return 1;
        }
    };
    let base = base_url.or_else(|| store.meta("runtime.base_url").ok().flatten());
    let reg = deskagent_core::runtime::registry::ModelRegistry::new(kind, base);

    if let Some(name) = pick {
        match reg.remember_choice(&store, &name) {
            Ok(()) => println!("remembered {} / {name}", kind.as_str()),
            Err(e) => {
                eprintln!("deskagent: remember: {e}");
                return 1;
            }
        }
        return 0;
    }

    match reg.list() {
        Ok(models) => {
            if models.is_empty() {
                println!("{}: no models", kind.as_str());
            }
            for m in &models {
                let detail = m
                    .parameter_size
                    .clone()
                    .or_else(|| m.size_bytes.map(|b| format!("{:.1} GB", b as f64 / 1e9)))
                    .unwrap_or_default();
                println!("{}\t{}", m.name, detail);
            }
            if let Some((_, remembered)) =
                deskagent_core::runtime::registry::ModelRegistry::remembered_choice(&store)
            {
                println!("\nremembered: {remembered}");
            }
            0
        }
        Err(e) => {
            eprintln!("deskagent: list models: {e}");
            1
        }
    }
}

fn cmd_approvals(data_dir: &PathBuf) -> i32 {
    let store = data::open_store(data_dir);
    match deskagent_core::approvals::list_cards(&store) {
        Ok(cards) => {
            if cards.is_empty() {
                println!("no approval cards");
            }
            for c in &cards {
                let event_summary = c
                    .event
                    .as_ref()
                    .map(|e| format!(" · {:.1} conf · {}", e.kind.as_str(), e.content.chars().take(60).collect::<String>()))
                    .unwrap_or_default();
                println!("{}  [{}] {}  {}", short_id(&c.id), c.status, c.description, event_summary);
            }
            let pending = cards.iter().filter(|c| c.status == "pending").count();
            println!("\n{pending} pending — resolve with `deskagent approve <id>` / `deskagent reject <id>`");
            0
        }
        Err(e) => {
            eprintln!("deskagent: approvals: {e}");
            1
        }
    }
}

fn cmd_decide(data_dir: &PathBuf, id: &str, approved: bool) -> i32 {
    let store = data::open_store(data_dir);
    let full = match find_card(&store, id) {
        Some(c) => c,
        None => {
            eprintln!("deskagent: no approval card matching \"{id}\" (see `deskagent approvals`)");
            return 1;
        }
    };
    match deskagent_core::approvals::decide(&store, &full, approved) {
        Ok(d) => {
            println!(
                "{} {} ({:+.1} confidence)",
                if approved { "approved" } else { "rejected" },
                short_id(&d.card_id),
                d.confidence_delta
            );
            0
        }
        Err(e) => {
            eprintln!("deskagent: decide: {e}");
            1
        }
    }
}

fn cmd_memory(data_dir: &PathBuf, approved: bool, kind: Option<String>) -> i32 {
    let store = data::open_store(data_dir);
    let memories = match store.list_memories() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("deskagent: list memories: {e}");
            return 1;
        }
    };
    let filter = |m: &deskagent_core::store::MemoryEvent| -> bool {
        if approved && m.approval != deskagent_core::store::ApprovalStatus::Approved {
            return false;
        }
        if let Some(k) = &kind {
            if m.kind.as_str() != k {
                return false;
            }
        }
        true
    };
    let shown: Vec<_> = memories.iter().filter(|m| filter(m)).collect();
    if shown.is_empty() {
        println!("no memories match");
    }
    for m in &shown {
        println!(
            "{}  [{} · {} · {}] conf {:.2} · {}",
            short_id(&m.id),
            m.kind.as_str(),
            m.approval.as_str(),
            m.scope.scope_type.as_str(),
            m.confidence,
            m.content
        );
    }
    println!("\n{} of {} memories shown", shown.len(), memories.len());
    0
}

fn cmd_persona(data_dir: &PathBuf) -> i32 {
    let store = data::open_store(data_dir);
    match deskagent_core::consolidation::get_persona(&store) {
        Ok(Some(p)) => {
            println!("DeskAgent persona v{} (generated {})", p.version, p.generated_at);
            println!("memories: {}", p.memories_count);
            println!();
            println!("summary: {}", p.summary);
            if !p.preferences.is_empty() {
                println!("\npreferences:");
                for pref in &p.preferences {
                    println!("  - {pref}");
                }
            }
            if !p.facts.is_empty() {
                println!("\nfacts:");
                for f in &p.facts {
                    println!("  - {f}");
                }
            }
            if !p.skills.is_empty() {
                println!("\nskills:");
                for s in &p.skills {
                    println!("  - {s}");
                }
            }
            0
        }
        Ok(None) => {
            println!("no persona yet — it regenerates automatically after approved memories accumulate");
            0
        }
        Err(e) => {
            eprintln!("deskagent: persona: {e}");
            1
        }
    }
}

fn cmd_export(data_dir: &PathBuf, out: Option<PathBuf>) -> i32 {
    let store = data::open_store(data_dir);
    match store.export_all() {
        Ok(json) => {
            let pretty = serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string());
            match out {
                Some(path) => match std::fs::write(&path, &pretty) {
                    Ok(()) => {
                        let count = json.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
                        println!("exported {count} memories → {}", path.display());
                        0
                    }
                    Err(e) => {
                        eprintln!("deskagent: export write: {e}");
                        1
                    }
                },
                None => {
                    println!("{pretty}");
                    0
                }
            }
        }
        Err(e) => {
            eprintln!("deskagent: export: {e}");
            1
        }
    }
}

fn cmd_wipe(data_dir: &PathBuf, yes: bool) -> i32 {
    if !yes {
        eprintln!("deskagent: wipe deletes EVERYTHING (memories, approvals, transcript, undo log). Pass --yes to confirm.");
        return 2;
    }
    let store = data::open_store(data_dir);
    match store.wipe_all() {
        Ok(()) => {
            println!("all memory data deleted ({}).", data_dir.join(data::DB_NAME).display());
            println!("The data dir itself and the encryption key are kept; remove the dir to delete those too.");
            0
        }
        Err(e) => {
            eprintln!("deskagent: wipe: {e}");
            1
        }
    }
}

fn find_card(store: &MemoryStore, id: &str) -> Option<String> {
    let cards = deskagent_core::approvals::list_cards(store).ok()?;
    cards
        .iter()
        .find(|c| c.id == id || c.id.starts_with(id) || short_id(&c.id) == id)
        .map(|c| c.id.clone())
}

fn short_id(id: &str) -> String {
    id.chars().take(16).collect()
}
