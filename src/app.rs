use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::peer::Peer;
use tokio::sync::oneshot;

use crate::config::Config;
use crate::tg::{self, ConnectedSession, DialogItem, Me, MemberItem, MsgItem, Tg};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = "telegram-tui";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dashboard,
    Help,
    Dialogs,
    Chat,
    Accounts,
    Setup,
    Login,
    Prompt,
    Busy,
    SearchResults,
    Members,
    Exports,
    Profile,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    None,
    SendTarget,
    SendMessage,
    Reply,
    Note,
    Search,
    ExportChatTarget,
    ExportMembersTarget,
    JoinTarget,
    DeleteConfirm,
    DeleteMessageConfirm,
    SearchInChat,
    EditMessage,
    SendFileTarget,
    SendFilePath,
    DownloadLink,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoginStage {
    Phone,
    Code,
    Password,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SetupStage {
    ApiId,
    ApiHash,
}

pub enum Outcome {
    Toast(String),
    Error(String),
    Dialogs(Vec<DialogItem>),
    Messages(Vec<MsgItem>),
    MoreMessages(Vec<MsgItem>),
    Members(Vec<MemberItem>),
    Search(Vec<MsgItem>),
    PeerResolved(Peer),
    FilePeerResolved(Peer),
    Downloaded(String),
    Connected(ConnectedSession, String),
    LoginCode(Result<LoginToken, String>),
    CodeDone(Result<Result<Me, PasswordToken>, String>),
    PasswordDone(Result<Me, String>),
}

pub struct App {
    pub cfg: Config,
    pub tg: Tg,
    pub mode: Mode,
    pub mode_before_busy: Mode,
    pub dash_sel: usize,
    pub input: String,
    pub dialogs: Vec<DialogItem>,
    pub dialogs_sel: usize,
    pub messages: Vec<MsgItem>,
    pub msg_scroll: usize,
    pub msg_action_id: Option<i32>,
    pub msg_oldest_id: Option<i32>,
    pub search: Vec<MsgItem>,
    pub members: Vec<MemberItem>,
    pub members_sel: usize,
    pub sessions: Vec<String>,
    pub accounts_sel: usize,
    pub me: Option<Me>,
    pub toast: Option<String>,
    pub busy_title: String,
    pub spinner: u8,
    pub prompt_kind: PromptKind,
    pub prompt_title: String,
    pub send_peer: Option<Peer>,
    pub delete_name: Option<String>,
    pub login_stage: LoginStage,
    pub login_phone: String,
    pub login_token: Option<LoginToken>,
    pub login_password_token: Option<PasswordToken>,
    pub setup_stage: SetupStage,
    pub exports: Vec<std::path::PathBuf>,
    pub downloads: Vec<std::path::PathBuf>,
    pub pending: Option<oneshot::Receiver<Outcome>>,
    pub quit: bool,
}

pub const DASHBOARD_ITEMS: &[(&str, &str)] = &[
    ("/setup", "Setup API credentials"),
    ("/login", "Login a new account"),
    ("/inbox", "Check inbox"),
    ("/send", "Send a message"),
    ("/sendfile", "Send a file"),
    ("/note", "Save note"),
    ("/search", "Search messages"),
    ("/dialogs", "Export dialogs CSV"),
    ("/members", "Export members CSV"),
    ("/chat", "Export chat history"),
    ("/profile", "View profile"),
    ("/join", "Join group/channel"),
    ("/download", "Download media from a link"),
    ("/accounts", "Accounts"),
    ("/exports", "Exported files"),
    ("/help", "Help"),
    ("/quit", "Exit"),
];

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
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
            mode: Mode::Dashboard,
            mode_before_busy: Mode::Dashboard,
            dash_sel: 0,
            input: String::new(),
            dialogs: Vec::new(),
            dialogs_sel: 0,
            messages: Vec::new(),
            msg_scroll: 0,
            msg_action_id: None,
            msg_oldest_id: None,
            search: Vec::new(),
            members: Vec::new(),
            members_sel: 0,
            accounts_sel: 0,
            toast: None,
            busy_title: String::new(),
            spinner: 0,
            prompt_kind: PromptKind::None,
            prompt_title: String::new(),
            send_peer: None,
            delete_name: None,
            login_stage: LoginStage::Phone,
            login_phone: String::new(),
            login_token: None,
            login_password_token: None,
            setup_stage: SetupStage::ApiId,
            exports: Vec::new(),
            downloads: Vec::new(),
            pending: None,
            quit: false,
        })
    }

    pub fn connected(&self) -> bool {
        self.tg.client.is_some()
    }

    pub(crate) fn spawn<F>(&mut self, title: &str, future: F)
    where
        F: std::future::Future<Output = Outcome> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(future.await);
        });
        self.mode_before_busy = self.mode;
        self.mode = Mode::Busy;
        self.busy_title = title.to_string();
        self.pending = Some(rx);
    }

    pub fn pump_pending(&mut self) {
        if let Some(rx) = &mut self.pending {
            if let Ok(outcome) = rx.try_recv() {
                self.pending = None;
                self.handle_outcome(outcome);
            }
        }
    }

    fn handle_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Toast(msg) => {
                self.toast = Some(msg);
                self.mode = self.mode_before_busy;
            }
            Outcome::Error(msg) => {
                self.toast = Some(format!("error: {}", msg));
                self.mode = self.mode_before_busy;
            }
            Outcome::Dialogs(list) => {
                self.dialogs = list;
                self.dialogs_sel = 0;
                self.mode = Mode::Dialogs;
            }
            Outcome::Messages(list) => {
                self.messages = list;
                self.msg_oldest_id = self.messages.first().map(|m| m.id);
                self.msg_scroll = 0;
                self.mode = Mode::Chat;
            }
            Outcome::MoreMessages(list) => {
                if list.is_empty() {
                    self.toast = Some("no older messages".to_string());
                } else {
                    let mut older = list;
                    older.append(&mut self.messages);
                    self.messages = older;
                    self.msg_oldest_id = self.messages.first().map(|m| m.id);
                }
                self.mode = self.mode_before_busy;
            }
            Outcome::Members(list) => {
                self.members = list;
                self.members_sel = 0;
                self.mode = Mode::Members;
            }
            Outcome::Search(list) => {
                self.search = list;
                self.mode = Mode::SearchResults;
            }
            Outcome::PeerResolved(peer) => {
                self.send_peer = Some(peer);
                self.start_prompt(PromptKind::SendMessage, "Message text");
            }
            Outcome::FilePeerResolved(peer) => {
                self.send_peer = Some(peer);
                self.start_prompt(PromptKind::SendFilePath, "File path to upload");
            }
            Outcome::Downloaded(path) => {
                self.toast = Some(format!("saved -> {}", path));
                self.mode = self.mode_before_busy;
            }
            Outcome::Connected(sess, name) => {
                let me = sess.me.clone();
                let authorized = sess.authorized;
                self.tg.set_parts(sess);
                self.tg.session = Some(name);
                self.me = me;
                if authorized {
                    self.sessions = self.cfg.list_sessions();
                    self.toast = Some("connected".to_string());
                    self.mode = Mode::Dashboard;
                    self.reset_login();
                } else {
                    let phone = self.login_phone.clone();
                    let Some(client) = self.tg.client.clone() else {
                        self.toast = Some("login failed".to_string());
                        self.mode = Mode::Dashboard;
                        self.reset_login();
                        return;
                    };
                    let Some(api_hash) = self.cfg.api_hash.clone() else {
                        self.toast = Some("api hash missing; run /setup".to_string());
                        self.mode = Mode::Dashboard;
                        self.reset_login();
                        return;
                    };
                    self.spawn("Requesting login code", async move {
                        match tg::request_code(&client, &phone, &api_hash).await {
                            Ok(token) => Outcome::LoginCode(Ok(token)),
                            Err(e) => Outcome::LoginCode(Err(e.to_string())),
                        }
                    });
                }
            }
            Outcome::LoginCode(Ok(token)) => {
                self.login_token = Some(token);
                self.login_stage = LoginStage::Code;
                self.mode = Mode::Login;
                self.input.clear();
            }
            Outcome::LoginCode(Err(e)) => {
                self.toast = Some(format!("error: {}", e));
                self.mode = Mode::Dashboard;
                self.reset_login();
            }
            Outcome::CodeDone(Ok(Ok(me))) => {
                self.me = Some(me.clone());
                self.tg.me = Some(me);
                self.sessions = self.cfg.list_sessions();
                self.toast = Some("logged in".to_string());
                self.mode = Mode::Dashboard;
                self.reset_login();
            }
            Outcome::CodeDone(Ok(Err(token))) => {
                self.login_password_token = Some(token);
                self.login_stage = LoginStage::Password;
                self.mode = Mode::Login;
                self.input.clear();
            }
            Outcome::CodeDone(Err(e)) => {
                self.toast = Some(format!("error: {}", e));
                self.mode = Mode::Dashboard;
                self.reset_login();
            }
            Outcome::PasswordDone(Ok(me)) => {
                self.me = Some(me.clone());
                self.tg.me = Some(me);
                self.sessions = self.cfg.list_sessions();
                self.toast = Some("logged in".to_string());
                self.mode = Mode::Dashboard;
                self.reset_login();
            }
            Outcome::PasswordDone(Err(e)) => {
                self.toast = Some(format!("error: {}", e));
                self.mode = Mode::Dashboard;
                self.reset_login();
            }
        }
    }

    fn reset_login(&mut self) {
        self.login_stage = LoginStage::Phone;
        self.login_phone.clear();
        self.login_token = None;
        self.login_password_token = None;
        self.input.clear();
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match self.mode {
            Mode::Busy => {}
            Mode::Dashboard => self.key_dashboard(key),
            Mode::Help | Mode::Profile | Mode::Exports => {
                if key.code == KeyCode::Esc {
                    self.mode = Mode::Dashboard;
                }
            }
            Mode::SearchResults => self.key_scroll_escape(key),
            Mode::Members => self.key_members(key),
            Mode::Dialogs => self.key_dialogs(key),
            Mode::Chat => self.key_chat(key),
            Mode::Accounts => self.key_accounts(key),
            Mode::Setup => self.key_setup(key),
            Mode::Login => self.key_login(key),
            Mode::Prompt => self.key_prompt(key),
        }
    }

    fn key_scroll_escape(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Dashboard,
            KeyCode::Up | KeyCode::Char('k') => {
                self.msg_scroll = self.msg_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.msg_scroll + 1 < self.search.len() {
                    self.msg_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.msg_scroll = self.msg_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.msg_scroll = (self.msg_scroll + 10).min(self.search.len().saturating_sub(1));
            }
            _ => {}
        }
    }

    fn key_dialogs(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Dashboard,
            KeyCode::Up | KeyCode::Char('k') => {
                self.dialogs_sel = self.dialogs_sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.dialogs_sel + 1 < self.dialogs.len() {
                    self.dialogs_sel += 1;
                }
            }
            KeyCode::Enter => self.open_chat(self.dialogs_sel),
            KeyCode::Char('r') => self.load_dialogs(),
            KeyCode::Char('x') => self.export_selected_members(),
            KeyCode::Char('e') => self.export_selected_chat(),
            _ => {}
        }
    }

    fn key_members(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Chat,
            KeyCode::Up | KeyCode::Char('k') => {
                self.members_sel = self.members_sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.members_sel + 1 < self.members.len() {
                    self.members_sel += 1;
                }
            }
            KeyCode::PageUp => self.members_sel = self.members_sel.saturating_sub(10),
            KeyCode::PageDown => {
                self.members_sel = (self.members_sel + 10).min(self.members.len().saturating_sub(1))
            }
            KeyCode::Char('x') => self.export_selected_members(),
            KeyCode::Char('e') => self.export_selected_chat(),
            _ => {}
        }
    }

    fn key_chat(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Dialogs,
            KeyCode::Up | KeyCode::Char('k') => {
                self.msg_scroll = self.msg_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.msg_scroll + 1 < self.messages.len() {
                    self.msg_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.msg_scroll = self.msg_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.msg_scroll = (self.msg_scroll + 10).min(self.messages.len().saturating_sub(1));
            }
            KeyCode::Home => self.msg_scroll = 0,
            KeyCode::End => self.msg_scroll = self.messages.len().saturating_sub(1),
            KeyCode::Char('r') => {
                if let Some(peer) = self.current_peer() {
                    self.send_peer = Some(peer);
                    self.start_prompt(PromptKind::Reply, "Reply message");
                }
            }
            KeyCode::Char('s') => {
                if let Some(peer) = self.current_peer() {
                    self.send_peer = Some(peer);
                    self.start_prompt(PromptKind::SendMessage, "New message");
                }
            }
            KeyCode::Char('o') | KeyCode::Char('l') => self.load_older_messages(),
            KeyCode::Char('M') => self.mark_read_chat(),
            KeyCode::Char('R') => self.refresh_chat(),
            KeyCode::Char('f') => {
                if let Some(peer) = self.current_peer() {
                    self.send_peer = Some(peer);
                    self.start_prompt(PromptKind::SearchInChat, "Search query");
                }
            }
            KeyCode::Char('d') => {
                if let Some(id) = self.selected_msg_id() {
                    self.msg_action_id = Some(id);
                    self.start_prompt(PromptKind::DeleteMessageConfirm, "Type DELETE to remove this message");
                }
            }
            KeyCode::Char('E') => {
                if let Some(id) = self.selected_msg_id() {
                    self.msg_action_id = Some(id);
                    self.start_prompt(PromptKind::EditMessage, "New message text");
                }
            }
            KeyCode::Char('p') => self.pin_selected(false),
            KeyCode::Char('P') => self.pin_selected(true),
            KeyCode::Char('v') => self.load_members(),
            KeyCode::Char('e') => self.export_current_chat(),
            KeyCode::Char('m') => self.export_current_members(),
            KeyCode::Char('g') => self.download_selected_media(),
            _ => {}
        }
    }

    fn key_accounts(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Dashboard,
            KeyCode::Up | KeyCode::Char('k') => {
                self.accounts_sel = self.accounts_sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.accounts_sel + 1 < self.sessions.len() {
                    self.accounts_sel += 1;
                }
            }
            KeyCode::Enter => self.switch_account(),
            KeyCode::Char('l') => {
                self.reset_login();
                self.mode = Mode::Login;
            }
            KeyCode::Char('d') => {
                if let Some(name) = self.sessions.get(self.accounts_sel).cloned() {
                    self.delete_name = Some(name);
                    self.start_prompt(PromptKind::DeleteConfirm, "Type DELETE to remove this session");
                }
            }
            _ => {}
        }
    }

    fn key_setup(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Dashboard,
            KeyCode::Enter => self.setup_submit(),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn key_login(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.reset_login();
                self.mode = Mode::Dashboard;
            }
            KeyCode::Enter => self.login_submit(),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn key_prompt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.prompt_kind = PromptKind::None;
                self.send_peer = None;
                self.delete_name = None;
                self.msg_action_id = None;
                self.input.clear();
                self.mode = self.mode_before_busy;
            }
            KeyCode::Enter => self.prompt_submit(),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn key_dashboard(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.dash_sel = self.dash_sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.dash_sel + 1 < DASHBOARD_ITEMS.len() {
                    self.dash_sel += 1;
                }
            }
            KeyCode::Enter => {
                let cmd = DASHBOARD_ITEMS[self.dash_sel].0;
                self.run_command(cmd);
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn start_prompt(&mut self, kind: PromptKind, title: &str) {
        self.mode_before_busy = if self.mode == Mode::Busy {
            Mode::Dashboard
        } else {
            self.mode
        };
        self.prompt_kind = kind;
        self.prompt_title = title.to_string();
        self.input.clear();
        self.mode = Mode::Prompt;
    }

    fn current_peer(&self) -> Option<Peer> {
        self.dialogs.get(self.dialogs_sel).map(|d| d.peer.clone())
    }

    fn selected_msg_id(&self) -> Option<i32> {
        let last = self.messages.len().saturating_sub(1);
        self.messages
            .get((self.msg_scroll / 2).min(last))
            .map(|m| m.id)
    }

    fn run_command(&mut self, raw: &str) {
        let cmd = raw.trim().to_lowercase();
        self.input.clear();
        match cmd.as_str() {
            "" => {}
            "0" | "q" | "/quit" | "/exit" | "quit" => {
                self.quit = true;
            }
            "/home" | "/menu" => {}
            "/help" | "?" | "/h" => self.mode = Mode::Help,
            "/setup" | "/config" | "1" => {
                self.input.clear();
                self.setup_stage = SetupStage::ApiId;
                self.mode = Mode::Setup;
            }
            "/login" | "2" => {
                self.reset_login();
                self.mode = Mode::Login;
            }
            "/inbox" | "3" => self.load_dialogs(),
            "/send" | "5" => self.start_prompt(PromptKind::SendTarget, "Target @username"),
            "/sendfile" => self.start_prompt(PromptKind::SendFileTarget, "Send file: target @username"),
            "/note" | "6" => self.start_prompt(PromptKind::Note, "Note text (saved to yourself)"),
            "/search" | "7" => self.start_prompt(PromptKind::Search, "Search query"),
            "/dialogs" | "8" => self.export_all_dialogs(),
            "/members" | "9" => {
                self.start_prompt(PromptKind::ExportMembersTarget, "Group/channel @username")
            }
            "/chat" | "10" => self.start_prompt(PromptKind::ExportChatTarget, "Chat @username"),
            "/profile" | "11" => {
                if !self.connected() {
                    self.toast = Some("not connected; use /login first".to_string());
                } else {
                    self.mode = Mode::Profile;
                }
            }
            "/join" | "12" => {
                self.start_prompt(PromptKind::JoinTarget, "Public @username or t.me link")
            }
            "/accounts" | "/switch" | "/status" | "13" | "15" => {
                self.refresh_sessions();
                self.mode = Mode::Accounts;
            }
            "/download" | "4" => self.start_prompt(
                PromptKind::DownloadLink,
                "Message link (t.me/... or t.me/c/.../msg)",
            ),
            "/exports" => {
                self.exports = self.cfg.list_exports();
                self.downloads = self.cfg.list_downloads();
                self.mode = Mode::Exports;
            }
            _ => {
                self.toast = Some(format!("unknown command: {}", cmd));
            }
        }
    }

    fn refresh_sessions(&mut self) {
        self.sessions = self.cfg.list_sessions();
        if !self.sessions.is_empty() && self.accounts_sel >= self.sessions.len() {
            self.accounts_sel = 0;
        }
    }

    fn load_dialogs(&mut self) {
        let Some(client) = self.tg.client.clone() else {
            self.toast = Some("not connected; use /login first".to_string());
            return;
        };
        self.spawn("Loading dialogs", async move {
            match tg::fetch_dialogs(&client).await {
                Ok(list) => Outcome::Dialogs(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn refresh_chat(&mut self) {
        self.open_chat(self.dialogs_sel);
    }

    fn load_older_messages(&mut self) {
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(offset) = self.msg_oldest_id else {
            self.toast = Some("no older messages".to_string());
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        self.spawn("Loading older messages", async move {
            match tg::fetch_messages_older(&client, &peer, offset, 100).await {
                Ok(list) => Outcome::MoreMessages(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn mark_read_chat(&mut self) {
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        self.spawn("Marking as read", async move {
            match tg::mark_read(&client, &peer).await {
                Ok(()) => Outcome::Toast("marked as read".to_string()),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn pin_selected(&mut self, unpin: bool) {
        let Some(id) = self.selected_msg_id() else {
            return;
        };
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        let label = if unpin { "Unpinning" } else { "Pinning" };
        self.spawn(label, async move {
            match tg::set_pinned(&client, &peer, id, unpin).await {
                Ok(()) => Outcome::Toast(if unpin {
                    "unpinned".to_string()
                } else {
                    "pinned".to_string()
                }),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn load_members(&mut self) {
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        self.spawn("Loading members", async move {
            match tg::fetch_members(&client, &peer).await {
                Ok(list) => Outcome::Members(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn open_chat(&mut self, index: usize) {
        let Some(peer) = self.dialogs.get(index).map(|d| d.peer.clone()) else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        self.spawn("Loading messages", async move {
            match tg::fetch_messages(&client, &peer, 100).await {
                Ok(list) => Outcome::Messages(list),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn export_all_dialogs(&mut self) {
        let Some(client) = self.tg.client.clone() else {
            self.toast = Some("not connected; use /login first".to_string());
            return;
        };
        let path = self.cfg.exports_dir().join(format!("dialogs-{}.csv", timestamp()));
        let shown = path.clone();
        self.spawn("Exporting dialogs", async move {
            match tg::export_dialogs(&client, &path).await {
                Ok(n) => Outcome::Toast(format!("exported {} dialogs -> {}", n, shown.display())),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn export_selected_members(&mut self) {
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        let path = self
            .cfg
            .exports_dir()
            .join(format!("members-{}.csv", timestamp()));
        let shown = path.clone();
        self.spawn("Exporting members", async move {
            match tg::export_members(&client, &peer, &path).await {
                Ok(n) => Outcome::Toast(format!("exported {} members -> {}", n, shown.display())),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn export_selected_chat(&mut self) {
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            return;
        };
        let path = self
            .cfg
            .exports_dir()
            .join(format!("chat-{}.txt", timestamp()));
        let shown = path.clone();
        self.spawn("Exporting chat", async move {
            match tg::export_chat(&client, &peer, &path, 1000).await {
                Ok(n) => Outcome::Toast(format!("exported {} messages -> {}", n, shown.display())),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn export_current_chat(&mut self) {
        self.export_selected_chat();
    }

    fn download_selected_media(&mut self) {
        let Some(peer) = self.current_peer() else {
            return;
        };
        let Some(id) = self.selected_msg_id() else {
            return;
        };
        let Some(client) = self.tg.client.clone() else {
            self.toast = Some("not connected; use /login first".to_string());
            return;
        };
        let dir = self.cfg.downloads_dir();
        self.spawn("Downloading media", async move {
            match tg::download_selected_media(&client, &peer, id, &dir).await {
                Ok(path) => Outcome::Downloaded(path),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn export_current_members(&mut self) {
        self.export_selected_members();
    }

    fn switch_account(&mut self) {
        let Some(name) = self.sessions.get(self.accounts_sel).cloned() else {
            return;
        };
        let Some(api_id) = self.cfg.api_id else {
            self.toast = Some("api id missing; run /setup".to_string());
            return;
        };
        let path = self.cfg.session_path(&name);
        self.dialogs.clear();
        self.messages.clear();
        self.search.clear();
        self.spawn("Connecting", async move {
            match tg::connect_session(&path, api_id).await {
                Ok(sess) => Outcome::Connected(sess, name),
                Err(e) => Outcome::Error(e.to_string()),
            }
        });
    }

    fn setup_submit(&mut self) {
        match self.setup_stage {
            SetupStage::ApiId => {
                let value = self.input.trim().to_string();
                match value.parse::<i32>() {
                    Ok(id) => {
                        self.cfg.api_id = Some(id);
                        self.setup_stage = SetupStage::ApiHash;
                        self.input.clear();
                    }
                    Err(_) => {
                        self.toast = Some("api id must be a number".to_string());
                    }
                }
            }
            SetupStage::ApiHash => {
                let hash = self.input.trim().to_string();
                if hash.is_empty() {
                    self.toast = Some("api hash cannot be empty".to_string());
                    return;
                }
                let id = self.cfg.api_id.unwrap_or(0);
                self.cfg.api_hash = Some(hash.clone());
                match self.cfg.save_credentials(id, &hash) {
                    Ok(()) => {
                        self.toast = Some("credentials saved".to_string());
                    }
                    Err(e) => {
                        self.toast = Some(format!("error saving: {}", e));
                    }
                }
                self.setup_stage = SetupStage::ApiId;
                self.input.clear();
                self.mode = Mode::Dashboard;
            }
        }
    }

    fn login_submit(&mut self) {
        match self.login_stage {
            LoginStage::Phone => {
                let phone = self.input.trim().to_string();
                if phone.is_empty() {
                    self.toast = Some("enter a phone number".to_string());
                    return;
                }
                self.login_phone = phone.clone();
                self.input.clear();
                let Some(api_id) = self.cfg.api_id else {
                    self.toast = Some("api id missing; run /setup".to_string());
                    self.reset_login();
                    self.mode = Mode::Dashboard;
                    return;
                };
                let clean: String = phone.chars().filter(|c| *c != '+' && *c != ' ').collect();
                let path = self.cfg.session_path(&clean);
                self.spawn("Connecting", async move {
                    match tg::connect_session(&path, api_id).await {
                        Ok(sess) => Outcome::Connected(sess, clean),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            LoginStage::Code => {
                let code = std::mem::take(&mut self.input);
                let Some(token) = self.login_token.take() else {
                    self.reset_login();
                    self.mode = Mode::Dashboard;
                    return;
                };
                let Some(client) = self.tg.client.clone() else {
                    return;
                };
                self.spawn("Signing in", async move {
                    match tg::submit_code(&client, &token, &code).await {
                        Ok(inner) => Outcome::CodeDone(Ok(inner)),
                        Err(e) => Outcome::CodeDone(Err(e.to_string())),
                    }
                });
            }
            LoginStage::Password => {
                let password = std::mem::take(&mut self.input);
                let Some(token) = self.login_password_token.take() else {
                    self.reset_login();
                    self.mode = Mode::Dashboard;
                    return;
                };
                let Some(client) = self.tg.client.clone() else {
                    return;
                };
                self.spawn("Checking password", async move {
                    match tg::submit_password(&client, token, &password).await {
                        Ok(me) => Outcome::PasswordDone(Ok(me)),
                        Err(e) => Outcome::PasswordDone(Err(e.to_string())),
                    }
                });
            }
        }
    }

    fn prompt_submit(&mut self) {
        let kind = self.prompt_kind;
        let text = std::mem::take(&mut self.input);
        self.prompt_kind = PromptKind::None;
        match kind {
            PromptKind::None => {}
            PromptKind::SendTarget => {
                if text.is_empty() {
                    self.toast = Some("target is empty".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Resolving target", async move {
                    match tg::resolve_username(&client, &text).await {
                        Ok(peer) => Outcome::PeerResolved(peer),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::SendMessage | PromptKind::Reply => {
                let Some(peer) = self.send_peer.clone() else {
                    self.toast = Some("no target".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.send_peer = None;
                self.spawn("Sending message", async move {
                    match tg::send_text(&client, &peer, &text).await {
                        Ok(()) => match tg::fetch_messages(&client, &peer, 100).await {
                            Ok(list) => Outcome::Messages(list),
                            Err(e) => Outcome::Error(e.to_string()),
                        },
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::Note => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Saving note", async move {
                    match tg::send_to_self(&client, &text).await {
                        Ok(()) => Outcome::Toast("note saved to Saved Messages".to_string()),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::Search => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Searching", async move {
                    match tg::search_messages(&client, &text, 50).await {
                        Ok(list) => Outcome::Search(list),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::ExportChatTarget => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                let path = self
                    .cfg
                    .exports_dir()
                    .join(format!("chat-{}.txt", timestamp()));
                let shown = path.clone();
                self.spawn("Exporting chat", async move {
                    let peer = match tg::resolve_username(&client, &text).await {
                        Ok(p) => p,
                        Err(e) => return Outcome::Error(e.to_string()),
                    };
                    match tg::export_chat(&client, &peer, &path, 1000).await {
                        Ok(n) => Outcome::Toast(format!(
                            "exported {} messages -> {}",
                            n,
                            shown.display()
                        )),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::ExportMembersTarget => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                let path = self
                    .cfg
                    .exports_dir()
                    .join(format!("members-{}.csv", timestamp()));
                let shown = path.clone();
                self.spawn("Exporting members", async move {
                    let peer = match tg::resolve_username(&client, &text).await {
                        Ok(p) => p,
                        Err(e) => return Outcome::Error(e.to_string()),
                    };
                    match tg::export_members(&client, &peer, &path).await {
                        Ok(n) => Outcome::Toast(format!(
                            "exported {} members -> {}",
                            n,
                            shown.display()
                        )),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::JoinTarget => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Joining", async move {
                    let peer = match tg::resolve_username(&client, &text).await {
                        Ok(p) => p,
                        Err(e) => return Outcome::Error(e.to_string()),
                    };
                    match tg::join_chat(&client, &peer).await {
                        Ok(()) => Outcome::Toast("joined".to_string()),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::DeleteConfirm => {
                let confirmed = text == "DELETE";
                let name = self.delete_name.take();
                if confirmed {
                    if let Some(name) = name {
                        if self.tg.session.as_deref() == Some(name.as_str()) {
                            self.tg.disconnect();
                            self.me = None;
                        }
                        self.cfg.remove_session(&name);
                        self.toast = Some(format!("removed session: {}", name));
                    }
                } else {
                    self.toast = Some("deletion cancelled".to_string());
                }
                self.refresh_sessions();
                self.mode = self.mode_before_busy;
            }
            PromptKind::DeleteMessageConfirm => {
                let confirmed = text == "DELETE";
                let id = self.msg_action_id.take();
                let Some(peer) = self.send_peer.clone() else {
                    self.send_peer = None;
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.send_peer = None;
                if !confirmed {
                    self.toast = Some("deletion cancelled".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(id) = id else {
                    self.mode = self.mode_before_busy;
                    return;
                };
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Deleting message", async move {
                    match tg::delete_message(&client, &peer, id).await {
                        Ok(()) => match tg::fetch_messages(&client, &peer, 100).await {
                            Ok(list) => Outcome::Messages(list),
                            Err(e) => Outcome::Error(e.to_string()),
                        },
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::SearchInChat => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(peer) = self.send_peer.clone() else {
                    self.send_peer = None;
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.send_peer = None;
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Searching in chat", async move {
                    match tg::search_in_chat(&client, &peer, &text, 50).await {
                        Ok(list) => Outcome::Search(list),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::EditMessage => {
                let Some(id) = self.msg_action_id.take() else {
                    self.send_peer = None;
                    self.mode = self.mode_before_busy;
                    return;
                };
                let Some(peer) = self.send_peer.clone() else {
                    self.send_peer = None;
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.send_peer = None;
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Editing message", async move {
                    match tg::edit_message_text(&client, &peer, id, &text).await {
                        Ok(()) => match tg::fetch_messages(&client, &peer, 100).await {
                            Ok(list) => Outcome::Messages(list),
                            Err(e) => Outcome::Error(e.to_string()),
                        },
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::SendFileTarget => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Resolving target", async move {
                    match tg::resolve_username(&client, &text).await {
                        Ok(peer) => Outcome::FilePeerResolved(peer),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::SendFilePath => {
                let Some(peer) = self.send_peer.clone() else {
                    self.send_peer = None;
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.send_peer = None;
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                self.spawn("Uploading file", async move {
                    match tg::send_file(&client, &peer, &text, "").await {
                        Ok(()) => match tg::fetch_messages(&client, &peer, 100).await {
                            Ok(list) => Outcome::Messages(list),
                            Err(e) => Outcome::Error(e.to_string()),
                        },
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
            PromptKind::DownloadLink => {
                if text.is_empty() {
                    self.mode = self.mode_before_busy;
                    return;
                }
                let Some(client) = self.tg.client.clone() else {
                    self.toast = Some("not connected".to_string());
                    self.mode = self.mode_before_busy;
                    return;
                };
                let dir = self.cfg.downloads_dir();
                self.spawn("Downloading media", async move {
                    match tg::download_from_link(&client, &text, &dir).await {
                        Ok(path) => Outcome::Downloaded(path),
                        Err(e) => Outcome::Error(e.to_string()),
                    }
                });
            }
        }
    }
}
