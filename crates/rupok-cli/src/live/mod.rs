//! The live board channel of `rupok mcp --live`.
//!
//! [`LiveHub`] owns the loopback WebSocket server ([`ws`]) that an open
//! board connects to, the pairing state, the routing of agent operations to
//! the paired board, and a Loro mirror of the lesson open there. The protocol
//! and the operations live in [`rupok_live`]; the MCP tools that use the hub
//! are in [`crate::mcp_live`].
//!
//! The hub is plain library code with no MCP dependency, so another front end
//! (for example an ACP client relaying an in-app chat on the reserved `chat`
//! channel) can own one in-process: see [`LiveHub::set_extension`] and
//! [`LiveHub::send_extension`].
//!
//! # Pairing and security
//!
//! - The server binds `127.0.0.1` only, on the first free port of
//!   [`rupok_live::protocol::PORTS`] (or `--live-port`).
//! - The WebSocket handshake is refused unless `Host` names this loopback
//!   server and `Origin` is a Rupok board (`https://rupok.app`, a local dev
//!   server, the Tauri shell, or `--live-origin`). Browsers always send
//!   `Origin`, and a page cannot forge it.
//! - A board that says hello without a valid token gets a random 6-digit code
//!   (CSPRNG, unique among waiting boards, expires after 10 minutes). The
//!   teacher reads it on their board and gives it to *their* agent, which
//!   calls `connect_board(code)`. Only then is the board paired and handed a
//!   128-bit session token for reconnects (page reloads) for the life of this
//!   process. Five wrong codes in a row rotate every waiting board's code.
//! - One board at a time: pairing another board drops the previous one.
//! - Until paired, a connection can do nothing but wait for its code.

pub mod ws;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use rupok_core::Lesson;
use rupok_format::Package;
use rupok_live::protocol::{
    BoardMsg, BoardSession, BoardTools, CHANNELS, DocInfo, DocMsg, PROTOCOL_VERSION, ServerMsg,
    ServerSession, ServerTools, encode,
};
use rupok_live::{Op, sync};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

/// Boards that may wait for a code at the same time.
const MAX_PENDING: usize = 4;
/// Wrong codes in a row before every waiting code is replaced.
const MAX_BAD_CODES: u32 = 5;

/// Settings of the live channel.
#[derive(Clone, Debug)]
pub struct LiveConfig {
    /// Ports to try, in order. `0` picks any free port (tests).
    pub ports: Vec<u16>,
    /// Extra board origins allowed besides the built-in ones.
    pub extra_origins: Vec<String>,
    /// How long an agent call waits for the board.
    pub op_timeout: Duration,
    /// How long a new connection may take to say hello.
    pub hello_timeout: Duration,
    /// How long a pairing code stays valid.
    pub code_ttl: Duration,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            ports: rupok_live::protocol::PORTS.to_vec(),
            extra_origins: Vec::new(),
            op_timeout: Duration::from_secs(30),
            hello_timeout: Duration::from_secs(10),
            code_ttl: Duration::from_secs(600),
        }
    }
}

/// A frame (or a close) queued for one connection's writer task.
#[derive(Debug)]
pub(crate) enum Outgoing {
    Text(String),
    Close(u16, String),
}

pub(crate) type Tx = mpsc::UnboundedSender<Outgoing>;

struct Pending {
    code: String,
    created: Instant,
    tx: Tx,
    client: String,
    doc: Option<DocInfo>,
}

struct Board {
    conn: u64,
    tx: Tx,
    client: String,
    doc: Option<DocInfo>,
    state: Option<BoardState>,
}

/// What the paired board last reported.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BoardState {
    pub page: usize,
    pub page_count: usize,
    pub allow_edit: bool,
    pub read_only: bool,
    pub title: String,
}

struct Mirror {
    key: String,
    pkg: Package,
    lesson: Lesson,
}

struct State {
    agent: String,
    port: Option<u16>,
    unavailable: Option<String>,
    next_conn: u64,
    next_op: u64,
    pending: HashMap<u64, Pending>,
    board: Option<Board>,
    token: Option<String>,
    waiting: HashMap<u64, oneshot::Sender<Result<Value, CallError>>>,
    mirror: Option<Mirror>,
    bad_codes: u32,
    extension: Option<mpsc::UnboundedSender<(String, Value)>>,
}

/// A failed agent call: a message for the agent and optional details.
#[derive(Clone, Debug, PartialEq)]
pub struct CallError {
    pub message: String,
    pub data: Option<Value>,
}

impl CallError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            data: None,
        }
    }

    /// The JSON body returned to the agent.
    pub fn to_json(&self) -> Value {
        let mut v = match &self.data {
            Some(Value::Object(m)) => Value::Object(m.clone()),
            _ => json!({}),
        };
        v["error"] = json!(self.message);
        v
    }
}

/// Status of the live channel, as the agent sees it.
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub agent: String,
    pub paired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardState>,
    /// Boards showing a pairing code right now.
    pub boards_waiting: usize,
    /// What to do next.
    pub hint: String,
}

/// The live channel: pairing, op routing and the lesson mirror.
pub struct LiveHub {
    cfg: LiveConfig,
    state: Mutex<State>,
}

impl std::fmt::Debug for LiveHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveHub").finish_non_exhaustive()
    }
}

impl LiveHub {
    pub fn new(cfg: LiveConfig) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            state: Mutex::new(State {
                agent: "AI assistant".into(),
                port: None,
                unavailable: None,
                next_conn: 1,
                next_op: 1,
                pending: HashMap::new(),
                board: None,
                token: None,
                waiting: HashMap::new(),
                mirror: None,
                bad_codes: 0,
                extension: None,
            }),
        })
    }

    /// A hub that never listens (e.g. `--no-live`); every live call explains why.
    pub fn disabled(reason: &str) -> Arc<Self> {
        let hub = Self::new(LiveConfig::default());
        hub.lock().unavailable = Some(reason.to_string());
        hub
    }

    pub fn config(&self) -> &LiveConfig {
        &self.cfg
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Bind the WebSocket server and start accepting boards. Returns the port.
    pub async fn start(self: &Arc<Self>) -> std::io::Result<u16> {
        match ws::bind(&self.cfg.ports).await {
            Ok((listener, port)) => {
                {
                    let mut s = self.lock();
                    s.port = Some(port);
                    s.unavailable = None;
                }
                tokio::spawn(ws::serve(listener, port, self.clone()));
                Ok(port)
            }
            Err(e) => {
                self.lock().unavailable = Some(format!(
                    "the live channel could not listen on 127.0.0.1 ports {:?}: {e}",
                    self.cfg.ports
                ));
                Err(e)
            }
        }
    }

    pub fn port(&self) -> Option<u16> {
        self.lock().port
    }

    pub fn agent(&self) -> String {
        self.lock().agent.clone()
    }

    /// Set the name shown on the board ("Claude Code", "Codex", …).
    pub fn set_agent(&self, name: &str) {
        let mut s = self.lock();
        s.agent = name.to_string();
        let frame = encode(&ServerMsg::Session(ServerSession::Agent {
            agent: name.to_string(),
        }));
        if let Some(b) = &s.board {
            let _ = b.tx.send(Outgoing::Text(frame.clone()));
        }
        for p in s.pending.values() {
            let _ = p.tx.send(Outgoing::Text(frame.clone()));
        }
    }

    pub fn is_paired(&self) -> bool {
        self.lock().board.is_some()
    }

    pub fn status(&self) -> Status {
        let s = self.lock();
        let enabled = s.unavailable.is_none() && s.port.is_some();
        let board = s
            .board
            .as_ref()
            .map(|b| b.state.clone().unwrap_or_default());
        let hint = if let Some(why) = &s.unavailable {
            format!("Live board editing is unavailable: {why}. The file tools still work.")
        } else if !enabled {
            "The live channel is starting.".into()
        } else if let Some(b) = &board {
            let mut h = format!(
                "Paired with the board showing \"{}\", page {} of {}. Use the live tools, and path \"live\" with the lesson tools.",
                b.title, b.page, b.page_count
            );
            if !b.allow_edit {
                h.push_str(" The teacher has turned off \"Allow AI to edit live\": you can read the board but not change it.");
            }
            if b.read_only {
                h.push_str(" The board is in Student view (read-only).");
            }
            h
        } else if s.pending.is_empty() {
            "No board is connected. Ask the teacher to open Rupok and click \"Connect AI\" at the top of the board; it shows a 6-digit code. Then call connect_board with that code.".into()
        } else {
            format!(
                "{} board(s) are showing a pairing code. Ask the teacher for the 6-digit code shown in Rupok under \"Connect AI\", then call connect_board with it.",
                s.pending.len()
            )
        };
        Status {
            enabled,
            port: s.port,
            agent: s.agent.clone(),
            paired: s.board.is_some(),
            board,
            boards_waiting: s.pending.len(),
            hint,
        }
    }

    /// Pair the board showing `code`. On success the board gets a session
    /// token and the previous board (if any) is dropped.
    pub fn connect(&self, code: &str) -> Result<Status, CallError> {
        let typed: String = code.chars().filter(char::is_ascii_digit).collect();
        {
            let mut s = self.lock();
            if let Some(why) = &s.unavailable {
                return Err(CallError::new(format!("live board editing is unavailable: {why}")));
            }
            // Drop expired codes.
            let ttl = self.cfg.code_ttl;
            let expired: Vec<u64> = s
                .pending
                .iter()
                .filter(|(_, p)| p.created.elapsed() > ttl)
                .map(|(id, _)| *id)
                .collect();
            for id in expired {
                if let Some(p) = s.pending.remove(&id) {
                    close(&p.tx, "code_expired", "The pairing code expired. Click Connect AI to get a new one.");
                }
            }
            if typed.len() != 6 {
                return Err(CallError::new(
                    "the pairing code has 6 digits; ask the teacher to read the code shown on the board under \"Connect AI\"",
                ));
            }
            if s.pending.is_empty() {
                return Err(CallError::new(if s.board.is_some() {
                    "a board is already paired and no other board is waiting; call get_board_status"
                } else {
                    "no board is waiting to pair. Ask the teacher to click \"Connect AI\" on the board (it must be open on this computer), then give you the code it shows"
                }));
            }
            let found = s
                .pending
                .iter()
                .find(|(_, p)| ct_eq(p.code.as_bytes(), typed.as_bytes()))
                .map(|(id, _)| *id);
            let Some(conn) = found else {
                s.bad_codes += 1;
                if s.bad_codes >= MAX_BAD_CODES {
                    s.bad_codes = 0;
                    let agent = s.agent.clone();
                    let mut codes: Vec<String> = Vec::new();
                    for p in s.pending.values_mut() {
                        p.code = fresh_code(&codes);
                        p.created = Instant::now();
                        codes.push(p.code.clone());
                        let _ = p.tx.send(Outgoing::Text(welcome(&agent, &p.code, ttl)));
                    }
                    return Err(CallError::new(
                        "that code does not match, and after several wrong codes the board now shows a new one; ask the teacher for the new code",
                    ));
                }
                return Err(CallError::new(
                    "that code does not match any waiting board; ask the teacher to read the 6-digit code on the board again",
                ));
            };
            s.bad_codes = 0;
            let p = s.pending.remove(&conn).expect("found above");
            let token = random_hex(16);
            s.token = Some(token.clone());
            pair(&mut s, conn, p.tx, p.client, p.doc, token);
        }
        Ok(self.status())
    }

    /// Forget the paired board (it may pair again with a new code).
    pub fn disconnect(&self) -> bool {
        let mut s = self.lock();
        s.token = None;
        if let Some(b) = s.board.take() {
            close(&b.tx, "unpaired", "The AI assistant disconnected.");
            fail_waiting(&mut s, "the board was disconnected");
            true
        } else {
            false
        }
    }

    /// Run an agent op on the paired board and wait for its result.
    pub async fn call(&self, op: Op) -> Result<Value, CallError> {
        let (id, rx, track) = {
            let mut s = self.lock();
            let Some(board) = s.board.as_ref() else {
                drop(s);
                let st = self.status();
                return Err(CallError {
                    message: format!("no board is paired. {}", st.hint),
                    data: Some(json!({ "status": st })),
                });
            };
            let tx = board.tx.clone();
            let id = s.next_op;
            s.next_op += 1;
            let (otx, orx) = oneshot::channel();
            s.waiting.insert(id, otx);
            let agent = s.agent.clone();
            let track = match &op {
                Op::AddSmartObject {
                    html,
                    title,
                    width,
                    height,
                    steps,
                    network_hosts,
                    ..
                } => Some(rupok_live::apply::html_bundle(
                    html,
                    title,
                    width.unwrap_or(rupok_live::apply::DEFAULT_OBJECT_W),
                    height.unwrap_or(rupok_live::apply::DEFAULT_OBJECT_H),
                    steps.unwrap_or(0),
                    network_hosts.clone().unwrap_or_default(),
                )),
                _ => None,
            };
            let frame = encode(&ServerMsg::Tools(ServerTools::Op { id, agent, op }));
            if tx.send(Outgoing::Text(frame)).is_err() {
                s.waiting.remove(&id);
                return Err(CallError::new("the board connection closed; ask the teacher to check the board"));
            }
            (id, orx, track)
        };
        let res = match tokio::time::timeout(self.cfg.op_timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(CallError::new("the board disconnected before answering")),
            Err(_) => {
                self.lock().waiting.remove(&id);
                Err(CallError::new(format!(
                    "the board did not answer within {} s; it may be busy or the tab may be in the background",
                    self.cfg.op_timeout.as_secs()
                )))
            }
        };
        // Keep the mirror's package in step with bundles the agent added, so
        // its outline knows their step counts.
        if let (Ok(v), Some(bundle)) = (&res, track)
            && let Some(id) = v.get("bundle_id").and_then(Value::as_str)
            && let Some(m) = self.lock().mirror.as_mut()
        {
            m.pkg.objects.insert(id.to_string(), bundle);
        }
        res
    }

    /// The whole-lesson outline from the mirror (when the board is away).
    pub fn mirror_outline(&self) -> Option<Value> {
        let s = self.lock();
        let m = s.mirror.as_ref()?;
        crate::ops::outline_of(&m.pkg, &m.lesson)
            .ok()
            .and_then(|o| serde_json::to_value(o).ok())
    }

    /// Encoded version vector of the mirror, if any (tests, diagnostics).
    pub fn mirror_version(&self) -> Option<Vec<u8>> {
        self.lock().mirror.as_ref().map(|m| sync::version(&m.lesson))
    }

    /// Run `f` on the mirror lesson, if any.
    pub fn with_mirror<R>(&self, f: impl FnOnce(&Lesson) -> R) -> Option<R> {
        self.lock().mirror.as_ref().map(|m| f(&m.lesson))
    }

    /// Receive frames on channels the hub does not handle (`(channel,
    /// frame)`), e.g. `chat`, from the paired board.
    pub fn set_extension(&self, tx: mpsc::UnboundedSender<(String, Value)>) {
        self.lock().extension = Some(tx);
    }

    /// Send a raw frame to the paired board on an extension channel. The
    /// frame gets `"ch": channel`. Returns false when no board is paired.
    pub fn send_extension(&self, channel: &str, mut frame: Value) -> bool {
        let s = self.lock();
        let Some(b) = &s.board else { return false };
        frame["ch"] = json!(channel);
        b.tx.send(Outgoing::Text(frame.to_string())).is_ok()
    }

    // ---------------------------------------------------- connection side

    pub(crate) fn next_conn(&self) -> u64 {
        let mut s = self.lock();
        let id = s.next_conn;
        s.next_conn += 1;
        id
    }

    /// A connection said hello: pair it by token, or give it a code.
    pub(crate) fn hello(
        &self,
        conn: u64,
        tx: Tx,
        client: String,
        token: Option<String>,
        doc: Option<DocInfo>,
    ) {
        let mut s = self.lock();
        let by_token = match (&token, &s.token) {
            (Some(t), Some(cur)) => ct_eq(t.as_bytes(), cur.as_bytes()),
            _ => false,
        };
        if by_token {
            let t = s.token.clone().expect("checked");
            pair(&mut s, conn, tx, client, doc, t);
            return;
        }
        if s.pending.len() >= MAX_PENDING {
            close(&tx, "busy", "Too many boards are waiting to pair with this AI assistant.");
            return;
        }
        let used: Vec<String> = s.pending.values().map(|p| p.code.clone()).collect();
        let code = fresh_code(&used);
        let _ = tx.send(Outgoing::Text(welcome(&s.agent, &code, self.cfg.code_ttl)));
        s.pending.insert(
            conn,
            Pending {
                code,
                created: Instant::now(),
                tx,
                client,
                doc,
            },
        );
    }

    /// A frame from connection `conn` (after hello).
    pub(crate) fn frame(&self, conn: u64, msg: BoardMsg) {
        let mut s = self.lock();
        let paired = s.board.as_ref().is_some_and(|b| b.conn == conn);
        if !paired {
            // Waiting boards may only say goodbye.
            if matches!(msg, BoardMsg::Session(BoardSession::Bye)) {
                s.pending.remove(&conn);
            }
            return;
        }
        match msg {
            BoardMsg::Session(BoardSession::Hello { .. }) => {}
            BoardMsg::Session(BoardSession::State {
                page,
                page_count,
                allow_edit,
                read_only,
                title,
            }) => {
                if let Some(b) = s.board.as_mut() {
                    b.state = Some(BoardState {
                        page,
                        page_count,
                        allow_edit,
                        read_only,
                        title,
                    });
                }
            }
            BoardMsg::Session(BoardSession::Bye) => {
                s.board = None;
                s.token = None;
                fail_waiting(&mut s, "the teacher disconnected the board");
            }
            BoardMsg::Tools(BoardTools::Result {
                id,
                ok,
                result,
                error,
                data,
            }) => {
                if let Some(w) = s.waiting.remove(&id) {
                    let r = if ok {
                        Ok(result.unwrap_or(Value::Null))
                    } else {
                        Err(CallError {
                            message: error.unwrap_or_else(|| "the board refused the operation".into()),
                            data,
                        })
                    };
                    let _ = w.send(r);
                }
            }
            BoardMsg::Doc(d) => doc_frame(&mut s, d),
        }
    }

    /// A frame on a channel (or of a type) the hub does not handle.
    pub(crate) fn other(&self, conn: u64, channel: String, frame: Value) {
        let s = self.lock();
        if s.board.as_ref().is_some_and(|b| b.conn == conn)
            && let Some(tx) = &s.extension
        {
            let _ = tx.send((channel, frame));
        }
    }

    pub(crate) fn closed(&self, conn: u64) {
        let mut s = self.lock();
        s.pending.remove(&conn);
        if s.board.as_ref().is_some_and(|b| b.conn == conn) {
            s.board = None;
            fail_waiting(&mut s, "the board disconnected");
        }
    }
}

fn doc_frame(s: &mut State, d: DocMsg) {
    match d {
        DocMsg::Snapshot { data, doc } => {
            let loaded = B64
                .decode(data.as_bytes())
                .map_err(|e| e.to_string())
                .and_then(|b| Package::read(&b).map_err(|e| e.to_string()))
                .and_then(|pkg| pkg.lesson().map(|l| (pkg, l)).map_err(|e| e.to_string()));
            match loaded {
                Ok((pkg, lesson)) => {
                    if let Some(b) = s.board.as_mut() {
                        b.doc = Some(doc.clone());
                    }
                    s.mirror = Some(Mirror {
                        key: doc.key,
                        pkg,
                        lesson,
                    });
                }
                Err(e) => eprintln!("rupok live: bad snapshot from the board: {e}"),
            }
        }
        DocMsg::Update { data } => {
            let Some(m) = s.mirror.as_mut() else { return };
            match B64.decode(data.as_bytes()) {
                Ok(bytes) => {
                    if let Err(e) = m.lesson.import(&bytes) {
                        eprintln!("rupok live: could not merge a board update: {e}");
                    }
                }
                Err(e) => eprintln!("rupok live: bad update from the board: {e}"),
            }
        }
        DocMsg::Sync { key, vv } => {
            // The board asks for what it lacks.
            let (Some(m), Some(key), Some(vv)) = (s.mirror.as_ref(), key, vv) else {
                return;
            };
            if m.key != key {
                return;
            }
            let Ok(peer) = B64.decode(vv.as_bytes()) else {
                return;
            };
            if !sync::is_ahead(&m.lesson, &peer) {
                return;
            }
            if let (Ok(delta), Some(b)) = (sync::export_since(&m.lesson, &peer), s.board.as_ref()) {
                let _ = b.tx.send(Outgoing::Text(encode(&ServerMsg::Doc(DocMsg::Update {
                    data: B64.encode(delta),
                }))));
            }
        }
    }
}

/// Make `conn` the paired board.
fn pair(s: &mut State, conn: u64, tx: Tx, client: String, doc: Option<DocInfo>, token: String) {
    if let Some(old) = s.board.take()
        && old.conn != conn
    {
        close(&old.tx, "replaced", "Another board connected to this AI assistant.");
        fail_waiting(s, "the board was replaced by another board");
    }
    let channels: Vec<String> = CHANNELS.iter().map(|c| c.to_string()).collect();
    let _ = tx.send(Outgoing::Text(encode(&ServerMsg::Session(ServerSession::Paired {
        v: PROTOCOL_VERSION,
        token,
        agent: s.agent.clone(),
        channels,
    }))));
    // Tell the board what the mirror has of this lesson, if anything.
    let (key, vv) = match (&s.mirror, &doc) {
        (Some(m), Some(d)) if m.key == d.key => {
            (Some(m.key.clone()), Some(B64.encode(sync::version(&m.lesson))))
        }
        _ => (None, None),
    };
    let _ = tx.send(Outgoing::Text(encode(&ServerMsg::Doc(DocMsg::Sync { key, vv }))));
    s.board = Some(Board {
        conn,
        tx,
        client,
        doc,
        state: None,
    });
}

fn fail_waiting(s: &mut State, why: &str) {
    for (_, w) in s.waiting.drain() {
        let _ = w.send(Err(CallError::new(why)));
    }
}

fn welcome(agent: &str, code: &str, ttl: Duration) -> String {
    encode(&ServerMsg::Session(ServerSession::Welcome {
        v: PROTOCOL_VERSION,
        agent: agent.to_string(),
        code: code.to_string(),
        expires_in: ttl.as_secs(),
        channels: CHANNELS.iter().map(|c| c.to_string()).collect(),
    }))
}

fn close(tx: &Tx, code: &str, msg: &str) {
    let _ = tx.send(Outgoing::Text(encode(&ServerMsg::Session(ServerSession::Error {
        error: msg.to_string(),
        code: code.to_string(),
    }))));
    let _ = tx.send(Outgoing::Close(4000, code.to_string()));
}

/// Constant-time byte comparison (codes and tokens).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("the operating system's random number generator failed");
    b
}

fn random_hex(n: usize) -> String {
    let mut out = String::with_capacity(n * 2);
    for _ in 0..n.div_ceil(16) {
        for byte in random_bytes::<16>() {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out.truncate(n * 2);
    out
}

/// A uniformly random 6-digit code not in `used`.
fn fresh_code(used: &[String]) -> String {
    loop {
        let v = u32::from_le_bytes(random_bytes::<4>());
        // Reject the top sliver so every code is equally likely.
        if v >= u32::MAX - (u32::MAX % 1_000_000) {
            continue;
        }
        let code = format!("{:06}", v % 1_000_000);
        if !used.contains(&code) {
            return code;
        }
    }
}

/// A friendly name for an MCP client (`clientInfo.name` / `title`).
pub fn agent_display_name(name: &str, title: Option<&str>) -> String {
    let n = name.to_ascii_lowercase();
    let pick = if n.contains("claude-code") || n == "claude code" {
        "Claude Code"
    } else if n.starts_with("claude") {
        "Claude"
    } else if n.contains("codex") {
        "Codex"
    } else if n.contains("opencode") {
        "OpenCode"
    } else if n.contains("gemini") {
        "Gemini"
    } else if n.contains("cursor") {
        "Cursor"
    } else {
        let raw = title.filter(|t| !t.trim().is_empty()).unwrap_or(name);
        let clean: String = raw
            .chars()
            .filter(|c| !c.is_control())
            .take(40)
            .collect();
        return if clean.trim().is_empty() {
            "AI assistant".into()
        } else {
            clean.trim().to_string()
        };
    };
    pick.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_tokens() {
        let used = vec!["000000".to_string()];
        for _ in 0..200 {
            let c = fresh_code(&used);
            assert_eq!(c.len(), 6);
            assert!(c.bytes().all(|b| b.is_ascii_digit()));
            assert_ne!(c, "000000");
        }
        let t = random_hex(16);
        assert_eq!(t.len(), 32);
        assert_ne!(t, random_hex(16));
        assert!(ct_eq(b"123456", b"123456"));
        assert!(!ct_eq(b"123456", b"123457"));
        assert!(!ct_eq(b"12345", b"123456"));
    }

    #[test]
    fn agent_names() {
        assert_eq!(agent_display_name("claude-code", None), "Claude Code");
        assert_eq!(agent_display_name("claude-ai", None), "Claude");
        assert_eq!(agent_display_name("codex-mcp-client", None), "Codex");
        assert_eq!(agent_display_name("opencode", None), "OpenCode");
        assert_eq!(agent_display_name("x", Some("My Agent")), "My Agent");
        assert_eq!(agent_display_name("\u{7}", None), "AI assistant");
    }

    #[test]
    fn call_error_json() {
        let e = CallError {
            message: "bad".into(),
            data: Some(json!({"error": "bad", "diagnostics": [1]})),
        };
        assert_eq!(e.to_json(), json!({"error": "bad", "diagnostics": [1]}));
        assert_eq!(CallError::new("x").to_json(), json!({"error": "x"}));
    }
}
