use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::peer::Peer;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::input::Input;
use crate::tg::{AccountStatus, ConnectedSession, DialogItem, Me, MemberItem, MsgItem, Tg};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = "telegram-tui";

const TOAST_TTL: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Chat,
    Composer,
}

/// Panels drawn over the conversation pane. Everything that is not a chat lives here.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Help,
    Accounts,
    Members,
    Exports,
    Profile,
    Status,
    Leave,
    Forward,
    Search,
    Report,
}

/// What a resolved peer should be used for once the lookup returns.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Send,
    SendFile,
    Join,
    Report,
    ExportChat,
    ExportMembers,
    Members,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    ApiId,
    ApiHash,
    Phone,
    Code,
    Password,
    SendTarget,
    SendBody,
    Note,
    GlobalSearch,
    ChatSearch,
    EditMessage,
    DeleteMessage,
    SendFileTarget,
    SendFilePath,
    DownloadLink,
    JoinTarget,
    ExportChatTarget,
    ExportMembersTarget,
    MembersTarget,
    Limit,
    ProfileFirst,
    ProfileLast,
    ProfileBio,
    ProfileUsername,
    ProfilePhoto,
    ReportTarget,
    ReportDetail,
    LeaveConfirm,
    DeleteSession,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Error,
}

pub struct Toast {
    pub text: String,
    pub level: Level,
    expires: Instant,
}

pub enum Outcome {
    Toast(String),
    Error(String),
    Dialogs(Vec<DialogItem>),
    Messages(Vec<MsgItem>),
    Older(Vec<MsgItem>),
    Members(Vec<MemberItem>),
    Found(Vec<MsgItem>),
    Statuses(Vec<AccountStatus>),
    Leavable(Vec<DialogItem>),
    Resolved(Purpose, Box<Peer>),
    Connected(Box<ConnectedSession>, String),
    LoginCode(std::result::Result<LoginToken, String>),
    CodeDone(Box<std::result::Result<std::result::Result<Me, PasswordToken>, String>>),
    PasswordDone(std::result::Result<Me, String>),
    Refresh,
}

/// Async work in flight. Results arrive on one channel so a second action never
/// discards the first one's result.
pub struct Tasks {
    tx: mpsc::UnboundedSender<Outcome>,
    rx: mpsc::UnboundedReceiver<Outcome>,
    running: Vec<(String, JoinHandle<()>)>,
}

impl Tasks {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            running: Vec::new(),
        }
    }

    pub fn busy(&self) -> bool {
        !self.running.is_empty()
    }

    pub fn labels(&self) -> Vec<&str> {
        self.running.iter().map(|(l, _)| l.as_str()).collect()
    }

    fn spawn<F>(&mut self, label: &str, future: F)
    where
        F: std::future::Future<Output = Outcome> + Send + 'static,
    {
        let tx = self.tx.clone();
        let handle = tokio::spawn(async move {
            let _ = tx.send(future.await);
        });
        self.running.push((label.to_string(), handle));
    }

    fn reap(&mut self) {
        self.running.retain(|(_, h)| !h.is_finished());
    }

    /// Abort everything in flight; used by Esc so a hung call cannot strand the UI.
    fn cancel_all(&mut self) -> usize {
        let n = self.running.len();
        for (_, handle) in self.running.drain(..) {
            handle.abort();
        }
        n
    }
}

pub struct App {
    pub cfg: Config,
    pub tg: Tg,
    pub focus: Focus,
    pub overlay: Overlay,
    pub prompt: Option<Prompt>,
    pub prompt_title: String,
    pub prompt_input: Input,
    pub prompt_masked: bool,

    pub dialogs: Vec<DialogItem>,
    pub dialog_sel: usize,
    pub filter: Input,
    pub filtering: bool,

    pub open_peer: Option<Peer>,
    pub open_title: String,
    pub messages: Vec<MsgItem>,
    pub msg_sel: usize,
    pub composer: Input,
    pub reply_to: Option<i32>,

    pub members: Vec<MemberItem>,
    pub found: Vec<MsgItem>,
    pub statuses: Vec<AccountStatus>,
    pub leavable: Vec<DialogItem>,
    pub exports: Vec<PathBuf>,
    pub overlay_sel: usize,

    pub sessions: Vec<String>,
    pub me: Option<Me>,
    pub toast: Option<Toast>,
    pub spinner: u8,
    pub tasks: Tasks,
    pub quit: bool,

    pending: Option<Purpose>,
    pending_peer: Option<Peer>,
    pending_text: String,
    forward_msg: Option<i32>,
    report_reason: usize,
    login_phone: String,
    login_token: Option<LoginToken>,
    password_token: Option<PasswordToken>,
    pending_api_id: Option<i32>,
    connecting_session: Option<String>,
}

impl App {
    pub fn new() -> Result<Self> {
        let cfg = Config::load()?;
        let sessions = cfg.list_sessions();
        Ok(Self {
            tg: Tg::new(),
            sessions,
            me: None,
            cfg,
            focus: Focus::Sidebar,
            overlay: Overlay::None,
            prompt: None,
            prompt_title: String::new(),
            prompt_input: Input::default(),
            prompt_masked: false,
            dialogs: Vec::new(),
            dialog_sel: 0,
            filter: Input::default(),
            filtering: false,
            open_peer: None,
            open_title: String::new(),
            messages: Vec::new(),
            msg_sel: 0,
            composer: Input::default(),
            reply_to: None,
            members: Vec::new(),
            found: Vec::new(),
            statuses: Vec::new(),
            leavable: Vec::new(),
            exports: Vec::new(),
            overlay_sel: 0,
            toast: None,
            spinner: 0,
            tasks: Tasks::new(),
            quit: false,
            pending: None,
            pending_peer: None,
            pending_text: String::new(),
            forward_msg: None,
            report_reason: 0,
            login_phone: String::new(),
            login_token: None,
            password_token: None,
            pending_api_id: None,
            connecting_session: None,
        })
    }

    pub fn connected(&self) -> bool {
        self.tg.client.is_some()
    }

    pub(crate) fn client(&self) -> Option<grammers_client::Client> {
        self.tg.client.clone()
    }

    pub(crate) fn spawn<F>(&mut self, label: &str, future: F)
    where
        F: std::future::Future<Output = Outcome> + Send + 'static,
    {
        self.tasks.spawn(label, future);
    }

    /// Drain every finished task. Nothing is dropped, so two actions can overlap.
    pub fn pump(&mut self) {
        while let Ok(outcome) = self.tasks.rx.try_recv() {
            self.apply(outcome);
        }
        self.tasks.reap();
    }

    fn apply(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Toast(msg) => self.info(msg),
            Outcome::Error(msg) => self.error(msg),
            Outcome::Refresh => self.refresh_chat(),
            Outcome::Dialogs(list) => {
                let keep = self.dialogs.get(self.dialog_sel).map(|d| d.peer.id());
                self.dialogs = list;
                self.dialog_sel = keep
                    .and_then(|id| self.dialogs.iter().position(|d| d.peer.id() == id))
                    .unwrap_or(0);
            }
            Outcome::Messages(list) => {
                self.messages = list;
                self.msg_sel = self.messages.len().saturating_sub(1);
                if self.focus == Focus::Sidebar {
                    self.focus = Focus::Chat;
                }
            }
            Outcome::Older(list) => {
                if list.is_empty() {
                    self.info("no older messages");
                } else {
                    let added = list.len();
                    let mut merged = list;
                    merged.append(&mut self.messages);
                    self.messages = merged;
                    self.msg_sel += added;
                }
            }
            Outcome::Members(list) => {
                self.members = list;
                self.overlay = Overlay::Members;
                self.overlay_sel = 0;
            }
            Outcome::Found(list) => {
                if list.is_empty() {
                    self.info("no matches");
                } else {
                    self.found = list;
                    self.overlay = Overlay::Search;
                    self.overlay_sel = 0;
                }
            }
            Outcome::Statuses(list) => self.statuses = list,
            Outcome::Leavable(list) => self.leavable = list,
            Outcome::Resolved(purpose, peer) => self.resolved(purpose, *peer),
            Outcome::Connected(sess, name) => self.on_connected(*sess, name),
            Outcome::LoginCode(Ok(token)) => {
                self.login_token = Some(token);
                self.ask(Prompt::Code, "Login code");
            }
            Outcome::LoginCode(Err(e)) => {
                self.reset_login();
                self.error(e);
            }
            Outcome::CodeDone(v) => match *v {
                Ok(Ok(me)) => self.finish_login(me),
                Ok(Err(token)) => {
                    self.password_token = Some(token);
                    self.ask_masked(Prompt::Password, "2FA password");
                }
                Err(e) => {
                    self.reset_login();
                    self.error(e);
                }
            },
            Outcome::PasswordDone(Ok(me)) => self.finish_login(me),
            Outcome::PasswordDone(Err(e)) => {
                self.reset_login();
                self.error(e);
            }
        }
    }

    fn on_connected(&mut self, sess: ConnectedSession, name: String) {
        let authorized = sess.authorized;
        self.me = sess.me.clone();
        self.tg.set_parts(sess);
        self.tg.session = Some(name.clone());
        self.connecting_session = None;
        if authorized {
            self.cfg.set_last_session(&name);
            self.login_phone.clear();
            self.info(format!("connected as {}", name));
            self.load_dialogs();
        } else if !self.login_phone.is_empty() {
            self.send_code_request();
        } else {
            self.info("this session needs a login");
            self.begin_login();
        }
    }

    fn resolved(&mut self, purpose: Purpose, peer: Peer) {
        match purpose {
            Purpose::Send => {
                let text = std::mem::take(&mut self.pending_text);
                if text.is_empty() {
                    self.pending_peer = Some(peer);
                    self.ask(Prompt::SendBody, "Message");
                } else {
                    self.send_to(peer, text);
                }
            }
            Purpose::SendFile => {
                self.pending_peer = Some(peer);
                self.ask(Prompt::SendFilePath, "File path");
            }
            Purpose::Join => self.join(peer),
            Purpose::Report => {
                self.pending_peer = Some(peer);
                self.overlay = Overlay::Report;
                self.overlay_sel = 0;
            }
            // Walking a big channel is slow and rate limited, so ask how far to go
            // before starting rather than fetching everything.
            Purpose::ExportChat | Purpose::ExportMembers | Purpose::Members => {
                let default = if purpose == Purpose::ExportChat {
                    crate::actions::DEFAULT_EXPORT_LIMIT
                } else {
                    crate::actions::DEFAULT_MEMBER_LIMIT
                };
                self.pending_peer = Some(peer);
                self.pending = Some(purpose);
                self.ask_prefilled(Prompt::Limit, "How many at most?", &default.to_string());
            }
        }
    }

    pub fn begin_login(&mut self) {
        if self.cfg.api_id.is_none() || self.cfg.api_hash.is_none() {
            self.info("set your API credentials first");
            return self.ask(Prompt::ApiId, "API id");
        }
        self.ask(Prompt::Phone, "Phone number (+8801...)");
    }

    pub fn reset_login(&mut self) {
        self.login_token = None;
        self.password_token = None;
        self.login_phone.clear();
        self.prompt = None;
        self.prompt_input.clear();
        self.prompt_masked = false;
    }

    pub fn ask(&mut self, prompt: Prompt, title: &str) {
        self.prompt = Some(prompt);
        self.prompt_title = title.to_string();
        self.prompt_input.clear();
        self.prompt_masked = false;
    }

    pub fn ask_masked(&mut self, prompt: Prompt, title: &str) {
        self.ask(prompt, title);
        self.prompt_masked = true;
    }

    pub fn ask_prefilled(&mut self, prompt: Prompt, title: &str, value: &str) {
        self.ask(prompt, title);
        self.prompt_input.set(value);
    }

    pub(crate) fn take_pending_peer(&mut self) -> Option<Peer> {
        self.pending_peer.take()
    }

    pub(crate) fn stash_peer(&mut self, peer: Peer) {
        self.pending_peer = Some(peer);
    }

    /// Remember the account we just signed into so the next launch reuses it.
    fn finish_login(&mut self, me: Me) {
        let name = crate::submit::session_name(&self.login_phone.clone());
        self.reset_login();
        self.me = Some(me);
        self.tg.session = Some(name.clone());
        self.cfg.set_last_session(&name);
        self.sessions = self.cfg.list_sessions();
        self.info("signed in");
        self.load_dialogs();
    }

    pub(crate) fn set_pending(&mut self, purpose: Purpose) {
        self.pending = Some(purpose);
    }

    pub(crate) fn pending_purpose(&self) -> Option<Purpose> {
        self.pending
    }

    pub(crate) fn stash_text(&mut self, text: String) {
        self.pending_text = text;
    }

    pub(crate) fn set_forward(&mut self, id: Option<i32>) {
        self.forward_msg = id;
    }

    pub(crate) fn forward_target(&self) -> Option<i32> {
        self.forward_msg
    }

    pub fn report_reason_sel(&self) -> usize {
        self.report_reason
    }

    pub fn set_report_reason(&mut self, i: usize) {
        self.report_reason = i;
    }

    pub(crate) fn login_phone_mut(&mut self) -> &mut String {
        &mut self.login_phone
    }

    pub(crate) fn take_login_token(&mut self) -> Option<LoginToken> {
        self.login_token.take()
    }

    pub(crate) fn take_password_token(&mut self) -> Option<PasswordToken> {
        self.password_token.take()
    }

    pub(crate) fn set_pending_api_id(&mut self, id: i32) {
        self.pending_api_id = Some(id);
    }

    pub(crate) fn take_pending_api_id(&mut self) -> Option<i32> {
        self.pending_api_id.take()
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast {
            text: msg.into(),
            level: Level::Info,
            expires: Instant::now() + TOAST_TTL,
        });
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast {
            text: msg.into(),
            level: Level::Error,
            expires: Instant::now() + TOAST_TTL,
        });
    }

    /// Drop an expired toast so it stops covering the footer.
    pub fn tick(&mut self) {
        if let Some(t) = &self.toast {
            if Instant::now() >= t.expires {
                self.toast = None;
            }
        }
        self.tasks.reap();
    }

    pub fn cancel_tasks(&mut self) {
        let n = self.tasks.cancel_all();
        if n > 0 {
            self.info(format!("cancelled {} pending {}", n, plural(n, "task")));
        }
    }

    /// Chats matching the sidebar filter, paired with their index in `dialogs`.
    pub fn visible_dialogs(&self) -> Vec<(usize, &DialogItem)> {
        let needle = self.filter.text.trim().to_lowercase();
        self.dialogs
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                needle.is_empty()
                    || d.name.to_lowercase().contains(&needle)
                    || d.last.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Row of the selected chat within the filtered view.
    pub fn dialog_row(&self) -> Option<usize> {
        self.visible_dialogs()
            .iter()
            .position(|(i, _)| *i == self.dialog_sel)
    }

    pub fn selected_message(&self) -> Option<&MsgItem> {
        self.messages.get(self.msg_sel)
    }

    pub fn reply_preview(&self) -> Option<String> {
        let id = self.reply_to?;
        let m = self.messages.iter().find(|m| m.id == id)?;
        Some(format!("{}: {}", m.sender, crate::text::one_line(&m.text)))
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{}s", word)
    }
}
