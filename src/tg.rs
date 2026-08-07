use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::message::Message;
use grammers_client::peer::{Peer, User};
use grammers_client::{Client, SignInError};
use grammers_mtsender::{SenderPool, SenderPoolFatHandle};
use grammers_session::storages::SqliteSession;
use grammers_session::types::PeerRef;
use grammers_tl_types as tl;
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub struct Me {
    pub id: i64,
    pub name: String,
    pub username: Option<String>,
    pub phone: Option<String>,
}

impl Me {
    pub fn from_user(user: &User) -> Self {
        let first = user.first_name().unwrap_or("").to_string();
        let last = user.last_name().unwrap_or("").to_string();
        let name = if last.is_empty() {
            first
        } else {
            format!("{} {}", first, last)
        };
        Me {
            id: user.id().bot_api_dialog_id().unwrap_or_default(),
            name,
            username: user.username().map(|s| s.to_string()),
            phone: user.phone().map(|s| s.to_string()),
        }
    }
}

pub struct ConnectedSession {
    pub client: Client,
    pub handle: SenderPoolFatHandle,
    pub runner: JoinHandle<()>,
    pub me: Option<Me>,
    pub authorized: bool,
}

pub struct Tg {
    pub client: Option<Client>,
    pool_handle: Option<SenderPoolFatHandle>,
    runner: Option<JoinHandle<()>>,
    pub session: Option<String>,
    pub me: Option<Me>,
}

impl Tg {
    pub fn new() -> Self {
        Self {
            client: None,
            pool_handle: None,
            runner: None,
            session: None,
            me: None,
        }
    }

    pub fn set_parts(&mut self, sess: ConnectedSession) {
        self.disconnect();
        self.client = Some(sess.client);
        self.pool_handle = Some(sess.handle);
        self.runner = Some(sess.runner);
        self.me = sess.me;
    }

    pub fn disconnect(&mut self) {
        if let Some(handle) = self.pool_handle.take() {
            handle.quit();
        }
        self.client = None;
        self.runner = None;
        self.session = None;
        self.me = None;
    }
}

pub async fn connect_session(path: &Path, api_id: i32) -> Result<ConnectedSession> {
    let raw = Arc::new(SqliteSession::open(path).await?);
    let SenderPool { runner, handle, .. } = SenderPool::new(Arc::clone(&raw), api_id);
    let client = Client::new(handle.clone());
    let runner_handle = tokio::spawn(runner.run());
    let authorized = client.is_authorized().await?;
    let me = if authorized {
        Some(Me::from_user(&client.get_me().await?))
    } else {
        None
    };
    Ok(ConnectedSession {
        client,
        handle,
        runner: runner_handle,
        me,
        authorized,
    })
}

pub async fn request_code(client: &Client, phone: &str, api_hash: &str) -> Result<LoginToken> {
    Ok(client.request_login_code(phone, api_hash).await?)
}

pub async fn submit_code(
    client: &Client,
    token: &LoginToken,
    code: &str,
) -> Result<Result<Me, PasswordToken>> {
    match client.sign_in(token, code).await {
        Ok(user) => Ok(Ok(Me::from_user(&user))),
        Err(SignInError::PasswordRequired(token)) => Ok(Err(token)),
        Err(e) => Err(anyhow!("{}", e)),
    }
}

pub async fn submit_password(client: &Client, token: PasswordToken, password: &str) -> Result<Me> {
    let user = client.check_password(token, password).await?;
    Ok(Me::from_user(&user))
}

#[derive(Clone)]
pub struct DialogItem {
    pub name: String,
    pub peer: Peer,
    pub unread: i32,
    pub last: String,
    pub kind: String,
}

#[derive(Clone)]
pub struct MsgItem {
    pub id: i32,
    pub sender: String,
    pub time: String,
    pub text: String,
    pub outgoing: bool,
}

impl MsgItem {
    pub fn from_message(msg: &Message) -> Self {
        let sender = if msg.outgoing() {
            "Me".to_string()
        } else {
            msg.sender()
                .and_then(|p| p.name())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        };
        let text = if msg.text().is_empty() {
            "[media/action]".to_string()
        } else {
            msg.text().to_string()
        };
        MsgItem {
            id: msg.id(),
            sender,
            time: msg.date().format("%H:%M").to_string(),
            text,
            outgoing: msg.outgoing(),
        }
    }
}

pub async fn fetch_dialogs(client: &Client) -> Result<Vec<DialogItem>> {
    let mut iter = client.iter_dialogs();
    let mut out = Vec::new();
    while let Some(dialog) = iter.next().await? {
        let peer = dialog.peer().clone();
        let kind = match &peer {
            Peer::User(_) => "user",
            Peer::Group(_) => "group",
            Peer::Channel(_) => "channel",
        }
        .to_string();
        let unread = match &dialog.raw {
            tl::enums::Dialog::Dialog(d) => d.unread_count,
            _ => 0,
        };
        let last = dialog
            .last_message
            .as_ref()
            .map(|m| {
                let t = m.text();
                if t.is_empty() {
                    "[media/action]".to_string()
                } else {
                    t.replace('\n', " ")
                }
            })
            .unwrap_or_else(|| "[no messages]".to_string());
        let name = peer.name().unwrap_or("Unknown").to_string();
        out.push(DialogItem {
            name,
            peer,
            unread,
            last,
            kind,
        });
    }
    Ok(out)
}

pub async fn fetch_messages(client: &Client, peer: &Peer, limit: usize) -> Result<Vec<MsgItem>> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_messages(pref);
    let mut out = Vec::new();
    while let Some(msg) = iter.next().await? {
        out.push(MsgItem::from_message(&msg));
        if out.len() >= limit {
            break;
        }
    }
    out.reverse();
    Ok(out)
}

pub async fn fetch_messages_older(
    client: &Client,
    peer: &Peer,
    offset_id: i32,
    limit: usize,
) -> Result<Vec<MsgItem>> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_messages(pref).offset_id(offset_id).limit(limit);
    let mut out = Vec::new();
    while let Some(msg) = iter.next().await? {
        out.push(MsgItem::from_message(&msg));
        if out.len() >= limit {
            break;
        }
    }
    out.reverse();
    Ok(out)
}

pub async fn mark_read(client: &Client, peer: &Peer) -> Result<()> {
    let pref = peer_ref(peer).await?;
    client.mark_as_read(pref).await?;
    Ok(())
}

pub async fn search_in_chat(
    client: &Client,
    peer: &Peer,
    query: &str,
    limit: usize,
) -> Result<Vec<MsgItem>> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.search_messages(pref).query(query);
    let mut out = Vec::new();
    while let Some(msg) = iter.next().await? {
        out.push(MsgItem::from_message(&msg));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

pub async fn delete_message(client: &Client, peer: &Peer, id: i32) -> Result<()> {
    let pref = peer_ref(peer).await?;
    client.delete_messages(pref, &[id]).await?;
    Ok(())
}

pub async fn edit_message_text(client: &Client, peer: &Peer, id: i32, text: &str) -> Result<()> {
    let pref = peer_ref(peer).await?;
    client.edit_message(pref, id, text).await?;
    Ok(())
}

pub async fn set_pinned(client: &Client, peer: &Peer, id: i32, unpin: bool) -> Result<()> {
    let pref = peer_ref(peer).await?;
    if unpin {
        client.unpin_message(pref, id).await?;
    } else {
        client.pin_message(pref, id).await?;
    }
    Ok(())
}

pub async fn send_file(client: &Client, peer: &Peer, path: &str, caption: &str) -> Result<()> {
    use grammers_client::message::InputMessage;
    let pref = peer_ref(peer).await?;
    let uploaded = client.upload_file(path).await?;
    let msg = if caption.is_empty() {
        InputMessage::new().document(uploaded)
    } else {
        InputMessage::new().document(uploaded).text(caption)
    };
    client.send_message(pref, msg).await?;
    Ok(())
}

pub async fn search_messages(client: &Client, query: &str, limit: usize) -> Result<Vec<MsgItem>> {
    let mut iter = client.search_all_messages().query(query);
    let mut out = Vec::new();
    while let Some(msg) = iter.next().await? {
        out.push(MsgItem::from_message(&msg));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

async fn peer_ref(peer: &Peer) -> Result<PeerRef> {
    peer.to_ref()
        .await
        .map_err(|e| anyhow!(e.to_string()))?
        .ok_or_else(|| anyhow!("peer not fully loaded"))
}

pub async fn send_text(client: &Client, peer: &Peer, text: &str) -> Result<()> {
    let pref = peer_ref(peer).await?;
    client.send_message(pref, text).await?;
    Ok(())
}

pub async fn send_to_self(client: &Client, text: &str) -> Result<()> {
    client.send_message(&tl::types::InputPeerSelf {}, text).await?;
    Ok(())
}

pub async fn resolve_username(client: &Client, input: &str) -> Result<Peer> {
    let cleaned = input
        .trim()
        .trim_start_matches('@')
        .trim_start_matches("https://t.me/")
        .trim_start_matches("http://t.me/")
        .trim_end_matches('/')
        .to_string();
    client
        .resolve_username(&cleaned)
        .await?
        .ok_or_else(|| anyhow!("could not find @{}", cleaned))
}

pub async fn join_chat(client: &Client, peer: &Peer) -> Result<()> {
    let pref = peer_ref(peer).await?;
    client.join_chat(pref).await?;
    Ok(())
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn export_dialogs(client: &Client, path: &Path) -> Result<usize> {
    let mut iter = client.iter_dialogs();
    let mut out = String::from("id,name,type,unread,last\n");
    let mut count = 0usize;
    while let Some(dialog) = iter.next().await? {
        count += 1;
        let peer = dialog.peer();
        let kind = match peer {
            Peer::User(_) => "user",
            Peer::Group(_) => "group",
            Peer::Channel(_) => "channel",
        };
        let last = dialog.last_message.as_ref().map(|m| m.text()).unwrap_or("");
        let unread = match &dialog.raw {
            tl::enums::Dialog::Dialog(d) => d.unread_count,
            _ => 0,
        };
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            peer.id().bot_api_dialog_id().unwrap_or_default(),
            csv_field(peer.name().unwrap_or("Unknown")),
            kind,
            unread,
            csv_field(&last.replace('\n', " ")),
        ));
    }
    std::fs::write(path, out)?;
    Ok(count)
}

#[derive(Clone)]
pub struct MemberItem {
    pub id: i64,
    pub username: Option<String>,
    pub first: String,
    pub last: String,
    pub phone: Option<String>,
}

pub async fn fetch_members(client: &Client, peer: &Peer) -> Result<Vec<MemberItem>> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_participants(pref);
    let mut out = Vec::new();
    while let Some(participant) = iter.next().await? {
        let user = &participant.user;
        out.push(MemberItem {
            id: user.id().bot_api_dialog_id().unwrap_or_default(),
            username: user.username().map(|s| s.to_string()),
            first: user.first_name().unwrap_or("").to_string(),
            last: user.last_name().unwrap_or("").to_string(),
            phone: user.phone().map(|s| s.to_string()),
        });
    }
    Ok(out)
}

pub async fn export_members(client: &Client, peer: &Peer, path: &Path) -> Result<usize> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_participants(pref);
    let mut out = String::from("user_id,username,first_name,last_name,phone\n");
    let mut count = 0usize;
    while let Some(participant) = iter.next().await? {
        count += 1;
        let user = &participant.user;
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            user.id().bot_api_dialog_id().unwrap_or_default(),
            csv_field(user.username().unwrap_or("")),
            csv_field(user.first_name().unwrap_or("")),
            csv_field(user.last_name().unwrap_or("")),
            csv_field(user.phone().unwrap_or("")),
        ));
    }
    std::fs::write(path, out)?;
    Ok(count)
}

pub async fn export_chat(
    client: &Client,
    peer: &Peer,
    path: &Path,
    limit: usize,
) -> Result<usize> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_messages(pref);
    let mut out = String::new();
    let mut count = 0usize;
    while let Some(msg) = iter.next().await? {
        if count >= limit {
            break;
        }
        let sender = if msg.outgoing() {
            "Me".to_string()
        } else {
            msg.sender()
                .and_then(|p| p.name())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        };
        out.push_str(&format!(
            "[{}] {}: {}\n",
            msg.date().format("%Y-%m-%d %H:%M"),
            sender,
            msg.text()
        ));
        count += 1;
    }
    std::fs::write(path, out)?;
    Ok(count)
}
