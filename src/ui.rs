//! Telegram-style layout: header, chat list on the left, conversation and
//! composer on the right, footer hints, overlays on top.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus, Level, Overlay, APP_NAME, APP_VERSION};
use crate::text;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const SIDEBAR_MIN: u16 = 22;

fn focused(app: &App, focus: Focus) -> Style {
    if app.focus == focus && app.overlay == Overlay::None && app.prompt.is_none() {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    // Below this the panes have no usable rows; show a hint instead of panicking.
    if area.height < 8 || area.width < 30 {
        let msg = Paragraph::new("Terminal too small — enlarge the window").style(
            Style::default().fg(Color::Yellow),
        );
        f.render_widget(msg, area);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(f, app, rows[0]);
    draw_footer(f, app, rows[2]);

    let sidebar_w = (rows[1].width / 3).clamp(SIDEBAR_MIN.min(rows[1].width), 44);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_w), Constraint::Min(20)])
        .split(rows[1]);

    draw_sidebar(f, app, cols[0]);
    draw_conversation(f, app, cols[1]);

    if app.overlay != Overlay::None {
        crate::ui_overlay::draw_overlay(f, app, rows[1]);
    }
    if app.prompt.is_some() {
        draw_prompt(f, app, rows[1]);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let who = app
        .me
        .as_ref()
        .map(|m| {
            let handle = m
                .username
                .as_deref()
                .map(|u| format!("@{}", u))
                .unwrap_or_else(|| m.phone.clone().unwrap_or_default());
            format!("{} {}", m.name, handle)
        })
        .unwrap_or_else(|| "not signed in".to_string());
    let left = format!(" {} v{} ", APP_NAME, APP_VERSION);
    let right = format!(
        "{} · {} {} ",
        who,
        app.sessions.len(),
        if app.sessions.len() == 1 { "account" } else { "accounts" }
    );
    let gap = (area.width as usize)
        .saturating_sub(text::width(&left) + text::width(&right));
    let spans = vec![
        Span::styled(
            left,
            Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().fg(ACCENT)),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let title = if app.filtering || !app.filter.is_empty() {
        format!(" Chats /{} ", app.filter.text)
    } else {
        format!(" Chats ({}) ", app.dialogs.len())
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focused(app, Focus::Sidebar))
        .title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let rows = app.visible_dialogs();
    if rows.is_empty() {
        let hint = if app.connected() {
            "no chats — press r to reload"
        } else {
            "press a for accounts, Ctrl+N to add one"
        };
        f.render_widget(
            Paragraph::new(hint).style(Style::default().fg(DIM)).wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let width = inner.width as usize;
    let items: Vec<ListItem> = rows
        .iter()
        .map(|(_, d)| {
            let badge = if d.unread > 0 {
                format!(" {}", d.unread)
            } else {
                String::new()
            };
            let name_w = width.saturating_sub(text::width(&badge) + 2);
            let mark = match d.kind.as_str() {
                "user" => '·',
                "group" => '#',
                _ => '@',
            };
            let name = text::pad(&text::one_line(&d.name), name_w);
            let mut spans = vec![
                Span::styled(format!("{} ", mark), Style::default().fg(DIM)),
                Span::styled(
                    name,
                    if d.unread > 0 {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
            ];
            if !badge.is_empty() {
                spans.push(Span::styled(
                    badge,
                    Style::default().fg(Color::Black).bg(ACCENT),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ListState::default();
    state.select(app.dialog_row());
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(if app.focus == Focus::Sidebar { ACCENT } else { DIM })
            .fg(Color::Black),
    );
    f.render_stateful_widget(list, inner, &mut state);
}

fn draw_conversation(f: &mut Frame, app: &mut App, area: Rect) {
    let composer_h = if app.reply_to.is_some() { 4 } else { 3 };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(composer_h)])
        .split(area);

    draw_messages(f, app, parts[0]);
    draw_composer(f, app, parts[1]);
}

fn draw_messages(f: &mut Frame, app: &mut App, area: Rect) {
    let title = match &app.open_peer {
        Some(_) => format!(" {} ", text::truncate(&app.open_title, area.width as usize - 4)),
        None => " No chat open ".to_string(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focused(app, Focus::Chat))
        .title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if app.open_peer.is_none() {
        let hint = "Select a chat on the left and press Enter.\nPress ? for the full key list.";
        f.render_widget(
            Paragraph::new(hint).style(Style::default().fg(DIM)).wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }
    if app.messages.is_empty() {
        f.render_widget(
            Paragraph::new("no messages loaded").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    }

    let width = inner.width as usize;
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|m| {
            let who = if m.outgoing { "You" } else { m.sender.as_str() };
            let head = Line::from(vec![
                Span::styled(
                    format!("{} ", m.time),
                    Style::default().fg(DIM),
                ),
                Span::styled(
                    who.to_string(),
                    Style::default()
                        .fg(if m.outgoing { Color::Green } else { ACCENT })
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            let mut lines = vec![head];
            if let Some(id) = m.reply_to {
                let quoted = app
                    .messages
                    .iter()
                    .find(|q| q.id == id)
                    .map(|q| text::one_line(&q.text))
                    .unwrap_or_else(|| format!("message {}", id));
                lines.push(Line::from(Span::styled(
                    format!("  ┊ {}", text::truncate(&quoted, width.saturating_sub(4))),
                    Style::default().fg(DIM),
                )));
            }
            for chunk in text::wrap(&m.text, width.saturating_sub(2)) {
                lines.push(Line::from(format!("  {}", chunk)));
            }
            if let Some(media) = &m.media {
                lines.push(Line::from(Span::styled(
                    format!("  [{}] press s to download", media),
                    Style::default().fg(Color::Magenta),
                )));
            }
            ListItem::new(lines)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.msg_sel.min(app.messages.len() - 1)));
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(if app.focus == Focus::Chat { ACCENT } else { DIM })
            .fg(Color::Black),
    );
    f.render_stateful_widget(list, inner, &mut state);
}

fn draw_composer(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focused(app, Focus::Composer))
        .title(if app.focus == Focus::Composer {
            " Message (Enter sends, Esc leaves) "
        } else {
            " Message (press i or Enter) "
        });
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut y = inner.y;
    if let Some(preview) = app.reply_preview() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("↳ {}", text::truncate(&preview, inner.width as usize - 2)),
                Style::default().fg(Color::Yellow),
            ))),
            Rect { height: 1, y, ..inner },
        );
        y += 1;
    }

    let field = Rect {
        y,
        height: inner.height.saturating_sub(y - inner.y).max(1),
        ..inner
    };
    let (shown, col) = app.composer.view(field.width as usize);
    if shown.is_empty() && app.focus != Focus::Composer {
        f.render_widget(
            Paragraph::new("type a message…").style(Style::default().fg(DIM)),
            field,
        );
    } else {
        f.render_widget(Paragraph::new(shown), field);
    }
    if app.focus == Focus::Composer && app.overlay == Overlay::None && app.prompt.is_none() {
        f.set_cursor_position((field.x + col as u16, field.y));
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    if let Some(toast) = &app.toast {
        let style = match toast.level {
            Level::Error => Style::default().fg(Color::Black).bg(Color::Red),
            Level::Info => Style::default().fg(Color::Black).bg(Color::Green),
        };
        let text = format!(" {} ", text::truncate(&toast.text, area.width as usize - 2));
        f.render_widget(Paragraph::new(Line::from(Span::styled(text, style))), area);
        return;
    }
    if app.tasks.busy() {
        const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
        let frame = FRAMES[(app.spinner / 2) as usize % FRAMES.len()];
        let label = format!(
            " {} {} · Esc cancels ",
            frame,
            app.tasks.labels().join(", ")
        );
        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(Color::Yellow)),
            area,
        );
        return;
    }
    let hints = match app.focus {
        Focus::Sidebar => "↑↓ move  Enter open  / filter  n send  a accounts  ? help  q quit",
        Focus::Chat => "↑↓ move  i write  r reply  f forward  e edit  d delete  s save  ? help",
        Focus::Composer => "Enter send  Esc back  Ctrl+U clear  Ctrl+W delete word",
    };
    f.render_widget(
        Paragraph::new(format!(" {}", hints)).style(Style::default().fg(DIM)),
        area,
    );
}

fn draw_prompt(f: &mut Frame, app: &App, area: Rect) {
    let width = area.width.saturating_sub(8).clamp(20, 72);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + area.height / 3,
        width,
        height: 3,
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} ", text::truncate(&app.prompt_title, width as usize - 4)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if inner.width == 0 {
        return;
    }
    let (shown, col) = app.prompt_input.view(inner.width as usize);
    let shown = if app.prompt_masked {
        "*".repeat(text::width(&shown))
    } else {
        shown
    };
    f.render_widget(Paragraph::new(shown), inner);
    f.set_cursor_position((inner.x + col as u16, inner.y));
}
