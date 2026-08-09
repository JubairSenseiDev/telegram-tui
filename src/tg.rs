use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::media::Media;
use grammers_client::message::Message;
use grammers_client::peer::{Peer, User};
use grammers_client::{Client, SignInError};
use grammers_mtsender::{InvocationError, SenderPool, SenderPoolFatHandle};
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

/// Longest FLOOD_WAIT we sit through instead of reporting back to the user.
const MAX_FLOOD_WAIT: u32 = 60;

/// Seconds Telegram wants us to wait, when the error is a FLOOD_WAIT.
pub fn flood_wait_secs(err: &InvocationError) -> Option<u32> {
    match err {
        InvocationError::Rpc(rpc) if rpc.name == "FLOOD_WAIT" => rpc.value,
        _ => None,
    }
}

/// Wait out a short FLOOD_WAIT so long operations survive rate limits.
/// Returns false when the wait is too long to absorb silently.
async fn absorb_flood(err: &InvocationError) -> bool {
    match flood_wait_secs(err) {
        Some(secs) if secs <= MAX_FLOOD_WAIT => {
            tokio::time::sleep(Duration::from_secs(secs as u64 + 1)).await;
            true
        }
        _ => false,
    }
}

fn explain(err: InvocationError) -> anyhow::Error {
    match flood_wait_secs(&err) {
        Some(secs) => anyhow!("rate limited by Telegram, retry in {}s", secs),
        None => anyhow!("{}", err),
    }
}

/// Run a call, retrying once after a short FLOOD_WAIT.
pub async fn retry_flood<T, F, Fut>(mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, InvocationError>>,
{
    match op().await {
        Ok(v) => Ok(v),
        Err(e) => {
            if absorb_flood(&e).await {
                op().await.map_err(explain)
            } else {
                Err(explain(e))
            }
        }
    }
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
    pub reply_to: Option<i32>,
    pub media: Option<String>,
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
        let media = msg_media_info(msg).map(|i| format!("{} · {}", i.kind, human_size(i.size)));
        let text = if msg.text().is_empty() {
            media
                .clone()
                .map(|m| format!("[{}]", m))
                .unwrap_or_else(|| "[no text]".to_string())
        } else {
            msg.text().to_string()
        };
        MsgItem {
            id: msg.id(),
            sender,
            time: msg.date().format("%H:%M").to_string(),
            text,
            outgoing: msg.outgoing(),
            reply_to: msg.reply_to_message_id(),
            media,
        }
    }
}

/// Advance an iterator, sitting through a short FLOOD_WAIT rather than aborting.
macro_rules! next_tolerant {
    ($iter:expr) => {{
        match $iter.next().await {
            Ok(v) => v,
            Err(e) => {
                if absorb_flood(&e).await {
                    $iter.next().await.map_err(explain)?
                } else {
                    return Err(explain(e));
                }
            }
        }
    }};
}

pub async fn fetch_dialogs(client: &Client, limit: usize) -> Result<Vec<DialogItem>> {
    let mut iter = client.iter_dialogs();
    let mut out = Vec::new();
    while let Some(dialog) = next_tolerant!(iter) {
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
        if out.len() >= limit {
            break;
        }
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

pub async fn reply_to(client: &Client, peer: &Peer, msg_id: i32, text: &str) -> Result<()> {
    use grammers_client::message::InputMessage;
    let pref = peer_ref(peer).await?;
    let msg = InputMessage::new().text(text).reply_to(Some(msg_id));
    retry_flood(|| client.send_message(pref, msg.clone())).await?;
    Ok(())
}

pub async fn forward(client: &Client, from: &Peer, ids: &[i32], to: &Peer) -> Result<usize> {
    let src = peer_ref(from).await?;
    let dst = peer_ref(to).await?;
    let sent = retry_flood(|| client.forward_messages(dst, ids, src)).await?;
    Ok(sent.into_iter().flatten().count())
}

/// Update first/last name and bio. Empty fields are left untouched.
pub async fn update_profile(
    client: &Client,
    first: Option<&str>,
    last: Option<&str>,
    bio: Option<&str>,
) -> Result<()> {
    let req = tl::functions::account::UpdateProfile {
        first_name: first.map(|s| s.to_string()),
        last_name: last.map(|s| s.to_string()),
        about: bio.map(|s| s.to_string()),
    };
    retry_flood(|| client.invoke(&req)).await?;
    Ok(())
}

pub async fn update_username(client: &Client, username: &str) -> Result<()> {
    let req = tl::functions::account::UpdateUsername {
        username: username.trim().trim_start_matches('@').to_string(),
    };
    retry_flood(|| client.invoke(&req)).await?;
    Ok(())
}

pub async fn set_profile_photo(client: &Client, path: &str) -> Result<()> {
    let uploaded = client.upload_file(path).await?;
    let req = tl::functions::photos::UploadProfilePhoto {
        fallback: false,
        bot: None,
        file: Some(uploaded.raw),
        video: None,
        video_start_ts: None,
        video_emoji_markup: None,
    };
    retry_flood(|| client.invoke(&req)).await?;
    Ok(())
}

pub const REPORT_REASONS: &[(&str, &str)] = &[
    ("spam", "Spam"),
    ("violence", "Violence"),
    ("porn", "Pornography"),
    ("child", "Child abuse"),
    ("copyright", "Copyright"),
    ("fake", "Fake account"),
    ("drugs", "Illegal drugs"),
    ("other", "Other"),
];

fn report_reason(key: &str) -> tl::enums::ReportReason {
    use tl::types::*;
    match key {
        "violence" => InputReportReasonViolence {}.into(),
        "porn" => InputReportReasonPornography {}.into(),
        "child" => InputReportReasonChildAbuse {}.into(),
        "copyright" => InputReportReasonCopyright {}.into(),
        "fake" => InputReportReasonFake {}.into(),
        "drugs" => InputReportReasonIllegalDrugs {}.into(),
        "other" => InputReportReasonOther {}.into(),
        _ => InputReportReasonSpam {}.into(),
    }
}

pub async fn report_peer(client: &Client, peer: &Peer, reason: &str, detail: &str) -> Result<()> {
    let pref = peer_ref(peer).await?;
    let req = tl::functions::account::ReportPeer {
        peer: pref.into(),
        reason: report_reason(reason),
        message: detail.to_string(),
    };
    retry_flood(|| client.invoke(&req)).await?;
    Ok(())
}

/// Leave a group or channel. Peers you created are refused, matching the
/// Python tool's behaviour of never abandoning a chat you own.
pub async fn leave_chat(client: &Client, peer: &Peer) -> Result<()> {
    if let Peer::Channel(ch) = peer {
        if ch.raw.creator {
            return Err(anyhow!("you created this chat, leaving it is refused"));
        }
    }
    let pref = peer_ref(peer).await?;
    let req = tl::functions::channels::LeaveChannel {
        channel: pref.into(),
    };
    retry_flood(|| client.invoke(&req)).await?;
    Ok(())
}

/// Groups and channels the user is in but did not create.
pub async fn leavable_chats(client: &Client, limit: usize) -> Result<Vec<DialogItem>> {
    Ok(fetch_dialogs(client, limit)
        .await?
        .into_iter()
        .filter(|d| match &d.peer {
            Peer::Channel(ch) => !ch.raw.creator,
            Peer::Group(_) => true,
            Peer::User(_) => false,
        })
        .collect())
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

#[derive(Clone)]
pub struct DownloadInfo {
    pub filename: String,
    pub kind: &'static str,
    pub size: u64,
}

pub fn msg_media_info(msg: &Message) -> Option<DownloadInfo> {
    let media = msg.media()?;
    match media {
        Media::Document(doc) => {
            let mime = doc.mime_type().unwrap_or("");
            let kind = if mime.starts_with("video/") {
                "video"
            } else if mime.starts_with("audio/") {
                "audio"
            } else if mime.starts_with("image/") {
                "image"
            } else {
                "file"
            };
            let base = doc
                .name()
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("file-{}", doc.id()));
            Some(DownloadInfo {
                filename: sanitize_filename(&format!("{}{}", base, ext_for_mime(mime))),
                kind,
                size: doc.size().unwrap_or(0) as u64,
            })
        }
        Media::Photo(photo) => Some(DownloadInfo {
            filename: format!("photo-{}.jpg", photo.id()),
            kind: "photo",
            size: photo.size().unwrap_or(0) as u64,
        }),
        _ => None,
    }
}

pub async fn download_selected_media(
    client: &Client,
    peer: &Peer,
    msg_id: i32,
    dir: &Path,
) -> Result<String> {
    let pref = peer_ref(peer).await?;
    let msgs = client.get_messages_by_id(pref, &[msg_id]).await?;
    let msg = msgs
        .into_iter()
        .flatten()
        .next()
        .ok_or_else(|| anyhow!("message not found"))?;
    let info = msg_media_info(&msg).ok_or_else(|| anyhow!("no downloadable media in this message"))?;
    let path = unique_path(dir, &info.filename);
    let done = msg.download_media(&path).await?;
    if !done {
        return Err(anyhow!("no media content to download"));
    }
    let size = std::fs::metadata(&path)
        .map(|m| m.len())
        .unwrap_or(info.size);
    Ok(format!(
        "{} ({}) -> {}",
        info.kind,
        human_size(size),
        path.to_string_lossy()
    ))
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut n = bytes as f64;
    let mut i = 0;
    while n >= 1024.0 && i < UNITS.len() - 1 {
        n /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, UNITS[i])
    } else {
        format!("{:.1} {}", n, UNITS[i])
    }
}

enum LinkTarget {
    Username(String),
    ChannelId(i64),
}

fn parse_tme(raw: &str) -> Option<(LinkTarget, i32)> {
    let mut s = raw.trim();
    for host in [
        "https://t.me/",
        "http://t.me/",
        "https://telegram.me/",
        "http://telegram.me/",
        "t.me/",
        "telegram.me/",
    ] {
        if let Some(rest) = s.strip_prefix(host) {
            s = rest;
            break;
        }
    }
    let s = s.split(['?', '#']).next().unwrap_or(s);
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        ["c", id, msg] => Some((
            LinkTarget::ChannelId(id.parse().ok()?),
            msg.parse().ok()?,
        )),
        [user, msg] => Some((LinkTarget::Username(user.to_string()), msg.parse().ok()?)),
        _ => None,
    }
}

fn channel_peer(client: &Client, chan_id: i64) -> Peer {
    let channel = tl::types::Channel {
        creator: false,
        left: false,
        broadcast: true,
        verified: false,
        megagroup: false,
        restricted: false,
        signatures: false,
        min: false,
        scam: false,
        has_link: false,
        has_geo: false,
        slowmode_enabled: false,
        call_active: false,
        call_not_empty: false,
        fake: false,
        gigagroup: false,
        noforwards: false,
        join_to_send: false,
        join_request: false,
        forum: false,
        stories_hidden: false,
        stories_hidden_min: false,
        stories_unavailable: false,
        signature_profiles: false,
        autotranslation: false,
        broadcast_messages_allowed: false,
        monoforum: false,
        forum_tabs: false,
        id: chan_id,
        access_hash: None,
        title: String::new(),
        username: None,
        photo: tl::enums::ChatPhoto::Empty,
        date: 0,
        restriction_reason: None,
        admin_rights: None,
        banned_rights: None,
        default_banned_rights: None,
        participants_count: None,
        usernames: None,
        stories_max_id: None,
        color: None,
        profile_color: None,
        emoji_status: None,
        level: None,
        subscription_until_date: None,
        bot_verification_icon: None,
        send_paid_messages_stars: None,
        linked_monoforum_id: None,
    };
    Peer::from_raw(client, tl::enums::Chat::Channel(channel))
}

pub async fn download_from_link(client: &Client, raw: &str, dir: &Path) -> Result<String> {
    let (target, msg_id) = parse_tme(raw).ok_or_else(|| anyhow!("unsupported link format"))?;
    let peer = match target {
        LinkTarget::Username(u) => resolve_username(client, &u).await?,
        LinkTarget::ChannelId(id) => channel_peer(client, id),
    };
    download_selected_media(client, &peer, msg_id, dir).await
}

fn ext_for_mime(mime: &str) -> &'static str {
    if mime.starts_with("video/") {
        ".mp4"
    } else if mime == "audio/mpeg" {
        ".mp3"
    } else if mime.starts_with("audio/") {
        ".m4a"
    } else if mime == "image/jpeg" {
        ".jpg"
    } else if mime == "image/png" {
        ".png"
    } else if mime == "image/gif" {
        ".gif"
    } else if mime == "image/webp" {
        ".webp"
    } else if mime == "application/pdf" {
        ".pdf"
    } else if mime.starts_with("text/") {
        ".txt"
    } else {
        ""
    }
}

fn sanitize_filename(name: &str) -> String {
    let out: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let out = out.trim().trim_start_matches('.').to_string();
    if out.is_empty() || out == ".." {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("file-{}", secs)
    } else {
        out
    }
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let p = dir.join(filename);
    if !p.exists() {
        return p;
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    for i in 1..10000u32 {
        let cand = dir.join(format!("{}-{}{}", stem, i, ext));
        if !cand.exists() {
            return cand;
        }
    }
    p
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
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    writeln!(out, "id,name,type,unread,last")?;
    let mut count = 0usize;
    while let Some(dialog) = next_tolerant!(iter) {
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
        writeln!(
            out,
            "{},{},{},{},{}",
            peer.id().bot_api_dialog_id().unwrap_or_default(),
            csv_field(peer.name().unwrap_or("Unknown")),
            kind,
            unread,
            csv_field(&last.replace('\n', " ")),
        )?;
    }
    out.flush()?;
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

/// Telegram throttles participant walks hard, so pace them like the Python tool did.
const MEMBER_THROTTLE: Duration = Duration::from_millis(50);

pub async fn fetch_members(client: &Client, peer: &Peer, limit: usize) -> Result<Vec<MemberItem>> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_participants(pref);
    let mut out = Vec::new();
    while let Some(participant) = next_tolerant!(iter) {
        let user = &participant.user;
        out.push(MemberItem {
            id: user.id().bot_api_dialog_id().unwrap_or_default(),
            username: user.username().map(|s| s.to_string()),
            first: user.first_name().unwrap_or("").to_string(),
            last: user.last_name().unwrap_or("").to_string(),
            phone: user.phone().map(|s| s.to_string()),
        });
        if out.len() >= limit {
            break;
        }
        tokio::time::sleep(MEMBER_THROTTLE).await;
    }
    Ok(out)
}

pub async fn export_members(
    client: &Client,
    peer: &Peer,
    path: &Path,
    limit: usize,
) -> Result<usize> {
    let pref = peer_ref(peer).await?;
    let mut iter = client.iter_participants(pref);
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    writeln!(out, "user_id,username,first_name,last_name,phone")?;
    let mut count = 0usize;
    while let Some(participant) = next_tolerant!(iter) {
        count += 1;
        let user = &participant.user;
        writeln!(
            out,
            "{},{},{},{},{}",
            user.id().bot_api_dialog_id().unwrap_or_default(),
            csv_field(user.username().unwrap_or("")),
            csv_field(user.first_name().unwrap_or("")),
            csv_field(user.last_name().unwrap_or("")),
            csv_field(user.phone().unwrap_or("")),
        )?;
        if count >= limit {
            break;
        }
        tokio::time::sleep(MEMBER_THROTTLE).await;
    }
    out.flush()?;
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
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    let mut count = 0usize;
    while let Some(msg) = next_tolerant!(iter) {
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
        writeln!(
            out,
            "[{}] {}: {}",
            msg.date().format("%Y-%m-%d %H:%M"),
            sender,
            escape_lines(msg.text())
        )?;
        count += 1;
    }
    out.flush()?;
    Ok(count)
}

/// Keep one message on one line so the export stays greppable.
fn escape_lines(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "")
}

#[derive(Clone)]
pub struct AccountStatus {
    pub session: String,
    pub me: Option<Me>,
    pub error: Option<String>,
}

/// Connect each stored session just long enough to read who it belongs to.
pub async fn account_status(paths: Vec<(String, PathBuf)>, api_id: i32) -> Vec<AccountStatus> {
    let mut out = Vec::new();
    for (session, path) in paths {
        match connect_session(&path, api_id).await {
            Ok(sess) => {
                let me = sess.me.clone();
                let authorized = sess.authorized;
                sess.handle.quit();
                sess.runner.abort();
                out.push(AccountStatus {
                    session,
                    me,
                    error: (!authorized).then(|| "not authorized".to_string()),
                });
            }
            Err(e) => out.push(AccountStatus {
                session,
                me: None,
                error: Some(e.to_string()),
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_public_link() {
        let (target, id) = parse_tme("https://t.me/somechannel/123").unwrap();
        assert_eq!(id, 123);
        match target {
            LinkTarget::Username(u) => assert_eq!(u, "somechannel"),
            _ => panic!("expected username"),
        }
    }

    #[test]
    fn parse_private_channel_link() {
        let (target, id) = parse_tme("t.me/c/1234567890/42").unwrap();
        assert_eq!(id, 42);
        match target {
            LinkTarget::ChannelId(c) => assert_eq!(c, 1234567890),
            _ => panic!("expected channel id"),
        }
    }

    #[test]
    fn parse_ignores_query() {
        let (target, id) = parse_tme("https://t.me/chan/7?single").unwrap();
        assert_eq!(id, 7);
        match target {
            LinkTarget::Username(u) => assert_eq!(u, "chan"),
            _ => panic!("expected username"),
        }
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_tme("not a link").is_none());
        assert!(parse_tme("https://t.me/chan/abc").is_none());
        assert!(parse_tme("https://t.me/chan").is_none());
    }

    #[test]
    fn sanitize_names() {
        assert_eq!(sanitize_filename("a/b:c*.mp4"), "a_b_c_.mp4");
        assert_eq!(sanitize_filename("..hidden"), "hidden");
        assert!(!sanitize_filename("...").is_empty());
    }

    #[test]
    fn mime_exts() {
        assert_eq!(ext_for_mime("video/mp4"), ".mp4");
        assert_eq!(ext_for_mime("audio/mpeg"), ".mp3");
        assert_eq!(ext_for_mime("image/jpeg"), ".jpg");
        assert_eq!(ext_for_mime("application/x-whatever"), "");
    }

    #[test]
    fn escapes_newlines_for_one_line_exports() {
        assert_eq!(escape_lines("a\nb"), "a\\nb");
        assert_eq!(escape_lines("a\\b"), "a\\\\b");
        assert_eq!(escape_lines("a\r\nb"), "a\\nb");
        assert!(!escape_lines("multi\nline\ntext").contains('\n'));
    }

    #[test]
    fn flood_wait_is_recognised_by_name() {
        let flood = InvocationError::Rpc(grammers_mtsender::RpcError {
            code: 420,
            name: "FLOOD_WAIT".to_string(),
            value: Some(31),
            caused_by: None,
        });
        assert_eq!(flood_wait_secs(&flood), Some(31));

        let other = InvocationError::Rpc(grammers_mtsender::RpcError {
            code: 400,
            name: "PHONE_CODE_INVALID".to_string(),
            value: None,
            caused_by: None,
        });
        assert_eq!(flood_wait_secs(&other), None);
    }

    #[test]
    fn report_reasons_all_map_to_a_variant() {
        for (key, _) in REPORT_REASONS {
            let _ = report_reason(key);
        }
        assert!(matches!(
            report_reason("unknown-key"),
            tl::enums::ReportReason::InputReportReasonSpam
        ));
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(1048576), "1.0 MB");
    }
}
