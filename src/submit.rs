//! What each prompt does once the user presses Enter.

use crate::actions::{DEFAULT_EXPORT_LIMIT, DEFAULT_MEMBER_LIMIT};
use crate::app::{App, Outcome, Prompt, Purpose};
use crate::tg::{self, REPORT_REASONS};

impl App {
    pub(crate) fn submit_prompt(&mut self, kind: Prompt, value: String) {
        match kind {
            Prompt::ApiId => match value.trim().parse::<i32>() {
                Ok(id) => {
                    self.set_pending_api_id(id);
                    self.ask(Prompt::ApiHash, "API hash");
                }
                Err(_) => {
                    self.error("api id must be a number");
                    self.ask(Prompt::ApiId, "API id");
                }
            },
            Prompt::ApiHash => {
                let Some(id) = self.take_pending_api_id() else {
                    return self.error("start setup again");
                };
                if value.is_empty() {
                    return self.error("api hash cannot be empty");
                }
                match self.cfg.save_credentials(id, &value) {
                    Ok(()) => {
                        self.cfg.api_id = Some(id);
                        self.cfg.api_hash = Some(value);
                        self.info("credentials saved");
                        self.ask(Prompt::Phone, "Phone number (+8801...)");
                    }
                    Err(e) => self.error(e.to_string()),
                }
            }
            Prompt::Phone => self.request_code(value),
            Prompt::Code => self.submit_code(value),
            Prompt::Password => self.submit_password(value),

            Prompt::SendTarget => {
                if value.is_empty() {
                    return;
                }
                self.stash_text(String::new());
                self.resolve(Purpose::Send, value);
            }
            Prompt::SendBody => {
                let Some(peer) = self.take_pending_peer() else {
                    return self.error("target was lost, try again");
                };
                if !value.is_empty() {
                    self.send_to(peer, value);
                }
            }
            Prompt::Note => {
                if !value.is_empty() {
                    self.send_note(value);
                }
            }
            Prompt::SendFileTarget => {
                if !value.is_empty() {
                    self.resolve(Purpose::SendFile, value);
                }
            }
            Prompt::SendFilePath => {
                let Some(peer) = self.take_pending_peer() else {
                    return self.error("target was lost, try again");
                };
                if value.is_empty() {
                    return;
                }
                if !std::path::Path::new(&value).is_file() {
                    return self.error(format!("no such file: {}", value));
                }
                self.send_file_to(peer, value);
            }
            Prompt::DownloadLink => {
                if !value.is_empty() {
                    self.download_link(value);
                }
            }
            Prompt::JoinTarget => {
                if !value.is_empty() {
                    self.resolve(Purpose::Join, value);
                }
            }
            Prompt::GlobalSearch => {
                if !value.is_empty() {
                    self.search_global(value);
                }
            }
            Prompt::ChatSearch => {
                if !value.is_empty() {
                    self.search_chat(value);
                }
            }
            Prompt::MembersTarget => {
                if !value.is_empty() {
                    self.resolve(Purpose::Members, value);
                }
            }
            Prompt::ExportChatTarget => {
                if !value.is_empty() {
                    self.resolve(Purpose::ExportChat, value);
                }
            }
            Prompt::ExportMembersTarget => {
                if !value.is_empty() {
                    self.resolve(Purpose::ExportMembers, value);
                }
            }
            Prompt::Limit => {
                let (Some(purpose), Some(peer)) = (self.pending_purpose(), self.take_pending_peer())
                else {
                    return self.error("target was lost, try again");
                };
                let fallback = if purpose == Purpose::ExportChat {
                    DEFAULT_EXPORT_LIMIT
                } else {
                    DEFAULT_MEMBER_LIMIT
                };
                let limit = value.trim().parse().unwrap_or(fallback).max(1);
                match purpose {
                    Purpose::ExportChat => self.export_chat(peer, limit),
                    Purpose::ExportMembers => self.export_members(peer, limit),
                    _ => self.load_members(peer, limit),
                }
            }

            // Destructive actions re-read the open chat and the selected message at
            // confirmation time, so they can never land on a different target.
            Prompt::DeleteMessage => {
                if value != "DELETE" {
                    return self.info("delete cancelled");
                }
                let (Some(peer), Some(msg)) =
                    (self.open_peer.clone(), self.selected_message().cloned())
                else {
                    return self.error("nothing selected");
                };
                self.delete_selected(peer, msg.id);
            }
            Prompt::EditMessage => {
                if value.is_empty() {
                    return self.info("edit cancelled");
                }
                let (Some(peer), Some(msg)) =
                    (self.open_peer.clone(), self.selected_message().cloned())
                else {
                    return self.error("nothing selected");
                };
                if !msg.outgoing {
                    return self.error("you can only edit your own messages");
                }
                self.edit_selected(peer, msg.id, value);
            }
            Prompt::LeaveConfirm => {
                let peer = self.take_pending_peer();
                if value != "LEAVE" {
                    return self.info("leave cancelled");
                }
                match peer {
                    Some(peer) => {
                        self.close_overlay();
                        self.leave(peer);
                    }
                    None => self.error("target was lost, try again"),
                }
            }
            Prompt::DeleteSession => {
                let Some(name) = self.sessions.get(self.overlay_sel).cloned() else {
                    return;
                };
                if value != "DELETE" {
                    return self.info("delete cancelled");
                }
                if self.tg.session.as_deref() == Some(name.as_str()) {
                    self.tg.disconnect();
                    self.me = None;
                    self.dialogs.clear();
                    self.messages.clear();
                    self.open_peer = None;
                }
                self.cfg.remove_session(&name);
                self.sessions = self.cfg.list_sessions();
                self.overlay_sel = 0;
                self.info(format!("removed session {}", name));
            }

            Prompt::ProfileFirst => {
                if !value.is_empty() {
                    self.update_profile(Some(value), None, None);
                }
            }
            Prompt::ProfileLast => self.update_profile(None, Some(value), None),
            Prompt::ProfileBio => {
                if value.chars().count() > 70 {
                    return self.error("bio must be 70 characters or fewer");
                }
                self.update_profile(None, None, Some(value));
            }
            Prompt::ProfileUsername => {
                if !value.is_empty() {
                    self.update_username(value);
                }
            }
            Prompt::ProfilePhoto => {
                if value.is_empty() {
                    return;
                }
                if !std::path::Path::new(&value).is_file() {
                    return self.error(format!("no such file: {}", value));
                }
                self.set_profile_photo(value);
            }

            Prompt::ReportTarget => {
                if !value.is_empty() {
                    self.resolve(Purpose::Report, value);
                }
            }
            Prompt::ReportDetail => {
                let Some(peer) = self.take_pending_peer() else {
                    return self.error("target was lost, try again");
                };
                let reason = REPORT_REASONS[self.report_reason_sel().min(REPORT_REASONS.len() - 1)].0;
                self.close_overlay();
                self.report(peer, reason.to_string(), value);
            }
        }
    }

    /// Logging in needs a connected client first, so connect the session for this
    /// phone number and let `on_connected` fire the code request.
    fn request_code(&mut self, phone: String) {
        if self.cfg.api_id.is_none() || self.cfg.api_hash.is_none() {
            return self.error("run setup first");
        }
        if phone.is_empty() {
            return self.error("phone number cannot be empty");
        }
        let name = session_name(&phone);
        *self.login_phone_mut() = phone;
        self.connect(name);
    }

    /// Ask Telegram to send a login code on the already-connected client.
    pub(crate) fn send_code_request(&mut self) {
        let (Some(client), Some(api_hash)) = (self.client(), self.cfg.api_hash.clone()) else {
            return self.error("not connected");
        };
        let phone = self.login_phone_mut().clone();
        if phone.is_empty() {
            return;
        }
        self.spawn("login", async move {
            match tg::request_code(&client, &phone, &api_hash).await {
                Ok(token) => Outcome::LoginCode(Ok(token)),
                Err(e) => Outcome::LoginCode(Err(e.to_string())),
            }
        });
    }

    fn submit_code(&mut self, code: String) {
        let Some(token) = self.take_login_token() else {
            return self.error("login expired, start again");
        };
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("login", async move {
            match tg::submit_code(&client, &token, &code).await {
                Ok(v) => Outcome::CodeDone(Box::new(Ok(v))),
                Err(e) => Outcome::CodeDone(Box::new(Err(e.to_string()))),
            }
        });
    }

    fn submit_password(&mut self, password: String) {
        let Some(token) = self.take_password_token() else {
            return self.error("login expired, start again");
        };
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("login", async move {
            match tg::submit_password(&client, token, &password).await {
                Ok(me) => Outcome::PasswordDone(Ok(me)),
                Err(e) => Outcome::PasswordDone(Err(e.to_string())),
            }
        });
    }
}

/// Session file name for a phone number: digits only, so it is filesystem safe.
pub fn session_name(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        "account".to_string()
    } else {
        digits
    }
}

#[cfg(test)]
mod tests {
    use super::session_name;

    #[test]
    fn session_names_keep_digits_only() {
        assert_eq!(session_name("+880 171-234 5678"), "8801712345678");
        assert_eq!(session_name("no digits here"), "account");
    }
}
