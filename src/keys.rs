//! Key handling. Focus decides who receives a key, so typing never collides
//! with navigation shortcuts.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::{App, Focus, Overlay, Prompt};

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if self.prompt.is_some() {
            return self.key_prompt(key);
        }
        if self.overlay != Overlay::None {
            return self.key_overlay(key);
        }
        if self.filtering {
            return self.key_filter(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.key_control(key);
        }
        if key.code == KeyCode::Tab {
            return self.cycle_focus();
        }
        match self.focus {
            Focus::Sidebar => self.key_sidebar(key),
            Focus::Chat => self.key_chat(key),
            Focus::Composer => self.key_composer(key),
        }
    }

    /// Tab walks sidebar → conversation → composer, skipping panes that have
    /// nothing to show because no chat is open.
    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar if self.open_peer.is_some() => Focus::Chat,
            Focus::Sidebar => Focus::Sidebar,
            Focus::Chat => Focus::Composer,
            Focus::Composer => Focus::Sidebar,
        };
    }

    fn key_control(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') => self.quit = true,
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('f') => self.start_filter(),
            KeyCode::Char('n') => self.begin_login(),
            KeyCode::Char('a') => self.open_accounts(),
            KeyCode::Char('r') => self.load_dialogs(),
            _ => {}
        }
    }

    fn start_filter(&mut self) {
        self.filtering = true;
        self.filter.clear();
        self.focus = Focus::Sidebar;
    }

    fn key_filter(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.filtering = false;
                self.filter.clear();
            }
            KeyCode::Enter => {
                self.filtering = false;
                if let Some((idx, _)) = self.visible_dialogs().first() {
                    let idx = *idx;
                    self.open_chat(idx);
                }
            }
            KeyCode::Backspace => self.filter.backspace(),
            KeyCode::Left => self.filter.left(),
            KeyCode::Right => self.filter.right(),
            KeyCode::Char(c) => self.filter.insert(c),
            _ => {}
        }
    }

    fn key_sidebar(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Down | KeyCode::Char('j') => self.move_dialog(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_dialog(-1),
            KeyCode::PageDown => self.move_dialog(10),
            KeyCode::PageUp => self.move_dialog(-10),
            KeyCode::Home => self.dialog_sel = 0,
            KeyCode::End => self.dialog_sel = self.dialogs.len().saturating_sub(1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                let sel = self.dialog_sel;
                if self.dialogs.get(sel).is_some() {
                    self.open_chat(sel);
                }
            }
            KeyCode::Char('/') => self.start_filter(),
            KeyCode::Char('g') => self.ask(Prompt::GlobalSearch, "Search all messages"),
            KeyCode::Char('n') => self.ask(Prompt::SendTarget, "Send to (@username or id)"),
            KeyCode::Char('w') => self.ask(Prompt::Note, "Note to Saved Messages"),
            KeyCode::Char('u') => self.ask(Prompt::SendFileTarget, "Send file to"),
            KeyCode::Char('J') => self.ask(Prompt::JoinTarget, "Join (@name or invite link)"),
            KeyCode::Char('D') => self.ask(Prompt::DownloadLink, "Media link (t.me/...)"),
            KeyCode::Char('m') => self.ask(Prompt::MembersTarget, "Members of"),
            KeyCode::Char('e') => self.export_dialogs(),
            KeyCode::Char('X') => self.ask(Prompt::ExportChatTarget, "Export history of"),
            KeyCode::Char('M') => self.ask(Prompt::ExportMembersTarget, "Export members of"),
            KeyCode::Char('E') => self.open_exports(),
            KeyCode::Char('a') => self.open_accounts(),
            KeyCode::Char('S') => self.load_statuses(),
            KeyCode::Char('p') => self.open_profile(),
            KeyCode::Char('L') => self.load_leavable(),
            KeyCode::Char('R') => self.ask(Prompt::ReportTarget, "Report (@username or id)"),
            KeyCode::Char('r') => self.load_dialogs(),
            KeyCode::Esc => self.cancel_tasks(),
            _ => {}
        }
    }

    fn move_dialog(&mut self, delta: isize) {
        let rows = self.visible_dialogs();
        if rows.is_empty() {
            return;
        }
        let cur = self.dialog_row().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.dialog_sel = rows[next].0;
    }

    fn key_chat(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => self.focus = Focus::Sidebar,
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Down | KeyCode::Char('j') => self.move_msg(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_msg(-1),
            KeyCode::PageDown => self.move_msg(10),
            KeyCode::PageUp => self.move_msg(-10),
            KeyCode::End => self.msg_sel = self.messages.len().saturating_sub(1),
            KeyCode::Home => {
                self.msg_sel = 0;
                self.load_older();
            }
            KeyCode::Enter | KeyCode::Char('i') => self.focus = Focus::Composer,
            KeyCode::Char('r') => self.begin_reply(),
            KeyCode::Char('f') => self.begin_forward(),
            KeyCode::Char('d') => self.begin_delete(),
            KeyCode::Char('e') => self.begin_edit(),
            KeyCode::Char('p') => self.pin_selected(false),
            KeyCode::Char('P') => self.pin_selected(true),
            KeyCode::Char('s') => self.download_selected(),
            KeyCode::Char('/') => self.ask(Prompt::ChatSearch, "Search in this chat"),
            KeyCode::Char('m') => self.mark_read(),
            KeyCode::Char('o') => self.load_older(),
            KeyCode::Char('R') => self.refresh_chat(),
            _ => {}
        }
    }

    fn move_msg(&mut self, delta: isize) {
        if self.messages.is_empty() {
            return;
        }
        let last = self.messages.len() as isize - 1;
        let next = (self.msg_sel as isize + delta).clamp(0, last) as usize;
        self.msg_sel = next;
    }

    fn key_composer(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('w') => self.composer.delete_word(),
                KeyCode::Char('u') => self.composer.clear(),
                KeyCode::Char('a') => self.composer.home(),
                KeyCode::Char('e') => self.composer.end(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => {
                if self.reply_to.is_some() {
                    self.reply_to = None;
                } else {
                    self.focus = Focus::Chat;
                }
            }
            KeyCode::Enter => {
                let text = self.composer.take();
                if !text.is_empty() {
                    self.send_current(text);
                }
            }
            KeyCode::Backspace => self.composer.backspace(),
            KeyCode::Delete => self.composer.delete(),
            KeyCode::Left => self.composer.left(),
            KeyCode::Right => self.composer.right(),
            KeyCode::Home => self.composer.home(),
            KeyCode::End => self.composer.end(),
            KeyCode::Char(c) => self.composer.insert(c),
            _ => {}
        }
    }

    fn begin_reply(&mut self) {
        match self.selected_message() {
            Some(m) => {
                self.reply_to = Some(m.id);
                self.focus = Focus::Composer;
            }
            None => self.error("select a message first"),
        }
    }

    fn begin_forward(&mut self) {
        match self.selected_message() {
            Some(m) => {
                self.set_forward(Some(m.id));
                self.overlay = Overlay::Forward;
                self.overlay_sel = 0;
            }
            None => self.error("select a message first"),
        }
    }

    fn begin_delete(&mut self) {
        let Some(msg) = self.selected_message().cloned() else {
            return self.error("select a message first");
        };
        let preview = crate::text::truncate(&crate::text::one_line(&msg.text), 30);
        let chat = crate::text::truncate(&self.open_title, 20);
        self.ask(
            Prompt::DeleteMessage,
            &format!("Delete \"{}\" in {}? type DELETE", preview, chat),
        );
    }

    fn begin_edit(&mut self) {
        let Some(msg) = self.selected_message().cloned() else {
            return self.error("select a message first");
        };
        if !msg.outgoing {
            return self.error("you can only edit your own messages");
        }
        self.ask_prefilled(Prompt::EditMessage, "Edit message", &msg.text);
    }

    fn open_accounts(&mut self) {
        self.sessions = self.cfg.list_sessions();
        self.overlay = Overlay::Accounts;
        self.overlay_sel = 0;
    }

    fn open_exports(&mut self) {
        self.exports = self.cfg.list_exports();
        self.exports.extend(self.cfg.list_downloads());
        self.overlay = Overlay::Exports;
        self.overlay_sel = 0;
    }

    fn open_profile(&mut self) {
        self.overlay = Overlay::Profile;
        self.overlay_sel = 0;
    }
}
