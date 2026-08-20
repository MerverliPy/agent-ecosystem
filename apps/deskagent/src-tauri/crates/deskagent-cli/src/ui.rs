//! Pure ratatui rendering for the four-pane TUI. `draw` is a pure function of
//! `&App` (no store access beyond the already-loaded state) so tests render each tab
//! through a `TestBackend` deterministically.

use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{App, Tab};

/// Minimum usable terminal size; below this a guard screen is shown instead of
/// garbled panes. iPhone portrait via Moshi is ~40 columns, so this is generous.
pub const MIN_COLS: u16 = 40;
pub const MIN_ROWS: u16 = 12;

/// Compact (narrow) layout kicks in below this width — targets iPhone portrait.
pub fn compact(width: u16) -> bool {
    width < 100
}

pub fn draw(f: &mut Frame, app: &App) {
    let [tabs_area, body, status_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)]).areas(f.area());
    let width = f.area().width;
    if width < MIN_COLS || f.area().height < MIN_ROWS {
        draw_too_small(f, app, f.area());
        return;
    }
    draw_tabs(f, app, tabs_area, width);
    match app.tab {
        Tab::Chat => draw_chat(f, app, body, width),
        Tab::Memory => draw_memory(f, app, body, width),
        Tab::Models => draw_models(f, app, body, width),
        Tab::Tasks => draw_tasks(f, app, body),
    }
    draw_status(f, app, status_area, width);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect, width: u16) {
    let narrow = compact(width);
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| {
            let title = t.title();
            let label = match t {
                Tab::Memory if narrow => format!(" Mem ({}) ", app.pending.len()),
                Tab::Memory => format!(" {title} ({}) ", app.pending.len()),
                _ => format!(" {title} "),
            };
            Line::from(label)
        })
        .collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().title(" DeskAgent ").borders(Borders::ALL))
        .select(app.tab as usize)
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

/// Guard screen for terminals too small to lay out the four panes.
fn draw_too_small(f: &mut Frame, _app: &App, area: Rect) {
    let msg = format!(
        "Terminal too small — need at least {MIN_COLS}×{MIN_ROWS} (widen or rotate your phone)."
    );
    let lines = vec![
        Line::from(Span::styled(msg, Style::default().fg(Color::Yellow))),
        Line::from(""),
        Line::from(Span::styled(
            "deskagent · Esc: quit",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect, width: u16) {
    let block = Block::default().title(chat_title(app, width)).borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [msgs_area, input_area] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    let mut lines: Vec<Line> = Vec::new();
    for m in &app.chat_lines {
        let (label, style) = match m.role.as_str() {
            "user" => ("you", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            "assistant" => ("agent", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            other => (other, Style::default()),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{label} · "), style),
            Span::raw(&m.text),
        ]));
        if m.role == "assistant" && m.citations > 0 {
            let n = m.citations;
            let cite = if compact(width) {
                format!("  ↳ {n} cited")
            } else {
                format!("  ↳ recalled {n} memory/memories")
            };
            lines.push(Line::from(Span::styled(cite, Style::default().fg(Color::DarkGray))));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(empty conversation — type below and press Enter)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let visible = msgs_area.height as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    // chat_scroll = lines scrolled up from the bottom (0 = pinned to latest).
    let top = max_scroll.saturating_sub(app.chat_scroll.min(max_scroll));
    let chat = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((top as u16, 0));
    f.render_widget(chat, msgs_area);

    let mut prompt = Line::from(vec![
        Span::raw("> "),
        Span::styled(&app.input, Style::default().fg(Color::Yellow)),
    ]);
    if app.chat_scroll > 0 {
        prompt.spans.push(Span::styled(
            "  [scrolled — scroll down]",
            Style::default().fg(Color::DarkGray),
        ));
    }
    f.render_widget(Paragraph::new(prompt), input_area);

    if app.tab == Tab::Chat {
        let prefix_chars = 2 + app.input[..app.input_cursor.min(app.input.len())].chars().count();
        let x = inner.x + prefix_chars.min(inner.width.saturating_sub(1) as usize) as u16;
        let y = inner.y + inner.height.saturating_sub(1);
        f.set_cursor_position(Position::new(x, y));
    }
}

fn chat_title(app: &App, width: u16) -> String {
    let session = app
        .active_id
        .as_ref()
        .and_then(|id| app.sessions.iter().find(|s| &s.id == id));
    let title = session.map(|s| s.title.clone()).unwrap_or_else(|| "no session".to_string());
    let title = truncate(&title, if compact(width) { 18 } else { 60 });
    format!(" Chat — {title} ")
}

fn draw_memory(f: &mut Frame, app: &App, area: Rect, width: u16) {
    let block = Block::default().title(" Memory+Approvals ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [persona_area, approvals_area, memories_area] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Percentage(35),
        Constraint::Percentage(65),
    ])
    .areas(inner);

    // ---- persona ----
    let persona_lines: Vec<Line> = match &app.persona {
        Some(p) => vec![
            Line::from(Span::styled(
                format!("summary (v{}): ", p.version),
                Style::default().add_modifier(Modifier::BOLD),
            )
            .to_owned()),
            Line::from(Span::raw(truncate(&p.summary, 200))),
            Line::from(Span::styled(
                format!(
                    "preferences {} · facts {} · skills {} · generated {}",
                    p.preferences.len(),
                    p.facts.len(),
                    p.skills.len(),
                    &p.generated_at[..p.generated_at.len().min(16)]
                ),
                Style::default().fg(Color::DarkGray),
            )),
        ],
        None => vec![Line::from(Span::styled(
            "No persona yet — it regenerates automatically after approved memories accumulate (50).",
            Style::default().fg(Color::DarkGray),
        ))],
    };
    f.render_widget(
        Paragraph::new(persona_lines).block(Block::default().borders(Borders::ALL).title(" Persona ")),
        persona_area,
    );

    // ---- approvals (inline Y/n) ----
    let items: Vec<ListItem> = app
        .pending
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let marker = if i == app.focus_approval { "▶ y/n  " } else { "      " };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(format!("[{}] ", short(&c.id)), Style::default().fg(Color::DarkGray)),
                Span::raw(truncate(&c.description, if compact(width) { 80 } else { 200 })),
            ]))
        })
        .collect();
    let (pending, decided) = app.approval_status_counts();
    let approvals_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Approvals — {pending} pending · {decided} decided (y/n on the ▶ card) ")),
    );
    f.render_widget(approvals_list, approvals_area);

    // ---- memories ----
    let mem_lines: Vec<Line> = app
        .memories
        .iter()
        .map(|m| {
            let status_color = match m.approval {
                deskagent_core::store::ApprovalStatus::Approved => Color::Green,
                deskagent_core::store::ApprovalStatus::Rejected => Color::Red,
                deskagent_core::store::ApprovalStatus::Pending => Color::Yellow,
            };
            Line::from(vec![
                Span::styled(format!("[{}", m.kind.as_str()), Style::default().fg(Color::Magenta)),
                Span::styled(format!(" {}", m.approval.as_str()), Style::default().fg(status_color)),
                Span::styled(
                    format!("] conf {:.2} · ", m.confidence),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(truncate(&m.content, if compact(width) { 80 } else { 220 })),
            ])
        })
        .collect();
    let visible = memories_area.height as usize;
    let max_scroll = mem_lines.len().saturating_sub(visible);
    let scroll = app.memory_scroll.min(max_scroll) as u16;
    let memories_para = Paragraph::new(mem_lines)
        .scroll((scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Memories ({}) ", app.memories.len())),
        );
    f.render_widget(memories_para, memories_area);
}

fn draw_models(f: &mut Frame, app: &App, area: Rect, width: u16) {
    let block = Block::default().title(" Models ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [list_area, hint_area] = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(inner);

    let items: Vec<ListItem> = app
        .models
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let sel = if i == app.model_sel { "▶ " } else { "  " };
            let detail = m
                .parameter_size
                .clone()
                .or_else(|| m.size_bytes.map(|b| format!("{:.1} GB", b as f64 / 1e9)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(sel, Style::default().fg(Color::Yellow)),
                Span::raw(&m.name),
                Span::styled(format!("  {detail}"), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    if items.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "(no models loaded — press r to query the backend)",
                Style::default().fg(Color::DarkGray),
            )))
            .wrap(Wrap { trim: false }),
            list_area,
        );
    } else {
        f.render_widget(
            List::new(items).highlight_style(Style::default().add_modifier(Modifier::BOLD)),
            list_area,
        );
    }

    let remembered = app
        .remembered
        .as_ref()
        .map(|(k, m)| format!("{}/{}", k.as_str(), m))
        .unwrap_or_else(|| "none yet".to_string());
    let hint = if compact(width) {
        Line::from(vec![
            Span::raw("Enter: pick · r: reload · "),
            Span::styled(format!("remembered: {remembered}"), Style::default().fg(Color::Green)),
        ])
    } else {
        Line::from(vec![
            Span::raw("Enter/m: pick · r: reload · j/k: move · "),
            Span::styled(format!("remembered: {remembered}"), Style::default().fg(Color::Green)),
        ])
    };
    f.render_widget(Paragraph::new(hint), hint_area);
}

fn draw_tasks(f: &mut Frame, _app: &App, area: Rect) {
    let text = vec![
        Line::from(Span::styled(
            "Scheduled tasks — placeholder",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("The web TasksPanel (apps/deskagent/src/components/TasksPanel.tsx) models a local"),
        Line::from("task list (once | daily | weekly · due/roll/done). The core exposes no tasks table"),
        Line::from("yet, so the CLI mirrors the same placeholder: no runner at P0."),
        Line::from(""),
        Line::from("The runner lands with the GUI milestone: persisted tasks honoring 'next_run' with due"),
        Line::from("alerts. Tracked in PHASES.md Phase 8 notes (deferred, not cancelled)."),
        Line::from(""),
        Line::from(Span::styled(
            "This pane is informational until then.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Tasks "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(f: &mut Frame, app: &App, area: Rect, width: u16) {
    let backend = app
        .backend_override
        .map(|k| k.as_str())
        .unwrap_or("remembered");
    let model = app
        .remembered
        .as_ref()
        .map(|(_, m)| m.as_str())
        .unwrap_or("-");
    // Mobile-first ordering: hints and the transient status are always visible;
    // the static info sits at the right edge and clips first on narrow terminals.
    let (hints, status_cap) = if width >= 100 {
        (" Tab: switch · 1-4 pane · Esc/Ctrl-C: quit ", 60)
    } else if width >= 60 {
        (" 1-4 pane · Esc: quit · y/n approve ", 26)
    } else {
        (" 1-4 pane · Esc ", 14)
    };
    let status = truncate(app.status.trim(), status_cap);
    let status_span = if status.is_empty() {
        String::new()
    } else {
        format!("  {status}")
    };
    let info = format!("· {} · backend {backend} · model {model} · {}", app.tab.title(), app.encryption);
    let line = Line::from(vec![
        Span::styled(" deskagent ", Style::default().bg(Color::Cyan).fg(Color::Black)),
        Span::raw(hints),
        Span::styled(status_span, Style::default().fg(Color::DarkGray)),
        Span::raw(format!("  {info}")),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn short(id: &str) -> String {
    id.chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(app: &App) -> Terminal<TestBackend> {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal
    }

    use std::sync::atomic::{AtomicUsize, Ordering};

    static UI_DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

    fn app() -> App {
        // Each test gets its own data dir: the UI tests open real (encrypted) stores
        // and sharing one SQLite file races under parallel test threads.
        let seq = UI_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "deskagent-cli-ui-{}-{seq}",
            std::process::id()
        ));
        let store = crate::data::open_store(&dir);
        App::new(store, dir, None, Some("http://127.0.0.1:1".into()))
    }

    /// Reconstruct the rendered buffer as plain text (row-major cell symbols) so
    /// pane titles can be asserted on.
    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for row in 0..buf.area.height {
            for col in 0..buf.area.width {
                out.push_str(buf[(col, row)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn chat_tab_renders_panes_and_input() {
        let app = app();
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("DeskAgent"), "header bar");
        assert!(text.contains("Chat"), "chat tab title");
        assert!(text.contains("Memory+Approvals"), "memory tab title");
        assert!(text.contains("Models"), "models tab title");
        assert!(text.contains("Tasks"), "tasks tab title");
        assert!(text.contains("empty conversation"), "empty chat hint");
    }

    #[test]
    fn memory_tab_renders_persona_and_approvals() {
        let mut app = app();
        app.tab = Tab::Memory;
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("Memory+Approvals"));
        assert!(text.contains("Persona"));
        assert!(text.contains("No persona yet"));
        assert!(text.contains("pending"));
        assert!(text.contains("Memories"));
    }

    #[test]
    fn models_tab_renders_hint_when_empty() {
        let mut app = app();
        app.tab = Tab::Models;
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("no models loaded"));
    }

    #[test]
    fn models_tab_lists_models_and_remembered_choice() {
        let mut app = app();
        app.tab = Tab::Models;
        app.models = vec![deskagent_core::runtime::ModelInfo {
            name: "qwen2.5:7b".into(),
            size_bytes: Some(4_700_000_000),
            family: None,
            parameter_size: Some("7B".into()),
        }];
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("qwen2.5:7b"));
        assert!(text.contains("remembered"));
    }

    #[test]
    fn tasks_tab_is_informational() {
        let mut app = app();
        app.tab = Tab::Tasks;
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("Scheduled tasks"));
        assert!(text.contains("placeholder"));
    }

    #[test]
    fn chat_lines_render_with_citation_note() {
        let mut app = app();
        app.chat_lines.push(crate::app::ChatLine {
            role: "user".into(),
            text: "hello".into(),
            citations: 0,
        });
        app.chat_lines.push(crate::app::ChatLine {
            role: "assistant".into(),
            text: "hi there".into(),
            citations: 2,
        });
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("hello"));
        assert!(text.contains("hi there"));
        assert!(text.contains("recalled 2 memory/memories"));
    }

    #[test]
    fn status_bar_shows_encryption_and_model() {
        let app = app();
        let terminal = render(&app);
        let text = buffer_text(&terminal);
        assert!(text.contains("encrypted"));
        assert!(text.contains("backend"));
    }

    // ---- mobile (Moshi iOS / SSH): narrow layout, guard, status -----------

    fn render_size(app: &App, w: u16, h: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal
    }

    #[test]
    fn narrow_layout_uses_compact_tab_labels() {
        let app = app();
        let terminal = render_size(&app, 60, 24);
        let text = buffer_text(&terminal);
        assert!(text.contains(" Mem (0) "), "compact memory tab label at 60 cols");
        assert!(text.contains(" Chat "));
        assert!(text.contains(" Models "));
        assert!(!text.contains("Memory+Approvals ("), "full label replaced on narrow terminals");
    }

    #[test]
    fn tiny_terminal_shows_guard_screen() {
        let app = app();
        let terminal = render_size(&app, 30, 10);
        let text = buffer_text(&terminal);
        assert!(text.contains("too small"));
    }

    #[test]
    fn narrow_status_keeps_hints_and_transient_status() {
        let mut app = app();
        app.status = "approved appr-abc (+0.15 confidence)".to_string();
        let terminal = render_size(&app, 50, 20);
        let text = buffer_text(&terminal);
        assert!(text.contains("1-4 pane"), "narrow key hints visible");
        assert!(text.contains("approved appr"), "transient status visible on a narrow terminal");
    }

    #[test]
    fn wide_status_keeps_backend_and_encryption_info() {
        let app = app();
        let terminal = render_size(&app, 120, 40);
        let text = buffer_text(&terminal);
        assert!(text.contains("backend remembered"));
        assert!(text.contains("encrypted"));
    }
}