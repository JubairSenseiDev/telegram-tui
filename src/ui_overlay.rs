//! The panels drawn over the conversation pane.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Overlay};
use crate::keys_modal::PROFILE_ITEMS;
use crate::text;
use crate::tg::REPORT_REASONS;

const DIM: Color = Color::DarkGray;

pub const HELP: &[(&str, &str)] = &[
    ("Chat list", "↑↓/jk move · Enter open · / filter · r reload"),
    ("Conversation", "↑↓ move · o older · Home load older · End newest"),
    ("Write", "i or Enter focus composer · Enter send · Esc back"),
    ("Message", "r reply · f forward · e edit · d delete · p/P pin · s save media"),
    ("Find", "/ search this chat · g search everywhere · m mark read"),
    ("Send", "n message someone · u send file · w note to self · D download link"),
    ("Chats", "J join · L leave groups · m members · R report"),
    ("Export", "e chats CSV · M members CSV · X chat history · E open exports"),
    ("Accounts", "a accounts · S account status · p profile · Ctrl+N add account"),
    ("General", "Tab switch pane · Esc cancel running work · ? help · q quit"),
];

pub fn draw_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    let width = area.width.saturating_sub(4).clamp(24, 90);
    let height = area.height.saturating_sub(2).clamp(6, 24);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, rect);

    let (title, footer) = match app.overlay {
        Overlay::Help => (" Keys ", "Esc closes"),
        Overlay::Accounts => (" Accounts ", "Enter switch · n add · d delete · Esc close"),
        Overlay::Members => (" Members ", "Enter message · e export CSV · Esc close"),
        Overlay::Exports => (" Files ", "Esc close"),
        Overlay::Profile => (" Profile ", "Enter edit · Esc close"),
        Overlay::Status => (" Account status ", "Enter switch · Esc close"),
        Overlay::Leave => (" Leave groups ", "Enter leave · Esc close"),
        Overlay::Forward => (" Forward to ", "Enter send · Esc cancel"),
        Overlay::Search => (" Results ", "Enter jump · Esc close"),
        Overlay::Report => (" Report reason ", "Enter choose · Esc cancel"),
        Overlay::None => return,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title)
        .title_bottom(Line::from(Span::styled(
            format!(" {} ", footer),
            Style::default().fg(DIM),
        )));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if app.overlay == Overlay::Help {
        return draw_help(f, inner);
    }
    if app.overlay == Overlay::Profile {
        return draw_profile(f, app, inner);
    }

    let w = inner.width as usize;
    let items: Vec<ListItem> = match app.overlay {
        Overlay::Accounts => app
            .sessions
            .iter()
            .map(|name| {
                let active = app.tg.session.as_deref() == Some(name.as_str());
                let mark = if active { "● " } else { "  " };
                ListItem::new(Line::from(vec![
                    Span::styled(mark, Style::default().fg(Color::Green)),
                    Span::raw(text::truncate(name, w.saturating_sub(2))),
                ]))
            })
            .collect(),
        Overlay::Members => app
            .members
            .iter()
            .map(|m| {
                let name = format!("{} {}", m.first, m.last).trim().to_string();
                let handle = m
                    .username
                    .as_deref()
                    .map(|u| format!("@{}", u))
                    .or_else(|| m.phone.clone())
                    .unwrap_or_else(|| m.id.to_string());
                ListItem::new(Line::from(vec![
                    Span::raw(text::pad(&name, w.saturating_sub(22).max(4))),
                    Span::styled(text::truncate(&handle, 20), Style::default().fg(DIM)),
                ]))
            })
            .collect(),
        Overlay::Exports => app
            .exports
            .iter()
            .map(|p| {
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                ListItem::new(text::truncate(&name, w))
            })
            .collect(),
        Overlay::Status => app
            .statuses
            .iter()
            .map(|s| {
                let detail = match (&s.me, &s.error) {
                    (Some(me), _) => format!(
                        "{} · {}",
                        me.name,
                        me.username
                            .as_deref()
                            .map(|u| format!("@{}", u))
                            .unwrap_or_else(|| me.phone.clone().unwrap_or_default())
                    ),
                    (None, Some(e)) => e.clone(),
                    (None, None) => "unknown".to_string(),
                };
                let colour = if s.me.is_some() { Color::Green } else { Color::Red };
                ListItem::new(Line::from(vec![
                    Span::raw(text::pad(&s.session, 16.min(w))),
                    Span::styled(
                        text::truncate(&detail, w.saturating_sub(17)),
                        Style::default().fg(colour),
                    ),
                ]))
            })
            .collect(),
        Overlay::Leave => app
            .leavable
            .iter()
            .map(|d| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", d.kind), Style::default().fg(DIM)),
                    Span::raw(text::truncate(&d.name, w.saturating_sub(10))),
                ]))
            })
            .collect(),
        Overlay::Forward => app
            .dialogs
            .iter()
            .map(|d| ListItem::new(text::truncate(&text::one_line(&d.name), w)))
            .collect(),
        Overlay::Search => app
            .found
            .iter()
            .map(|m| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", m.time), Style::default().fg(DIM)),
                    Span::raw(text::truncate(&text::one_line(&m.text), w.saturating_sub(7))),
                ]))
            })
            .collect(),
        Overlay::Report => REPORT_REASONS
            .iter()
            .map(|(_, label)| ListItem::new(text::truncate(label, w)))
            .collect(),
        Overlay::Help | Overlay::Profile | Overlay::None => Vec::new(),
    };

    if items.is_empty() {
        let msg = if app.tasks.busy() { "loading…" } else { "nothing here" };
        f.render_widget(Paragraph::new(msg).style(Style::default().fg(DIM)), inner);
        return;
    }

    let mut state = ListState::default();
    state.select(Some(app.overlay_sel.min(items.len() - 1)));
    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));
    f.render_stateful_widget(list, inner, &mut state);
}

fn draw_help(f: &mut Frame, inner: Rect) {
    let w = inner.width as usize;
    let label_w = 14.min(w / 3);
    let lines: Vec<Line> = HELP
        .iter()
        .map(|(section, keys)| {
            Line::from(vec![
                Span::styled(
                    text::pad(section, label_w),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw(text::truncate(keys, w.saturating_sub(label_w))),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_profile(f: &mut Frame, app: &mut App, inner: Rect) {
    let w = inner.width as usize;
    let mut items: Vec<ListItem> = Vec::new();
    if let Some(me) = &app.me {
        let handle = me
            .username
            .as_deref()
            .map(|u| format!("@{}", u))
            .unwrap_or_else(|| "no username".to_string());
        items.push(ListItem::new(Line::from(Span::styled(
            text::truncate(&format!("{} · {} · id {}", me.name, handle, me.id), w),
            Style::default().fg(Color::Green),
        ))));
    }
    for (label, detail) in PROFILE_ITEMS {
        items.push(ListItem::new(Line::from(vec![
            Span::raw(text::pad(label, 16.min(w))),
            Span::styled(
                text::truncate(detail, w.saturating_sub(17)),
                Style::default().fg(DIM),
            ),
        ])));
    }
    // The header row is not selectable, so offset the highlight past it.
    let offset = usize::from(app.me.is_some());
    let mut state = ListState::default();
    state.select(Some(app.overlay_sel + offset));
    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));
    f.render_stateful_widget(list, inner, &mut state);
}
