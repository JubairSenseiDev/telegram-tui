//! Async actions: every Telegram call the UI can start, and where its result lands.

use grammers_client::peer::Peer;

use crate::app::{App, Outcome, Overlay, Purpose};
use crate::tg;

const DIALOG_LIMIT: usize = 500;
const MSG_PAGE: usize = 60;
const SEARCH_LIMIT: usize = 100;
pub const DEFAULT_MEMBER_LIMIT: usize = 500;
pub const DEFAULT_EXPORT_LIMIT: usize = 5000;

fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

impl App {
    pub fn load_dialogs(&mut self) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("chats", async move {
            match tg::fetch_dialogs(&client, DIALOG_LIMIT).await {
                Ok(list) => Outcome::Dialogs(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn open_chat(&mut self, index: usize) {
        let Some(dialog) = self.dialogs.get(index).cloned() else {
            return;
        };
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.dialog_sel = index;
        self.open_peer = Some(dialog.peer.clone());
        self.open_title = dialog.name.clone();
        self.messages.clear();
        self.msg_sel = 0;
        self.reply_to = None;
        let peer = dialog.peer;
        self.spawn("messages", async move {
            match tg::fetch_messages(&client, &peer, MSG_PAGE).await {
                Ok(list) => Outcome::Messages(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn refresh_chat(&mut self) {
        let Some(open) = self.open_peer.as_ref().map(|p| p.id()) else {
            return;
        };
        if let Some(i) = self.dialogs.iter().position(|d| d.peer.id() == open) {
            self.open_chat(i);
        }
    }

    pub fn load_older(&mut self) {
        let (Some(client), Some(peer)) = (self.client(), self.open_peer.clone()) else {
            return;
        };
        let Some(oldest) = self.messages.first().map(|m| m.id) else {
            return;
        };
        self.spawn("history", async move {
            match tg::fetch_messages_older(&client, &peer, oldest, MSG_PAGE).await {
                Ok(list) => Outcome::Older(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn send_current(&mut self, text: String) {
        let (Some(client), Some(peer)) = (self.client(), self.open_peer.clone()) else {
            return self.error("open a chat first");
        };
        let reply = self.reply_to.take();
        self.spawn("send", async move {
            let result = match reply {
                Some(id) => tg::reply_to(&client, &peer, id, &text).await,
                None => tg::send_text(&client, &peer, &text).await,
            };
            match result {
                Ok(()) => Outcome::Refresh,
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn send_to(&mut self, peer: Peer, text: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("send", async move {
            match tg::send_text(&client, &peer, &text).await {
                Ok(()) => Outcome::Toast("message sent".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn send_note(&mut self, text: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("note", async move {
            match tg::send_to_self(&client, &text).await {
                Ok(()) => Outcome::Toast("saved to Saved Messages".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn send_file_to(&mut self, peer: Peer, path: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("upload", async move {
            match tg::send_file(&client, &peer, &path, "").await {
                Ok(()) => Outcome::Toast("file sent".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    /// Resolve a username or link, then act on it according to `purpose`.
    pub fn resolve(&mut self, purpose: Purpose, target: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("resolve", async move {
            match tg::resolve_username(&client, &target).await {
                Ok(peer) => Outcome::Resolved(purpose, Box::new(peer)),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn join(&mut self, peer: Peer) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("join", async move {
            match tg::join_chat(&client, &peer).await {
                Ok(()) => Outcome::Toast("joined".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn delete_selected(&mut self, peer: Peer, id: i32) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("delete", async move {
            match tg::delete_message(&client, &peer, id).await {
                Ok(()) => Outcome::Refresh,
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn edit_selected(&mut self, peer: Peer, id: i32, text: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("edit", async move {
            match tg::edit_message_text(&client, &peer, id, &text).await {
                Ok(()) => Outcome::Refresh,
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn pin_selected(&mut self, unpin: bool) {
        let (Some(client), Some(peer), Some(msg)) =
            (self.client(), self.open_peer.clone(), self.selected_message().cloned())
        else {
            return self.error("select a message first");
        };
        self.spawn("pin", async move {
            match tg::set_pinned(&client, &peer, msg.id, unpin).await {
                Ok(()) => Outcome::Toast(if unpin { "unpinned" } else { "pinned" }.to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn forward_selected(&mut self, id: i32, to: Peer) {
        let (Some(client), Some(from)) = (self.client(), self.open_peer.clone()) else {
            return self.error("open a chat first");
        };
        self.spawn("forward", async move {
            match tg::forward(&client, &from, &[id], &to).await {
                Ok(0) => Outcome::Error("nothing was forwarded".to_string()),
                Ok(n) => Outcome::Toast(format!("forwarded {} message(s)", n)),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn mark_read(&mut self) {
        let (Some(client), Some(peer)) = (self.client(), self.open_peer.clone()) else {
            return;
        };
        self.spawn("read", async move {
            match tg::mark_read(&client, &peer).await {
                Ok(()) => Outcome::Toast("marked as read".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn download_selected(&mut self) {
        let (Some(client), Some(peer), Some(msg)) =
            (self.client(), self.open_peer.clone(), self.selected_message().cloned())
        else {
            return self.error("select a message first");
        };
        if msg.media.is_none() {
            return self.error("that message has no media");
        }
        let dir = self.cfg.downloads_dir();
        self.spawn("download", async move {
            match tg::download_selected_media(&client, &peer, msg.id, &dir).await {
                Ok(desc) => Outcome::Toast(desc),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn download_link(&mut self, link: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        let dir = self.cfg.downloads_dir();
        self.spawn("download", async move {
            match tg::download_from_link(&client, &link, &dir).await {
                Ok(desc) => Outcome::Toast(desc),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn search_global(&mut self, query: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("search", async move {
            match tg::search_messages(&client, &query, SEARCH_LIMIT).await {
                Ok(list) => Outcome::Found(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn search_chat(&mut self, query: String) {
        let (Some(client), Some(peer)) = (self.client(), self.open_peer.clone()) else {
            return self.error("open a chat first");
        };
        self.spawn("search", async move {
            match tg::search_in_chat(&client, &peer, &query, SEARCH_LIMIT).await {
                Ok(list) => Outcome::Found(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn load_members(&mut self, peer: Peer, limit: usize) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("members", async move {
            match tg::fetch_members(&client, &peer, limit).await {
                Ok(list) => Outcome::Members(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn export_dialogs(&mut self) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        let path = self.cfg.exports_dir().join(format!("dialogs-{}.csv", stamp()));
        let shown = path.clone();
        self.spawn("export", async move {
            match tg::export_dialogs(&client, &path).await {
                Ok(n) => Outcome::Toast(format!("exported {} chats -> {}", n, shown.display())),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn export_members(&mut self, peer: Peer, limit: usize) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        let path = self.cfg.exports_dir().join(format!("members-{}.csv", stamp()));
        let shown = path.clone();
        self.spawn("export", async move {
            match tg::export_members(&client, &peer, &path, limit).await {
                Ok(n) => Outcome::Toast(format!("exported {} members -> {}", n, shown.display())),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn export_chat(&mut self, peer: Peer, limit: usize) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        let path = self.cfg.exports_dir().join(format!("chat-{}.txt", stamp()));
        let shown = path.clone();
        self.spawn("export", async move {
            match tg::export_chat(&client, &peer, &path, limit).await {
                Ok(n) => Outcome::Toast(format!("exported {} messages -> {}", n, shown.display())),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn update_profile(
        &mut self,
        first: Option<String>,
        last: Option<String>,
        bio: Option<String>,
    ) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("profile", async move {
            match tg::update_profile(&client, first.as_deref(), last.as_deref(), bio.as_deref())
                .await
            {
                Ok(()) => Outcome::Toast("profile updated".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn update_username(&mut self, username: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("profile", async move {
            match tg::update_username(&client, &username).await {
                Ok(()) => Outcome::Toast("username updated".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn set_profile_photo(&mut self, path: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("profile", async move {
            match tg::set_profile_photo(&client, &path).await {
                Ok(()) => Outcome::Toast("profile photo updated".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn report(&mut self, peer: Peer, reason: String, detail: String) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("report", async move {
            match tg::report_peer(&client, &peer, &reason, &detail).await {
                Ok(()) => Outcome::Toast("report submitted".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn load_leavable(&mut self) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.overlay = Overlay::Leave;
        self.overlay_sel = 0;
        self.spawn("groups", async move {
            match tg::leavable_chats(&client, DIALOG_LIMIT).await {
                Ok(list) => Outcome::Leavable(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn leave(&mut self, peer: Peer) {
        let Some(client) = self.client() else {
            return self.error("not connected");
        };
        self.spawn("leave", async move {
            match tg::leave_chat(&client, &peer).await {
                Ok(()) => Outcome::Toast("left the chat".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    pub fn load_statuses(&mut self) {
        let Some(api_id) = self.cfg.api_id else {
            return self.error("run setup first");
        };
        let paths: Vec<(String, std::path::PathBuf)> = self
            .sessions
            .clone()
            .into_iter()
            .map(|name| {
                let path = self.cfg.session_path(&name);
                (name, path)
            })
            .collect();
        self.overlay = Overlay::Status;
        self.overlay_sel = 0;
        self.spawn("accounts", async move {
            Outcome::Statuses(tg::account_status(paths, api_id).await)
        });
    }

    pub fn connect(&mut self, name: String) {
        let Some(api_id) = self.cfg.api_id else {
            return self.error("run setup first");
        };
        let path = self.cfg.session_path(&name);
        self.spawn("connect", async move {
            match tg::connect_session(&path, api_id).await {
                Ok(sess) => Outcome::Connected(Box::new(sess), name),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }
}
