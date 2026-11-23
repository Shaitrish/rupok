//! The in-app chat: `rupok agent` relays the board's `chat` channel to the
//! teacher's own agent (Claude Code, Codex, OpenCode, Gemini CLI…) spoken
//! to over ACP (the Agent Client Protocol) on the agent's stdio.
//!
//! ```text
//! board ── ws `chat` ──▶ ChatHub ── ACP (stdio) ──▶ agent ── MCP (loopback HTTP) ──▶ Rupok tools ──▶ board
//! ```
//!
//! The agent gets the Rupok MCP tools (`McpServer::with_live`, see
//! [`crate::mcp_http`]) in `session/new`, so what it builds lands on the
//! board through the live channel. Session updates are forwarded to the
//! board as their ACP JSON (`sessionUpdate`: `agent_message_chunk`,
//! `tool_call`, `tool_call_update`, `plan`…); permission requests wait for
//! the teacher's answer. Frames: docs/ai-agents.md § "`chat`".

pub mod agents;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_client_protocol::v1::schema::{
    CancelNotification, InitializeRequest, InitializeResponse, RequestPermissionRequest,
    RequestPermissionResponse, SessionNotification,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent, Client};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::live::LiveHub;
use crate::mcp_http::McpHttp;
use agents::AgentInfo;

/// Settings of the chat relay.
#[derive(Clone, Debug)]
pub struct ChatConfig {
    /// Folder the agent works in (its `cwd`; also the MCP workspace).
    pub cwd: PathBuf,
    /// The Rupok MCP endpoint handed to the agent.
    pub mcp: McpHttp,
    /// Agent to start first (`--agent`), else the first available.
    pub default_agent: Option<String>,
    /// A custom ACP command (`--agent-command`), with its label.
    pub custom: Option<(String, String)>,
}

/// Options of `rupok agent` (and of the desktop app's built-in relay).
#[derive(Clone, Debug, Default)]
pub struct AgentOptions {
    /// Lessons folder: the agent's working directory and the MCP workspace.
    pub workspace: PathBuf,
    pub default_agent: Option<String>,
    /// `(label, command)` of a custom ACP agent.
    pub custom: Option<(String, String)>,
    /// Live channel ports (default: 47373–47376).
    pub ports: Option<Vec<u16>>,
    pub extra_origins: Vec<String>,
    /// Use this pairing code instead of a random one (the desktop app hands
    /// it straight to its own board).
    pub code: Option<String>,
}

/// A running relay.
#[derive(Clone, Debug)]
pub struct Started {
    pub live: Arc<LiveHub>,
    /// Live channel port on 127.0.0.1.
    pub port: u16,
    /// The code the teacher types on the board (or the app passes to it).
    pub code: String,
    pub mcp: McpHttp,
}

/// Start the live channel (with chat), the MCP endpoint and the relay.
pub async fn start(opts: AgentOptions) -> anyhow::Result<Started> {
    let mut lc = crate::live::LiveConfig {
        extra_origins: opts.extra_origins.clone(),
        chat: true,
        ..Default::default()
    };
    if let Some(p) = opts.ports.clone() {
        lc.ports = p;
    }
    let live = LiveHub::new(lc);
    let port = live.start().await?;
    let code = match opts.code.clone() {
        Some(c) => {
            live.set_host_code(Some(c.clone()));
            c
        }
        None => live.new_host_code(),
    };
    let ws = crate::workspace::Workspace::new(&opts.workspace)?;
    let server = crate::mcp::McpServer::with_live(crate::tools::Tools::new(ws), live.clone());
    let mcp = crate::mcp_http::serve(server, 0).await?;
    spawn(
        live.clone(),
        ChatConfig {
            cwd: opts.workspace.clone(),
            mcp: mcp.clone(),
            default_agent: opts.default_agent.clone(),
            custom: opts.custom.clone(),
        },
    );
    Ok(Started {
        live,
        port,
        code,
        mcp,
    })
}

/// Commands for a running agent connection.
enum Cmd {
    Prompt(Vec<Value>),
    Cancel,
    NewSession,
    SetMode(String),
    SetModel(String),
    SetConfig { id: String, value: Value },
    ListSessions,
    LoadSession(String),
    Permission { id: u64, option: Option<String> },
}

struct Running {
    id: String,
    tx: mpsc::UnboundedSender<Cmd>,
    task: tokio::task::JoinHandle<()>,
}

/// Board-facing state of the relay.
struct Hub {
    live: Arc<LiveHub>,
    cfg: ChatConfig,
    agents: Vec<AgentInfo>,
    current: Option<Running>,
    /// Last status sent, re-sent when a board (re)pairs.
    status: Value,
}

/// Start relaying `chat` frames of `live` to ACP agents. Runs until the
/// process exits.
pub fn spawn(live: Arc<LiveHub>, cfg: ChatConfig) {
    let (tx, mut rx) = mpsc::unbounded_channel::<(String, Value)>();
    live.set_extension(tx);
    let custom = cfg.custom.clone();
    let agents = agents::detect(custom.as_ref().map(|(l, c)| (l.as_str(), c.as_str())));
    let mut hub = Hub {
        live,
        cfg,
        agents,
        current: None,
        status: json!({"t": "status", "state": "idle"}),
    };
    tokio::spawn(async move {
        let (self_tx, mut self_rx) = mpsc::unbounded_channel::<Value>();
        loop {
            tokio::select! {
                Some((ch, frame)) = rx.recv() => hub.board_frame(&ch, frame, &self_tx),
                Some(status) = self_rx.recv() => hub.agent_status(status),
                else => break,
            }
        }
    });
}

impl Hub {
    fn send(&self, frame: Value) {
        self.live.send_extension("chat", frame);
    }

    fn hello(&self) {
        let current = self.current.as_ref().map(|r| r.id.clone());
        self.send(json!({
            "t": "hello",
            "agents": self.agents,
            "current": current,
            "default": agents::pick(&self.agents, self.cfg.default_agent.as_deref()).map(|a| a.id.clone()),
        }));
        self.send(self.status.clone());
    }

    /// A status from an agent task: remember it and pass it on.
    fn agent_status(&mut self, status: Value) {
        if status["t"] == "status" {
            if status["state"] == "offline" || status["state"] == "error" {
                // The task ended (or will); a new prompt restarts it.
                if let Some(r) = &self.current
                    && status["agent"] == r.id.as_str()
                    && r.task.is_finished()
                {
                    self.current = None;
                }
            }
            self.status = status.clone();
        }
        self.send(status);
    }

    fn board_frame(&mut self, ch: &str, f: Value, me: &mpsc::UnboundedSender<Value>) {
        if ch == "session" {
            if f["t"] == "paired" {
                self.hello();
            }
            return;
        }
        if ch != "chat" {
            return;
        }
        match f["t"].as_str().unwrap_or("") {
            "hello" | "agents" => {
                let custom = self.cfg.custom.clone();
                self.agents =
                    agents::detect(custom.as_ref().map(|(l, c)| (l.as_str(), c.as_str())));
                self.hello();
            }
            "start" => {
                let id = f["agent"].as_str().map(str::to_string);
                self.start(id.as_deref(), me);
            }
            "prompt" => {
                let text = f["text"].as_str().unwrap_or("").trim().to_string();
                if text.is_empty() {
                    return;
                }
                let blocks = prompt_blocks(&text, &f["context"]);
                let wanted = f["agent"].as_str().map(str::to_string);
                let restart = match (&self.current, &wanted) {
                    (None, _) => true,
                    (Some(r), Some(w)) => &r.id != w,
                    _ => false,
                };
                if restart {
                    self.start(wanted.as_deref(), me);
                }
                if let Some(r) = &self.current {
                    let _ = r.tx.send(Cmd::Prompt(blocks));
                }
            }
            "cancel" => {
                if let Some(r) = &self.current {
                    let _ = r.tx.send(Cmd::Cancel);
                }
            }
            "new" => {
                if let Some(r) = &self.current {
                    let _ = r.tx.send(Cmd::NewSession);
                }
            }
            "stop" => self.stop(),
            "set_mode" => {
                if let (Some(r), Some(m)) = (&self.current, f["mode"].as_str()) {
                    let _ = r.tx.send(Cmd::SetMode(m.to_string()));
                }
            }
            "set_model" => {
                if let (Some(r), Some(m)) = (&self.current, f["model"].as_str()) {
                    let _ = r.tx.send(Cmd::SetModel(m.to_string()));
                }
            }
            "set_config" => {
                if let (Some(r), Some(id)) = (&self.current, f["id"].as_str()) {
                    let _ = r.tx.send(Cmd::SetConfig {
                        id: id.to_string(),
                        value: f["value"].clone(),
                    });
                }
            }
            "list_sessions" => {
                if let Some(r) = &self.current {
                    let _ = r.tx.send(Cmd::ListSessions);
                }
            }
            "load_session" => {
                let wanted = f["agent"].as_str().map(str::to_string);
                if self
                    .current
                    .as_ref()
                    .is_none_or(|r| wanted.as_ref().is_some_and(|w| &r.id != w))
                {
                    self.start(wanted.as_deref(), me);
                }
                if let (Some(r), Some(sid)) = (&self.current, f["session"].as_str()) {
                    let _ = r.tx.send(Cmd::LoadSession(sid.to_string()));
                }
            }
            "permission_reply" => {
                if let (Some(r), Some(id)) = (&self.current, f["id"].as_u64()) {
                    let option = f["option"].as_str().map(str::to_string);
                    let _ = r.tx.send(Cmd::Permission { id, option });
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self) {
        if let Some(r) = self.current.take() {
            r.task.abort(); // drops the connection: the agent process is killed
            let label = self.label(&r.id);
            self.agent_status(
                json!({"t": "status", "state": "idle", "agent": r.id, "label": label}),
            );
        }
    }

    fn label(&self, id: &str) -> String {
        self.agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.label.clone())
            .unwrap_or_else(|| id.to_string())
    }

    fn start(&mut self, wanted: Option<&str>, me: &mpsc::UnboundedSender<Value>) {
        self.stop();
        let wanted = wanted.or(self.cfg.default_agent.as_deref());
        let Some(info) = agents::pick(&self.agents, wanted).cloned() else {
            self.agent_status(json!({
                "t": "status", "state": "error", "code": "not_installed",
                "error": "No AI assistant is installed on this computer.",
            }));
            return;
        };
        if !info.available {
            self.agent_status(json!({
                "t": "status", "state": "error", "code": "not_installed", "agent": info.id,
                "label": info.label, "install": info.install,
                "error": format!("{} is not installed on this computer.", info.label),
            }));
            return;
        }
        self.live.set_agent(&info.label);
        let (tx, rx) = mpsc::unbounded_channel();
        let out = Out {
            live: self.live.clone(),
            status: me.clone(),
            agent: info.id.clone(),
            label: info.label.clone(),
        };
        out.status("starting", json!({}));
        let task = tokio::spawn(run_agent(info.clone(), self.cfg.clone(), rx, out));
        self.current = Some(Running {
            id: info.id,
            tx,
            task,
        });
    }
}

/// The teacher's message plus the context they attached (see docs/ai-agents.md
/// § "`chat`"): board JSON, pictures, lessons from the Library, local files.
fn prompt_blocks(text: &str, ctx: &Value) -> Vec<Value> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut blocks = vec![json!({"type": "text", "text": text})];
    let Some(items) = ctx.as_array() else {
        return blocks;
    };
    for c in items {
        let uri = c["uri"].as_str().unwrap_or("rupok://board").to_string();
        match c["kind"].as_str() {
            Some("page" | "selection" | "object" | "json") => {
                let mime = c["mime"].as_str().unwrap_or("application/json");
                let body = c["text"].as_str().unwrap_or("");
                blocks.push(json!({"type": "resource", "resource": {"uri": uri, "mimeType": mime, "text": body}}));
            }
            Some("image") => {
                if let Some(data) = c["data"].as_str() {
                    let mime = c["mime"].as_str().unwrap_or("image/png");
                    blocks.push(json!({"type": "image", "mimeType": mime, "data": data}));
                }
            }
            Some("lesson") => {
                // A lesson from the teacher's Library: send its outline, not the zip.
                let name = c["name"].as_str().unwrap_or("lesson");
                let outline = c["data"]
                    .as_str()
                    .and_then(|d| b64.decode(d).ok())
                    .and_then(|bytes| rupok_format::Package::read(&bytes).ok())
                    .and_then(|pkg| {
                        let lesson = pkg.lesson().ok()?;
                        crate::ops::outline_of(&pkg, &lesson).ok()
                    })
                    .and_then(|o| serde_json::to_string(&o).ok());
                match outline {
                    Some(o) => blocks.push(json!({"type": "resource", "resource": {
                        "uri": format!("rupok://library/{}", uri_part(name)), "mimeType": "application/json", "text": o}})),
                    None => blocks.push(json!({"type": "text", "text": format!("(The lesson \"{name}\" could not be read.)")})),
                }
            }
            Some("file") => {
                let (Some(data), name) = (c["data"].as_str(), c["name"].as_str().unwrap_or("file"))
                else {
                    continue;
                };
                let mime = c["mime"].as_str().unwrap_or("application/octet-stream");
                let furi = format!("file:///{}", uri_part(name));
                let texty = mime.starts_with("text/")
                    || matches!(
                        mime,
                        "application/json" | "application/xml" | "image/svg+xml"
                    );
                if mime.starts_with("image/") && mime != "image/svg+xml" {
                    blocks.push(json!({"type": "image", "mimeType": mime, "data": data}));
                } else if let (true, Some(t)) = (
                    texty,
                    b64.decode(data)
                        .ok()
                        .and_then(|b| String::from_utf8(b).ok()),
                ) {
                    blocks.push(json!({"type": "resource", "resource": {"uri": furi, "mimeType": mime, "text": t}}));
                } else {
                    blocks.push(json!({"type": "resource", "resource": {"uri": furi, "mimeType": mime, "blob": data}}));
                }
            }
            _ => {}
        }
    }
    blocks
}

fn uri_part(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-._/".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// What the agent accepts in prompts (`initialize` → `promptCapabilities`).
#[derive(Clone, Copy, Debug, Default)]
struct PromptCaps {
    images: bool,
    embedded: bool,
}

/// Turn blocks the agent cannot take into text it can: embedded resources
/// become fenced text, pictures and binary files a short note.
fn adapt(blocks: Vec<Value>, caps: PromptCaps) -> Vec<Value> {
    blocks
        .into_iter()
        .map(|b| match b["type"].as_str() {
            Some("resource") if !caps.embedded => {
                let r = &b["resource"];
                let uri = r["uri"].as_str().unwrap_or("");
                match r["text"].as_str() {
                    Some(t) => json!({"type": "text", "text": format!("Context from {uri}:\n```\n{t}\n```")}),
                    None => json!({"type": "text", "text": format!("(The file {uri} was not sent: this assistant does not accept attached files.)")}),
                }
            }
            Some("image") if !caps.images => {
                json!({"type": "text", "text": "(A picture was attached, but this assistant does not take pictures.)"})
            }
            _ => b,
        })
        .collect()
}

/// Where an agent task reports: chat frames to the board, statuses to the hub.
#[derive(Clone)]
struct Out {
    live: Arc<LiveHub>,
    status: mpsc::UnboundedSender<Value>,
    agent: String,
    label: String,
}

impl Out {
    fn frame(&self, f: Value) {
        self.live.send_extension("chat", f);
    }
    fn status(&self, state: &str, extra: Value) {
        let mut v =
            json!({"t": "status", "state": state, "agent": self.agent, "label": self.label});
        if let (Some(m), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, x) in e {
                m.insert(k.clone(), x.clone());
            }
        }
        let _ = self.status.send(v);
    }
}

fn cast<T: DeserializeOwned>(v: Value) -> Result<T, agent_client_protocol::Error> {
    serde_json::from_value(v).map_err(|e| {
        agent_client_protocol::Error::new(
            agent_client_protocol::ErrorCode::INTERNAL_ERROR,
            e.to_string(),
        )
    })
}

type Permissions = Arc<Mutex<HashMap<u64, Responder<RequestPermissionResponse>>>>;

/// Classify an agent error for the board: login needed, or something else.
fn describe(err: &str, info: &AgentInfo) -> (String, Value) {
    let low = err.to_ascii_lowercase();
    if low.contains("auth")
        || low.contains("login")
        || low.contains("log in")
        || low.contains("api key")
    {
        (
            "login".into(),
            json!({"error": format!("{} needs you to log in once.", info.label), "login": info.login, "detail": err}),
        )
    } else if low.contains("no such file")
        || low.contains("not found")
        || low.contains("cannot find")
    {
        (
            "not_installed".into(),
            json!({"error": format!("{} could not be started.", info.label), "install": info.install, "detail": err}),
        )
    } else {
        let short: String = err.chars().take(400).collect();
        (
            "crashed".into(),
            json!({"error": format!("{} stopped: {short}", info.label), "detail": err}),
        )
    }
}

/// A raw ACP request: methods and fields beyond the typed v1 schema
/// (`session/list`, `session/load`, `session/set_config_option`,
/// `session/set_model`) travel as JSON, and replies come back whole.
async fn raw(
    cx: &ConnectionTo<Agent>,
    method: &str,
    params: Value,
) -> Result<Value, agent_client_protocol::Error> {
    cx.send_request(agent_client_protocol::UntypedMessage::new(method, params)?)
        .block_task()
        .await
}

/// The live session: its id and what the agent offers for it.
#[derive(Clone, Debug, Default)]
struct Sess {
    id: String,
    /// `SessionModeState` (`currentModeId`, `availableModes`).
    modes: Value,
    /// Unstable `SessionModelState` (`currentModelId`, `availableModels`), if sent.
    models: Value,
    /// `SessionConfigOption`s (a `category: "model"` one is the model picker).
    config: Value,
}

impl Sess {
    fn from_reply(id: String, r: &Value) -> Self {
        Self {
            id,
            modes: r["modes"].clone(),
            models: r["models"].clone(),
            config: r.get("configOptions").cloned().unwrap_or(Value::Null),
        }
    }
    /// The config option that picks the model, if the agent has one.
    fn model_option(&self) -> Option<String> {
        self.config
            .as_array()?
            .iter()
            .find(|o| o["category"] == "model")
            .and_then(|o| o["id"].as_str().map(str::to_string))
    }
}

async fn run_agent(
    info: AgentInfo,
    cfg: ChatConfig,
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    out: Out,
) {
    let Some((bin, args)) = info.command.split_first() else {
        return;
    };
    let agent = AcpAgent::new(
        AcpAgentConfig::new(bin.clone())
            .args(args.iter().cloned())
            // Agents may write files; keep them in the lessons folder.
            .env("RUPOK_WORKSPACE", cfg.cwd.to_string_lossy().into_owned())
            // Its own folder first on PATH (an app's PATH is often short, and
            // adapters like `npx …` need node next to them).
            .env("PATH", child_path(bin)),
    );
    let perms: Permissions = Arc::new(Mutex::new(HashMap::new()));
    let perms_after = perms.clone();
    let next_perm = Arc::new(std::sync::atomic::AtomicU64::new(1));

    // The agent's latest slash commands, repeated in every status so a board
    // that (re)opens the panel later still has them.
    let commands = Arc::new(Mutex::new(Value::Null));
    let cmds_in = commands.clone();
    let o1 = out.clone();
    let o2 = out.clone();
    let p2 = perms.clone();
    let o3 = out.clone();
    let info2 = info.clone();
    let result = Client
        .v1()
        .name("rupok")
        .on_receive_notification(
            async move |n: SessionNotification, _cx| {
                let update = serde_json::to_value(&n.update).unwrap_or(Value::Null);
                if update["sessionUpdate"] == "available_commands_update" {
                    *cmds_in.lock().unwrap_or_else(|p| p.into_inner()) = update["availableCommands"].clone();
                }
                o1.frame(json!({"t": "update", "session": n.session_id.to_string(), "update": update}));
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |req: RequestPermissionRequest, responder: Responder<RequestPermissionResponse>, _cx| {
                let id = next_perm.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                p2.lock().unwrap_or_else(|p| p.into_inner()).insert(id, responder);
                o2.frame(json!({
                    "t": "permission", "id": id,
                    "tool_call": serde_json::to_value(&req.tool_call).unwrap_or(Value::Null),
                    "options": serde_json::to_value(&req.options).unwrap_or(Value::Null),
                }));
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, async move |cx: ConnectionTo<Agent>| {
            let out = o3;
            let info = info2;
            let init: InitializeResponse = cx
                .send_request(cast::<InitializeRequest>(json!({
                    "protocolVersion": 1,
                    "clientCapabilities": {},
                    "clientInfo": {"name": "rupok", "title": "Rupok", "version": env!("CARGO_PKG_VERSION")},
                }))?)
                .block_task()
                .await?;
            let caps = serde_json::to_value(&init.agent_capabilities).unwrap_or(Value::Null);
            let pcaps = PromptCaps {
                images: caps["promptCapabilities"]["image"].as_bool().unwrap_or(false),
                embedded: caps["promptCapabilities"]["embeddedContext"].as_bool().unwrap_or(false),
            };
            let can_load = caps["loadSession"].as_bool().unwrap_or(false);
            let can_list = caps["sessionCapabilities"]["list"].is_object();
            let agent_caps = json!({
                "images": pcaps.images, "embedded": pcaps.embedded,
                "audio": caps["promptCapabilities"]["audio"].as_bool().unwrap_or(false),
                "load": can_load, "list": can_list,
                "http_mcp": caps["mcpCapabilities"]["http"].as_bool().unwrap_or(false),
            });
            let agent_title = serde_json::to_value(&init.agent_info)
                .ok()
                .and_then(|v| v["title"].as_str().or(v["name"].as_str()).map(str::to_string));
            let mcp = json!([{
                "type": "http", "name": "rupok", "url": cfg.mcp.url,
                "headers": [{"name": "Authorization", "value": cfg.mcp.bearer()}],
            }]);
            let meta = |s: &Sess, extra: Value| {
                let cmds = commands.lock().unwrap_or_else(|p| p.into_inner()).clone();
                let mut v = json!({
                    "session": s.id, "caps": agent_caps, "images": pcaps.images, "commands": cmds,
                    "modes": s.modes, "models": s.models, "config": s.config, "agent_title": agent_title,
                });
                if let (Some(m), Some(e)) = (v.as_object_mut(), extra.as_object()) {
                    m.extend(e.clone());
                }
                v
            };
            let new_session = async |cx: &ConnectionTo<Agent>| -> Result<Sess, agent_client_protocol::Error> {
                let r = raw(cx, "session/new", json!({"cwd": cfg.cwd, "mcpServers": mcp})).await?;
                let id = r["sessionId"].as_str().unwrap_or_default().to_string();
                Ok(Sess::from_reply(id, &r))
            };
            let mut sess = new_session(&cx).await?;
            out.status("ready", meta(&sess, json!({})));

            // One prompt at a time; commands keep flowing while it runs.
            let (done_tx, mut done_rx) = mpsc::unbounded_channel::<(String, Result<Value, String>)>();
            let mut busy = false;
            let mut queued: Vec<Vec<Value>> = Vec::new();
            loop {
                if !busy && let Some(blocks) = (!queued.is_empty()).then(|| queued.remove(0)) {
                    busy = true;
                    out.status("busy", meta(&sess, json!({})));
                    let cx2 = cx.clone();
                    let tx = done_tx.clone();
                    let sid = sess.id.clone();
                    let params = json!({"sessionId": sid, "prompt": adapt(blocks, pcaps)});
                    tokio::spawn(async move {
                        let r = raw(&cx2, "session/prompt", params).await.map_err(|e| e.to_string());
                        let _ = tx.send((sid, r));
                    });
                }
                let state = if busy { "busy" } else { "ready" };
                tokio::select! {
                    cmd = rx.recv() => match cmd {
                        None => break,
                        Some(Cmd::Prompt(b)) => queued.push(b),
                        Some(Cmd::Cancel) => {
                            queued.clear();
                            cancel_permissions(&perms);
                            cx.send_notification(cast::<CancelNotification>(json!({"sessionId": sess.id}))?)?;
                        }
                        Some(Cmd::NewSession) => {
                            queued.clear();
                            cancel_permissions(&perms);
                            if busy {
                                let _ = cx.send_notification(cast::<CancelNotification>(json!({"sessionId": sess.id}))?);
                            }
                            sess = new_session(&cx).await?;
                            busy = false;
                            out.frame(json!({"t": "cleared", "session": sess.id}));
                            out.status("ready", meta(&sess, json!({})));
                        }
                        Some(Cmd::LoadSession(sid)) => {
                            if !can_load {
                                out.frame(json!({"t": "notice", "error": format!("{} cannot reopen earlier chats.", info.label)}));
                                continue;
                            }
                            queued.clear();
                            cancel_permissions(&perms);
                            if busy {
                                let _ = cx.send_notification(cast::<CancelNotification>(json!({"sessionId": sess.id}))?);
                                busy = false;
                            }
                            // The agent replays the conversation as session updates, then answers.
                            out.frame(json!({"t": "loading", "session": sid}));
                            match raw(&cx, "session/load", json!({"sessionId": sid, "cwd": cfg.cwd, "mcpServers": mcp})).await {
                                Ok(r) => {
                                    sess = Sess::from_reply(sid.clone(), &r);
                                    out.frame(json!({"t": "loaded", "session": sid}));
                                    out.status("ready", meta(&sess, json!({})));
                                }
                                Err(e) => out.frame(json!({"t": "notice", "error": format!("Could not reopen that chat: {e}")})),
                            }
                        }
                        Some(Cmd::ListSessions) => {
                            let list = if can_list {
                                raw(&cx, "session/list", json!({"cwd": cfg.cwd})).await
                                    .map(|r| r["sessions"].clone()).unwrap_or(json!([]))
                            } else {
                                json!([])
                            };
                            out.frame(json!({"t": "sessions", "agent": info.id, "sessions": list, "supported": can_list}));
                        }
                        Some(Cmd::SetMode(mode)) => {
                            match raw(&cx, "session/set_mode", json!({"sessionId": sess.id, "modeId": mode})).await {
                                Ok(_) => {
                                    if sess.modes.is_object() {
                                        sess.modes["currentModeId"] = json!(mode);
                                    }
                                    out.status(state, meta(&sess, json!({})));
                                }
                                Err(e) => out.frame(json!({"t": "notice", "error": format!("Could not switch mode: {e}")})),
                            }
                        }
                        Some(Cmd::SetModel(model)) => {
                            let r = match sess.model_option() {
                                Some(cid) => raw(&cx, "session/set_config_option", json!({"sessionId": sess.id, "configId": cid, "value": model})).await,
                                None => raw(&cx, "session/set_model", json!({"sessionId": sess.id, "modelId": model})).await,
                            };
                            match r {
                                Ok(reply) => {
                                    if let Some(c) = reply.get("configOptions").filter(|c| c.is_array()) {
                                        sess.config = c.clone();
                                    } else if let Some(cid) = sess.model_option()
                                        && let Some(arr) = sess.config.as_array_mut()
                                    {
                                        for o in arr.iter_mut().filter(|o| o["id"] == cid.as_str()) {
                                            o["currentValue"] = json!(model);
                                        }
                                    }
                                    if sess.models.is_object() {
                                        sess.models["currentModelId"] = json!(model);
                                    }
                                    out.status(state, meta(&sess, json!({})));
                                }
                                Err(e) => out.frame(json!({"t": "notice", "error": format!("Could not switch model: {e}")})),
                            }
                        }
                        Some(Cmd::SetConfig { id, value }) => {
                            match raw(&cx, "session/set_config_option", json!({"sessionId": sess.id, "configId": id, "value": value})).await {
                                Ok(reply) => {
                                    if let Some(c) = reply.get("configOptions").filter(|c| c.is_array()) {
                                        sess.config = c.clone();
                                    }
                                    out.status(state, meta(&sess, json!({})));
                                }
                                Err(e) => out.frame(json!({"t": "notice", "error": format!("Could not change that setting: {e}")})),
                            }
                        }
                        Some(Cmd::Permission { id, option }) => {
                            let r = perms.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                            if let Some(responder) = r {
                                let outcome = match option {
                                    Some(o) => json!({"outcome": "selected", "optionId": o}),
                                    None => json!({"outcome": "cancelled"}),
                                };
                                responder.respond(cast(json!({"outcome": outcome}))?)?;
                            }
                        }
                    },
                    Some((sid, r)) = done_rx.recv() => {
                        if sid != sess.id { continue } // a turn of an old session
                        busy = false;
                        match r {
                            Ok(resp) => {
                                out.frame(json!({"t": "done", "session": sid, "stop_reason": resp["stopReason"], "usage": resp.get("usage")}));
                                out.status("ready", meta(&sess, json!({})));
                            }
                            Err(e) => {
                                let (code, mut extra) = describe(&e, &info);
                                out.frame(json!({"t": "done", "session": sid, "stop_reason": "error", "error": extra["error"]}));
                                if code == "login" {
                                    extra["code"] = json!(code);
                                    out.status("error", extra);
                                } else {
                                    out.status("ready", meta(&sess, json!({"warning": extra["error"]})));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        })
        .await;
    cancel_permissions(&perms_after);
    match result {
        Ok(()) => out.status("offline", json!({})),
        Err(e) => {
            let (code, mut extra) = describe(&e.to_string(), &info);
            extra["code"] = json!(code);
            out.status("error", extra);
        }
    }
}

fn child_path(bin: &str) -> String {
    let mut dirs: Vec<PathBuf> = std::path::Path::new(bin)
        .parent()
        .map(|p| vec![p.to_path_buf()])
        .unwrap_or_default();
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(dirs)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn cancel_permissions(perms: &Permissions) {
    let pending: Vec<_> = perms
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .drain()
        .collect();
    for (_, r) in pending {
        if let Ok(resp) =
            cast::<RequestPermissionResponse>(json!({"outcome": {"outcome": "cancelled"}}))
        {
            let _ = r.respond(resp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_blocks_carry_board_context() {
        let b = prompt_blocks(
            "make a quiz",
            &json!([
                {"kind": "page", "uri": "rupok://board/page/3", "text": "{\"items\":[]}"},
                {"kind": "image", "data": "iVBORw0K"},
                {"kind": "bogus"}
            ]),
        );
        assert_eq!(b.len(), 3);
        assert_eq!(b[0]["text"], "make a quiz");
        assert_eq!(b[1]["resource"]["uri"], "rupok://board/page/3");
        assert_eq!(b[2]["type"], "image");
    }

    #[test]
    fn blocks_adapt_to_what_the_agent_takes() {
        let b = prompt_blocks(
            "look",
            &json!([
                {"kind": "page", "uri": "rupok://board/page/1", "text": "{}"},
                {"kind": "file", "name": "notes.txt", "mime": "text/plain", "data": "aGVsbG8="},
                {"kind": "file", "name": "a.pdf", "mime": "application/pdf", "data": "JVBERg=="},
                {"kind": "lesson", "name": "Bad", "data": "bm9wZQ=="},
                {"kind": "image", "data": "iVBORw0K"}
            ]),
        );
        assert_eq!(b[2]["resource"]["text"], "hello");
        assert_eq!(b[3]["resource"]["blob"], "JVBERg==");
        assert!(b[4]["text"].as_str().unwrap().contains("could not be read"));
        let plain = adapt(b.clone(), PromptCaps::default());
        assert!(plain.iter().all(|x| x["type"] == "text"));
        assert!(
            plain[1]["text"]
                .as_str()
                .unwrap()
                .starts_with("Context from rupok://board/page/1")
        );
        let full = adapt(
            b,
            PromptCaps {
                images: true,
                embedded: true,
            },
        );
        assert_eq!(full[5]["type"], "image");
    }

    #[test]
    fn errors_are_classified() {
        let info = agents::detect(Some(("X", "x")))[0].clone();
        assert_eq!(describe("Authentication required", &info).0, "login");
        assert_eq!(
            describe("No such file or directory (os error 2)", &info).0,
            "not_installed"
        );
        assert_eq!(describe("exit status 1", &info).0, "crashed");
    }
}
