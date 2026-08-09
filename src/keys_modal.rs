//! Keys for the overlay panels and the prompt line.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions::DEFAULT_MEMBER_LIMIT;
use crate::app::{App, Focus, Overlay, Prompt, Purpose};
use crate::tg::REPORT_REASONS;

/// Rows the focused overlay currently shows, for clamping the selection.
pub fn overlay_len(app: &App) -> usize {
    match app.overlay {
        Overlay::Accounts => app.sessions.len(),
        Overlay::Members => app.members.len(),
        Overlay::Exports => app.exports.len(),
        Overlay::Status => app.statuses.len(),
        Overlay::Leave => app.leavable.len(),
        Overlay::Forward => app.dialogs.len(),
        Overlay::Search => app.found.len(),
        Overlay::Report => REPORT_REASONS.len(),
        Overlay::Profile => PROFILE_ITEMS.len(),
        Overlay::Help | Overlay::None => 0,
    }
}

pub const PROFILE_ITEMS: &[(&str, &str)] = &[
    ("First name", "change the name shown to others"),
    ("Last name", "change or clear your last name"),
    ("Bio", "up to 70 characters"),
    ("Username", "your public @handle"),
    ("Profile photo", "upload from a local file"),
];

impl App {
    pub(crate) fn key_overlay(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        let len = overlay_len(self);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_overlay(),
            KeyCode::Down | KeyCode::Char('j') => self.move_overlay(1, len),
            KeyCode::Up | KeyCode::Char('k') => self.move_overlay(-1, len),
            KeyCode::PageDown => self.move_overlay(10, len),
            KeyCode::PageUp => self.move_overlay(-10, len),
            KeyCode::Home => self.overlay_sel = 0,
            KeyCode::End => self.overlay_sel = len.saturating_sub(1),
            KeyCode::Enter => self.activate_overlay(),
            KeyCode::Char('d') if self.overlay == Overlay::Accounts => {
                if let Some(name) = self.sessions.get(self.overlay_sel).cloned() {
                    self.ask(
                        Prompt::DeleteSession,
                        &format!("Delete session \"{}\"? type DELETE", name),
                    );
                }
            }
            KeyCode::Char('n') if self.overlay == Overlay::Accounts => self.begin_login(),
            KeyCode::Char('e') if self.overlay == Overlay::Members => {
                if let Some(peer) = self.open_peer.clone() {
                    self.export_members(peer, DEFAULT_MEMBER_LIMIT);
                }
            }
            _ => {}
        }
    }

    fn move_overlay(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.overlay_sel = 0;
            return;
        }
        let last = len as isize - 1;
        self.overlay_sel = (self.overlay_sel as isize + delta).clamp(0, last) as usize;
    }

    pub(crate) fn close_overlay(&mut self) {
        self.overlay = Overlay::None;
        self.overlay_sel = 0;
        self.set_forward(None);
    }

    fn activate_overlay(&mut self) {
        let sel = self.overlay_sel;
        match self.overlay {
            Overlay::Accounts => {
                if let Some(name) = self.sessions.get(sel).cloned() {
                    self.close_overlay();
                    self.connect(name);
                }
            }
            Overlay::Status => {
                if let Some(status) = self.statuses.get(sel).cloned() {
                    self.close_overlay();
                    self.connect(status.session);
                }
            }
            Overlay::Forward => {
                let (Some(id), Some(peer)) =
                    (self.forward_target(), self.dialogs.get(sel).map(|d| d.peer.clone()))
                else {
                    return;
                };
                self.close_overlay();
                self.forward_selected(id, peer);
            }
            Overlay::Leave => {
                if let Some(dialog) = self.leavable.get(sel).cloned() {
                    self.stash_peer(dialog.peer);
                    self.ask(
                        Prompt::LeaveConfirm,
                        &format!("Leave \"{}\"? type LEAVE", dialog.name),
                    );
                }
            }
            Overlay::Search => {
                if let Some(hit) = self.found.get(sel).cloned() {
                    self.close_overlay();
                    if let Some(row) = self.messages.iter().position(|m| m.id == hit.id) {
                        self.msg_sel = row;
                        self.focus = Focus::Chat;
                    } else {
                        self.info(format!("message {} is outside the loaded page", hit.id));
                    }
                }
            }
            Overlay::Report => {
                self.set_report_reason(sel);
                self.ask(Prompt::ReportDetail, "Report detail (optional)");
            }
            Overlay::Profile => match sel {
                0 => self.ask(Prompt::ProfileFirst, "First name"),
                1 => self.ask(Prompt::ProfileLast, "Last name"),
                2 => self.ask(Prompt::ProfileBio, "Bio (max 70 chars)"),
                3 => self.ask(Prompt::ProfileUsername, "Username"),
                _ => self.ask(Prompt::ProfilePhoto, "Path to image"),
            },
            Overlay::Members => {
                if let Some(m) = self.members.get(sel).cloned() {
                    let who = m
                        .username
                        .map(|u| format!("@{}", u))
                        .unwrap_or_else(|| m.id.to_string());
                    self.close_overlay();
                    self.stash_text(String::new());
                    self.set_pending(Purpose::Send);
                    self.resolve(Purpose::Send, who);
                }
            }
            Overlay::Exports | Overlay::Help | Overlay::None => {}
        }
    }

    pub(crate) fn key_prompt(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('u') => self.prompt_input.clear(),
                KeyCode::Char('w') => self.prompt_input.delete_word(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.prompt = None;
                self.prompt_input.clear();
                self.prompt_masked = false;
            }
            KeyCode::Enter => {
                let value = self.prompt_input.take();
                let Some(kind) = self.prompt.take() else { return };
                self.prompt_masked = false;
                self.submit_prompt(kind, value);
            }
            KeyCode::Backspace => self.prompt_input.backspace(),
            KeyCode::Delete => self.prompt_input.delete(),
            KeyCode::Left => self.prompt_input.left(),
            KeyCode::Right => self.prompt_input.right(),
            KeyCode::Home => self.prompt_input.home(),
            KeyCode::End => self.prompt_input.end(),
            KeyCode::Char(c) => self.prompt_input.insert(c),
            _ => {}
        }
    }
}
