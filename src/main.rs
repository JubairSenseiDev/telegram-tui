mod app;
mod config;
mod tg;

use std::io;

use anyhow::Result;
use app::{
    App, DASHBOARD_ITEMS, LoginStage, Mode, Outcome, PromptKind, SetupStage, APP_NAME, APP_VERSION,
};
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use std::time::Duration;

fn main() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run())
}

async fn run() -> Result<()> {
    let mut app = App::new()?;
    if let Some(session) = app.cfg.last_session.clone() {
        if app.cfg.api_id.is_some() {
            let name = session;
            let api_id = app.cfg.api_id.unwrap();
            let path = app.cfg.session_path(&name);
            app.spawn("Connecting", async move {
                match tg::connect_session(&path, api_id).await {
                    Ok(sess) => Outcome::Connected(sess, name),
                    Err(e) => Outcome::Error(e.to_string()),
                }
            });
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = event_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    app.tg.disconnect();
    res
}

async fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_draw = std::time::Instant::now();
    while !app.quit {
        app.pump_pending();
        terminal.draw(|f| view(f, app))?;

        let deadline = last_draw + Duration::from_millis(80);
        let mut tick = false;
        while !tick && !app.quit {
            if let Ok(true) = event::poll(Duration::from_millis(40)) {
                match event::read()? {
                    Event::Key(key) => app.handle_key(key),
                    Event::Resize(..) => {}
                    _ => {}
                }
            } else if std::time::Instant::now() >= deadline {
                tick = true;
            }
        }
        if last_draw.elapsed() >= Duration::from_millis(80) {
            app.spinner = app.spinner.wrapping_add(1);
            last_draw = std::time::Instant::now();
        }
    }
    Ok(())
}

fn line<'a>(text: String, style: Style) -> Line<'a> {
    Line::from(Span::styled(text, style))
}

fn truncate_chars(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push_str("...");
    }
    out
}

fn content(title: String, style: Style) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(title, style))
}

fn center(area: Rect, w: u16, h: u16) -> Rect {
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(w), Constraint::Fill(1)])
        .split(area);
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(h), Constraint::Fill(1)])
        .split(horiz[1]);
    vert[1]
}

fn view(f: &mut ratatui::Frame, app: &mut App) {
    match app.mode {
        Mode::Dashboard => view_dashboard(f, app),
        Mode::Help => view_help(f),
        Mode::Profile => view_profile(f, app),
        Mode::Exports => view_exports(f, app),
        Mode::Dialogs => view_dialogs(f, app),
        Mode::Chat => view_chat(f, app),
        Mode::SearchResults => view_search(f, app),
        Mode::Members => view_members(f, app),
        Mode::Accounts => view_accounts(f, app),
        Mode::Setup => view_setup(f, app),
        Mode::Login => view_login(f, app),
        Mode::Prompt => view_prompt(f, app),
        Mode::Busy => view_busy(f, app),
    }
    if let Some(toast) = app.toast.clone() {
        draw_toast(f, toast);
    }
}

fn view_dashboard(f: &mut ratatui::Frame, app: &mut App) {
    let root = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(root);

    let status = if app.connected() {
        app.tg
            .me
            .as_ref()
            .map(|m| format!("connected: {}", m.name))
            .unwrap_or_else(|| "connected".to_string())
    } else {
        "not connected".to_string()
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} v{}", APP_NAME, APP_VERSION),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                status,
                if app.connected() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]))
        .block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );

    let items: Vec<ListItem> = DASHBOARD_ITEMS
        .iter()
        .enumerate()
        .map(|(i, (cmd, desc))| {
            let selected = i == app.dash_sel;
            let cmd_style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            };
            let desc_style = if selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<10}", cmd), cmd_style),
                Span::styled(desc.to_string(), desc_style),
            ]))
        })
        .collect();
    f.render_widget(
        List::new(items)
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black))
            .block(Block::default().borders(Borders::ALL).title("COMMANDS")),
        chunks[1],
    );

    let label = if app.input.is_empty() {
        "type /help or a command"
    } else {
        "command"
    };
    let input_widget = Paragraph::new(app.input.clone())
        .style(Style::default().fg(Color::White))
        .block(content(label.to_string(), Style::default().fg(Color::Magenta)));
    f.render_widget(input_widget, chunks[2]);
    f.set_cursor_position((chunks[2].x + 1 + app.input.chars().count() as u16, chunks[2].y + 1));
}

fn view_help(f: &mut ratatui::Frame) {
    let help_lines = vec![
        "TELEGRAM TUI - KEYBOARD-FIRST CLIENT",
        "",
        "Dashboard      type a command (like /help) or use arrows + Enter",
        "Common         Esc = back   Ctrl+C = quit",
        "Lists (dialogs, chat, accounts, search)",
        "  k / Up       move up        j / Down   move down",
        "  PageUp       jump up        PageDown   jump down",
        "  Enter        open / select",
        "Chat",
        "  s            send new message to this chat",
        "  r            reply to a message",
        "  e            export this chat history to a file",
        "  m            export members of this chat",
        "  o / l        load older messages",
        "  f            search inside this chat",
        "  d            delete selected message (type DELETE)",
        "  E            edit selected message",
        "  p / P        pin / unpin selected message",
        "  v            view members of this chat",
        "  g            download media of selected message",
        "  M            mark chat as read",
        "  R            refresh chat",
        "Members",
        "  j/k          scroll       x  export members CSV",
        "  e            export chat  Esc  back",
        "Dialogs",
        "  r            reload dialogs",
        "  x            export members of selected dialog",
        "  e            export chat history of selected dialog",
        "Accounts",
        "  Enter        switch to session",
        "  l            login a new account",
        "  d            delete selected session (type DELETE to confirm)",
        "Commands      /setup /login /inbox /send /sendfile /note /search",
        "              /dialogs /members /chat /profile /join /download",
        "              /accounts /exports /help /quit",
        "",
        "Press Esc to return.",
    ];
    let mut items: Vec<ListItem> = help_lines
        .iter()
        .map(|s| {
            if s.is_empty() {
                ListItem::new("")
            } else if s.ends_with(':') || *s == "TELEGRAM TUI - KEYBOARD-FIRST CLIENT" {
                ListItem::new(line(
                    s.to_string(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                ListItem::new(line(
                    s.to_string(),
                    Style::default().fg(Color::Gray),
                ))
            }
        })
        .collect();
    items.insert(0, ListItem::new(""));
    f.render_widget(
        List::new(items)
            .block(content(
                "HELP".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default()),
        f.area(),
    );
}

fn view_profile(f: &mut ratatui::Frame, app: &mut App) {
    let mut items: Vec<ListItem> = Vec::new();
    if let Some(me) = &app.me {
        items.push(ListItem::new(line(
            format!("name: {}", me.name),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        items.push(ListItem::new(line(
            format!("username: @{}", me.username.as_deref().unwrap_or("-")),
            Style::default().fg(Color::Gray),
        )));
        items.push(ListItem::new(line(
            format!("phone: {}", me.phone.as_deref().unwrap_or("-")),
            Style::default().fg(Color::Gray),
        )));
        items.push(ListItem::new(line(
            format!("user id: {}", me.id),
            Style::default().fg(Color::Gray),
        )));
    } else {
        items.push(ListItem::new("no account loaded"));
    }
    if let Some(name) = &app.tg.session {
        items.push(ListItem::new(line(
            format!("session: {}", name),
            Style::default().fg(Color::DarkGray),
        )));
    }
    items.push(ListItem::new(""));
    items.push(ListItem::new("Press Esc to return."));
    f.render_widget(
        List::new(items)
            .block(content(
                "PROFILE".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default()),
        f.area(),
    );
}

fn view_exports(f: &mut ratatui::Frame, app: &mut App) {
    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(line(
        format!("DOWNLOADS — {}", app.cfg.downloads_dir().to_string_lossy()),
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )));
    if app.downloads.is_empty() {
        items.push(ListItem::new("  no downloads yet"));
    } else {
        for p in &app.downloads {
            items.push(ListItem::new(line(
                format!("  {}", p.to_string_lossy()),
                Style::default().fg(Color::White),
            )));
        }
    }
    items.push(ListItem::new(""));
    items.push(ListItem::new(line(
        format!("EXPORTS — {}", app.cfg.exports_dir().to_string_lossy()),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    if app.exports.is_empty() {
        items.push(ListItem::new("  no exported files yet"));
    } else {
        for p in &app.exports {
            items.push(ListItem::new(line(
                format!("  {}", p.to_string_lossy()),
                Style::default().fg(Color::White),
            )));
        }
    }
    f.render_widget(
        List::new(items)
            .block(content(
                "FILES".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default()),
        f.area(),
    );
    let w = f.area().width;
    let hint = Paragraph::new("Press Esc to return.").style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(f.area().x, f.area().y + f.area().height - 1, w, 1));
}

fn view_dialogs(f: &mut ratatui::Frame, app: &mut App) {
    let items: Vec<ListItem> = if app.dialogs.is_empty() {
        vec![ListItem::new("no dialogs (press r to reload)")]
    } else {
        app.dialogs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let selected = i == app.dialogs_sel;
                let name_style = if selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                };
                let meta_style = if selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                };
                let unread = if d.unread > 0 {
                    format!(" [{}] ", d.unread)
                } else {
                    " ".to_string()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("  {:<24}", d.name), name_style),
                    Span::styled(unread, Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("({}) {}", d.kind, d.last),
                        meta_style,
                    ),
                ]))
            })
            .collect()
    };
    f.render_widget(
        List::new(items)
            .block(content(
                "DIALOGS".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black)),
        f.area(),
    );
    let hint = Paragraph::new("Enter open | r reload | x members | e export chat | Esc back")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(f.area().x, f.area().y + f.area().height - 1, f.area().width, 1));
}

fn view_chat(f: &mut ratatui::Frame, app: &mut App) {
    let title = app
        .dialogs
        .get(app.dialogs_sel)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "CHAT".to_string());
    let mut items: Vec<ListItem> = Vec::new();
    if app.messages.is_empty() {
        items.push(ListItem::new("no messages yet"));
    } else {
        for m in &app.messages {
            let sender_style = if m.outgoing {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            };
            let body_style = Style::default().fg(Color::White);
            let header = format!("  #{}  {}  {}", m.id, m.sender, m.time);
            let body = truncate_chars(&m.text, 120);
            items.push(ListItem::new(Line::from(vec![
                Span::styled(header, sender_style),
            ])));
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("    {}", body),
                body_style,
            )])));
        }
    }
    items.push(ListItem::new(""));
    f.render_widget(
        List::new(items)
            .block(content(
                title,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default()),
        f.area(),
    );

    let last = app.messages.len().saturating_sub(1);
    let pos = if last > 0 {
        format!(" {}/{} ", app.msg_scroll.min(last) + 1, app.messages.len())
    } else {
        " ".to_string()
    };
    let hint = Paragraph::new(format!(
        "j/k scroll | s send | r reply | e export chat | m members | g download | Home/End | Esc back{}",
        pos
    ))
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(f.area().x, f.area().y + f.area().height - 1, f.area().width, 1));

    let page = (f.area().height as usize).saturating_sub(2);
    let mut target = app.msg_scroll;
    if target > app.messages.len().saturating_sub(1) {
        target = app.messages.len().saturating_sub(1);
    }
    let _ = page;
    if target > 0 {
        let offset = target.min(last.saturating_sub(1));
        f.set_cursor_position((f.area().x + 2, f.area().y + 1 + (offset % page) as u16));
    }
}

fn view_search(f: &mut ratatui::Frame, app: &mut App) {
    let items: Vec<ListItem> = if app.search.is_empty() {
        vec![ListItem::new("no results")]
    } else {
        app.search
            .iter()
            .map(|m| {
                let sender = if m.outgoing {
                    "Me".to_string()
                } else {
                    m.sender.clone()
                };
                let text = truncate_chars(&m.text, 100);
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{} {} ", m.time, sender),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(text, Style::default().fg(Color::White)),
                ]))
            })
            .collect()
    };
    f.render_widget(
        List::new(items)
            .block(content(
                "SEARCH RESULTS".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default()),
        f.area(),
    );
    let last = app.search.len().saturating_sub(1);
    let pos = if last > 0 {
        format!(" {}/{} ", app.msg_scroll.min(last) + 1, app.search.len())
    } else {
        " ".to_string()
    };
    let hint = Paragraph::new(format!("j/k scroll | PageUp/Down | Esc back{}", pos))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(f.area().x, f.area().y + f.area().height - 1, f.area().width, 1));
}

fn view_members(f: &mut ratatui::Frame, app: &mut App) {
    let items: Vec<ListItem> = if app.members.is_empty() {
        vec![ListItem::new("no members")]
    } else {
        app.members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let selected = i == app.members_sel;
                let base = if selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                let name = {
                    let full = format!("{} {}", m.first, m.last);
                    let full = full.trim();
                    if full.is_empty() { "unknown".to_string() } else { full.to_string() }
                };
                let uname = m.username.as_deref().map(|u| format!("@{}", u)).unwrap_or_default();
                let phone = m.phone.as_deref().unwrap_or("");
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  #{} {:<26}", m.id, name),
                        base.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{:<20}", uname), base),
                    Span::styled(phone.to_string(), base),
                ]))
            })
            .collect()
    };
    f.render_widget(
        List::new(items)
            .block(content(
                "MEMBERS".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black)),
        f.area(),
    );
    let last = app.members.len().saturating_sub(1);
    let pos = if last > 0 {
        format!(" {}/{} ", app.members_sel.min(last) + 1, app.members.len())
    } else {
        " ".to_string()
    };
    let hint = Paragraph::new(format!("j/k scroll | x export members | e export chat | Esc back{}", pos))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(f.area().x, f.area().y + f.area().height - 1, f.area().width, 1));
}

fn view_accounts(f: &mut ratatui::Frame, app: &mut App) {
    let items: Vec<ListItem> = if app.sessions.is_empty() {
        vec![ListItem::new("no sessions yet - press l to login")]
    } else {
        app.sessions
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let selected = i == app.accounts_sel;
                let current = app.tg.session.as_deref() == Some(name.as_str());
                let s = if current {
                    format!("  {}  (active)", name)
                } else {
                    format!("  {}", name)
                };
                ListItem::new(line(
                    s,
                    if selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ))
            })
            .collect()
    };
    f.render_widget(
        List::new(items)
            .block(content(
                "ACCOUNTS".to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black)),
        f.area(),
    );
    let hint = Paragraph::new("Enter switch | l login | d delete | Esc back")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(f.area().x, f.area().y + f.area().height - 1, f.area().width, 1));
}

fn view_setup(f: &mut ratatui::Frame, app: &mut App) {
    let (title, prompt, mask) = match app.setup_stage {
        SetupStage::ApiId => (
            "SETUP — API ID",
            "API ID (from my.telegram.org)",
            false,
        ),
        SetupStage::ApiHash => (
            "SETUP — API HASH",
            "API HASH",
            true,
        ),
    };
    let area = center(f.area(), 64, 7);
    let block = content(title.to_string(), Style::default().fg(Color::Magenta));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    f.render_widget(
        Paragraph::new(prompt.to_string()).style(Style::default().fg(Color::Gray)),
        chunks[0],
    );
    let shown = if mask {
        "*".repeat(app.input.chars().count())
    } else {
        app.input.clone()
    };
    f.render_widget(
        Paragraph::new(shown.clone()).style(Style::default().fg(Color::White)),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new("Enter confirm | Esc cancel").style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
    f.set_cursor_position((chunks[1].x + shown.chars().count() as u16, chunks[1].y));
}

fn view_login(f: &mut ratatui::Frame, app: &mut App) {
    let (title, prompt, mask) = match app.login_stage {
        LoginStage::Phone => (
            "LOGIN — PHONE",
            "phone number (e.g. +1234567890)",
            false,
        ),
        LoginStage::Code => (
            "LOGIN — CODE",
            "verification code",
            false,
        ),
        LoginStage::Password => (
            "LOGIN — PASSWORD",
            "2FA password",
            true,
        ),
    };
    let area = center(f.area(), 64, 7);
    let block = content(title.to_string(), Style::default().fg(Color::Green));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    f.render_widget(
        Paragraph::new(prompt.to_string()).style(Style::default().fg(Color::Gray)),
        chunks[0],
    );
    let shown = if mask {
        "*".repeat(app.input.chars().count())
    } else {
        app.input.clone()
    };
    f.render_widget(
        Paragraph::new(shown.clone()).style(Style::default().fg(Color::White)),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new("Enter confirm | Esc cancel").style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
    f.set_cursor_position((chunks[1].x + shown.chars().count() as u16, chunks[1].y));
}

fn view_prompt(f: &mut ratatui::Frame, app: &mut App) {
    let title = match app.prompt_kind {
        PromptKind::None => "PROMPT".to_string(),
        kind => format!(
            "{} — {}",
            match kind {
                PromptKind::SendTarget => "SEND",
                PromptKind::SendMessage => "SEND MESSAGE",
                PromptKind::Reply => "REPLY",
                PromptKind::Note => "NOTE",
                PromptKind::Search => "SEARCH",
                PromptKind::ExportChatTarget => "EXPORT CHAT",
                PromptKind::ExportMembersTarget => "EXPORT MEMBERS",
                PromptKind::JoinTarget => "JOIN",
                PromptKind::DeleteConfirm => "DELETE",
                PromptKind::DeleteMessageConfirm => "DELETE MESSAGE",
                PromptKind::SearchInChat => "SEARCH IN CHAT",
                PromptKind::EditMessage => "EDIT MESSAGE",
                PromptKind::SendFileTarget => "SEND FILE",
                PromptKind::SendFilePath => "SEND FILE",
                PromptKind::DownloadLink => "DOWNLOAD",
                PromptKind::None => "PROMPT",
            },
            app.prompt_title
        ),
    };
    let area = center(f.area(), 64, 7);
    let block = content(title, Style::default().fg(Color::Yellow));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    let shown = app.input.clone();
    f.render_widget(
        Paragraph::new(shown.clone()).style(Style::default().fg(Color::White)),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new("Enter confirm | Esc cancel").style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
    f.set_cursor_position((chunks[1].x + shown.chars().count() as u16, chunks[1].y));
}

fn view_busy(f: &mut ratatui::Frame, app: &mut App) {
    let frames = ["|", "/", "-", "\\"];
    let spinner = frames[(app.spinner % 4) as usize];
    let area = center(f.area(), 44, 5);
    let block = content(
        "WORKING".to_string(),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    );
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    f.render_widget(
        Paragraph::new(format!("{}  {}", spinner, app.busy_title))
            .style(Style::default().fg(Color::Yellow)),
        chunks[0],
    );
}

fn draw_toast(f: &mut ratatui::Frame, toast: String) {
    let area = f.area();
    let w = area.width.saturating_sub(4);
    let text = if toast.chars().count() > w as usize {
        format!("{}...", toast.chars().take(w as usize).collect::<String>())
    } else {
        toast
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Yellow).fg(Color::Black));
    let tw = text.chars().count() as u16 + 2;
    let x = area.x + area.width.saturating_sub(tw) / 2;
    let y = area.y + 1;
    let rect = Rect::new(x, y, tw, 1);
    f.render_widget(block.clone(), rect);
    let inner = block.inner(rect);
    f.render_widget(Paragraph::new(text), inner);
}
