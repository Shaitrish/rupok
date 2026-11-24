//! The in-app chat end to end: `chat::start` (live channel with chat, MCP
//! over loopback HTTP, ACP relay) driving `tests/fake-acp-agent.mjs`, and a
//! simulated board on a real WebSocket that pairs with the relay's code,
//! chats, answers a permission request and runs the agent's ops on its
//! lesson exactly as the wasm board does. Needs Node.js; skipped without it.

use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use futures_util::{SinkExt, StreamExt};
use rupok_cli::chat::{AgentOptions, Started, start};
use rupok_core::{ItemData, Lesson};
use rupok_format::Package;
use rupok_live::{BoardCtx, Op};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
const WAIT: Duration = Duration::from_secs(20);

fn node() -> Option<String> {
    let ok = std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    ok.then(|| "node".to_string())
}

fn fake_agent() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake-acp-agent.mjs")
        .to_string_lossy()
        .into_owned()
}

async fn relay(ws_dir: &std::path::Path, command: String) -> Started {
    start(AgentOptions {
        workspace: ws_dir.to_path_buf(),
        default_agent: Some("custom".into()),
        custom: Some(("Fake Agent".into(), command)),
        ports: Some(vec![0]),
        ..Default::default()
    })
    .await
    .expect("relay starts")
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

async fn recv(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(WAIT, ws.next())
            .await
            .expect("timed out waiting for the relay")
        {
            Some(Ok(Message::Text(t))) => return Some(serde_json::from_str(t.as_str()).unwrap()),
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return None,
            Some(Ok(_)) => {}
        }
    }
}

/// A board: lesson, package, socket.
struct Board {
    ws: Ws,
    pkg: Package,
    lesson: Lesson,
    page: String,
    chat: Vec<Value>,
    ops: Vec<String>,
}

impl Board {
    async fn open(port: u16) -> Self {
        let mut req = format!("ws://127.0.0.1:{port}")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("Origin", "http://127.0.0.1:3737".parse().unwrap());
        let (mut ws, _) = connect_async(req).await.expect("connect");
        let lesson = Lesson::new("Physics", 1_000.0);
        let pkg = Package::from_lesson(&lesson, "test", 1_000.0);
        let page = lesson.page_id_at(0).unwrap();
        send(
            &mut ws,
            json!({"ch": "session", "t": "hello", "v": 1, "client": "test-board",
                   "channels": ["session", "doc", "tools", "chat"],
                   "doc": {"key": rupok_live::doc_key(&lesson), "title": "Physics"}}),
        )
        .await;
        Self {
            ws,
            pkg,
            lesson,
            page,
            chat: Vec::new(),
            ops: Vec::new(),
        }
    }

    /// Pair with the relay's code and answer the first doc sync.
    async fn pair(&mut self, code: &str) {
        let w = self.next_on("session").await;
        assert_eq!(w["t"], "welcome");
        assert_eq!(w["pair"], "board", "the relay asks the board for its code");
        assert_eq!(w["chat"], true);
        send(
            &mut self.ws,
            json!({"ch": "session", "t": "pair", "code": code}),
        )
        .await;
        let p = self.next_on("session").await;
        assert_eq!(p["t"], "paired", "{p}");
        assert!(
            p["channels"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c == "chat")
        );
        self.pkg.update_lesson(&self.lesson, "test", 2_000.0);
        let bytes = self.pkg.write().unwrap();
        let key = rupok_live::doc_key(&self.lesson);
        send(
            &mut self.ws,
            json!({"ch": "doc", "t": "snapshot", "data": B64.encode(bytes), "doc": {"key": key, "title": "Physics"}}),
        )
        .await;
        send(&mut self.ws, json!({"ch": "session", "t": "state", "page": 1, "page_count": 1, "allow_edit": true, "title": "Physics"})).await;
    }

    async fn next_on(&mut self, ch: &str) -> Value {
        loop {
            let f = recv(&mut self.ws)
                .await
                .unwrap_or_else(|| panic!("closed waiting for {ch}"));
            if f["ch"] == ch {
                return f;
            }
            self.handle(f).await;
        }
    }

    /// Serve tool ops; record chat frames.
    async fn handle(&mut self, f: Value) {
        if f["ch"] == "tools" && f["t"] == "op" {
            let id = f["id"].as_u64().unwrap();
            let op: Op = serde_json::from_value(f["op"].clone()).unwrap();
            self.ops.push(f["op"]["op"].as_str().unwrap().to_string());
            let out = rupok_live::apply(
                BoardCtx {
                    pkg: &mut self.pkg,
                    lesson: &mut self.lesson,
                    page_id: &self.page.clone(),
                    selection: &[],
                    read_only: false,
                },
                &op,
                f["agent"].as_str().unwrap(),
                3_000.0,
            );
            if let (Op::GoToPage { page }, Ok(_)) = (&op, &out) {
                self.page = self.lesson.page_id_at(*page as usize - 1).unwrap();
            }
            let reply = match out {
                Ok(o) => {
                    json!({"ch": "tools", "t": "result", "id": id, "ok": true, "result": o.result})
                }
                Err(e) => {
                    json!({"ch": "tools", "t": "result", "id": id, "ok": false, "error": e.to_json()["error"]})
                }
            };
            send(&mut self.ws, reply).await;
        } else if f["ch"] == "chat" {
            self.chat.push(f);
        }
    }

    /// Run until a chat frame matches `pred` (serving ops meanwhile).
    async fn until_chat(&mut self, pred: impl Fn(&Value) -> bool) -> Value {
        if let Some(f) = self.chat.iter().find(|f| pred(f)) {
            return f.clone();
        }
        loop {
            let f = recv(&mut self.ws).await.expect("closed");
            let is_chat = f["ch"] == "chat";
            self.handle(f.clone()).await;
            if is_chat && pred(&f) {
                return f;
            }
        }
    }

    fn text(&self) -> String {
        self.chat
            .iter()
            .filter(|f| f["t"] == "update" && f["update"]["sessionUpdate"] == "agent_message_chunk")
            .map(|f| {
                f["update"]["content"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    }
}

#[tokio::test]
async fn chat_builds_a_class_on_the_board() {
    let Some(node) = node() else {
        eprintln!("skipped: node is not installed");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let r = relay(dir.path(), format!("{node} {}", fake_agent())).await;
    let mut b = Board::open(r.port).await;

    // A wrong code is refused; the right one pairs.
    let w = b.next_on("session").await;
    assert_eq!(w["pair"], "board");
    let wrong = if r.code == "000000" {
        "111111"
    } else {
        "000000"
    };
    send(
        &mut b.ws,
        json!({"ch": "session", "t": "pair", "code": wrong}),
    )
    .await;
    let e = b.next_on("session").await;
    assert_eq!(e["code"], "bad_code", "{e}");
    send(
        &mut b.ws,
        json!({"ch": "session", "t": "pair", "code": r.code}),
    )
    .await;
    assert_eq!(b.next_on("session").await["t"], "paired");
    b.pkg.update_lesson(&b.lesson, "test", 2_000.0);
    let bytes = b.pkg.write().unwrap();
    let key = rupok_live::doc_key(&b.lesson);
    send(&mut b.ws, json!({"ch": "doc", "t": "snapshot", "data": B64.encode(bytes), "doc": {"key": key, "title": "Physics"}})).await;

    // The relay introduces the agents.
    let hello = b.until_chat(|f| f["t"] == "hello").await;
    assert_eq!(hello["agents"][0]["id"], "custom");
    assert_eq!(hello["agents"][0]["label"], "Fake Agent");
    assert!(
        hello["agents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == "claude")
    );

    // Prompt: the agent starts, streams, asks permission.
    send(&mut b.ws, json!({"ch": "chat", "t": "prompt", "text": "Make a class on Ohm's law",
                           "context": [{"kind": "page", "uri": "rupok://board/page/1", "text": "{\"items\":[]}"}]})).await;
    let ready = b
        .until_chat(|f| f["t"] == "status" && f["state"] == "ready")
        .await;
    assert_eq!(ready["agent"], "custom");
    assert_eq!(ready["modes"]["availableModes"][1]["id"], "acceptEdits");
    assert_eq!(ready["config"][0]["category"], "model");
    assert_eq!(ready["models"]["currentModelId"], "sonnet");
    assert_eq!(ready["caps"]["load"], true);
    assert_eq!(ready["caps"]["list"], true);
    let cmds = b
        .until_chat(|f| f["update"]["sessionUpdate"] == "available_commands_update")
        .await;
    assert_eq!(cmds["update"]["availableCommands"][0]["name"], "review");
    let perm = b.until_chat(|f| f["t"] == "permission").await;
    assert_eq!(perm["tool_call"]["title"], "Add a page to your Rupok board");
    let kinds: Vec<&str> = perm["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["allow_once", "allow_always", "reject_once"]);
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "permission_reply", "id": perm["id"], "option": "always"}),
    )
    .await;

    // It builds on the board through the MCP tools, then finishes.
    let done = b.until_chat(|f| f["t"] == "done").await;
    assert_eq!(done["stop_reason"], "end_turn", "{done}");
    assert_eq!(
        b.ops,
        [
            "add_page_after_current",
            "add_smart_object",
            "add_text",
            "go_to_page"
        ]
    );
    assert_eq!(b.lesson.page_count(), 2);
    let items = b.lesson.items(&b.lesson.page_id_at(1).unwrap()).unwrap();
    assert!(items.iter().any(|i| matches!(i.data, ItemData::Object(_))));
    assert!(items.iter().any(|i| matches!(i.data, ItemData::Text(_))));
    let text = b.text();
    assert!(
        text.contains("Ohm's law") && text.contains("Page 2 has"),
        "{text}"
    );
    let tool_updates: Vec<&Value> = b
        .chat
        .iter()
        .filter(|f| f["t"] == "update" && f["update"]["sessionUpdate"] == "tool_call")
        .collect();
    assert_eq!(tool_updates.len(), 3);
    assert!(
        b.chat
            .iter()
            .any(|f| f["update"]["sessionUpdate"] == "plan")
    );

    // /stamp: custom stamps (SVG with behaviour, a Smart dice), placed, ink
    // drawn and restyled, pages copied and reordered, all through MCP.
    b.chat.clear();
    b.ops.clear();
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "prompt", "text": "/stamp a gold star that sparkles and a dice"}),
    )
    .await;
    let done = b.until_chat(|f| f["t"] == "done").await;
    assert_eq!(done["stop_reason"], "end_turn", "{done} {}", b.text());
    assert_eq!(
        b.ops,
        [
            "create_stamp",
            "create_stamp",
            "add_stamp",
            "add_stamp",
            "draw_stroke",
            "restyle_items",
            "list_stamps",
            "duplicate_pages",
            "move_pages"
        ]
    );
    assert_eq!(b.lesson.stamp_defs().len(), 2);
    assert!(b.lesson.stamp_def("dice").is_some());
    assert_eq!(b.lesson.page_count(), 3);
    assert!(b.text().contains("Dice"), "{}", b.text());

    // Board context reaches the agent as a resource block.
    b.chat.clear();
    send(&mut b.ws, json!({"ch": "chat", "t": "prompt", "text": "What context did you get?",
                           "context": [{"kind": "page", "uri": "rupok://board/page/2", "text": "{\"items\":[1,2]}"}]})).await;
    b.until_chat(|f| f["t"] == "done").await;
    assert!(
        b.text().contains("rupok://board/page/2 with 2 item(s)"),
        "{}",
        b.text()
    );

    // Stop cancels a running turn.
    b.chat.clear();
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "prompt", "text": "be slow"}),
    )
    .await;
    b.until_chat(|f| f["t"] == "update").await;
    send(&mut b.ws, json!({"ch": "chat", "t": "cancel"})).await;
    let done = b.until_chat(|f| f["t"] == "done").await;
    assert_eq!(done["stop_reason"], "cancelled");

    // Mode switch, and a new chat.
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "set_mode", "mode": "acceptEdits"}),
    )
    .await;
    let st = b
        .until_chat(|f| f["t"] == "status" && f["modes"]["currentModeId"] == "acceptEdits")
        .await;
    assert_eq!(st["state"], "ready");
    assert!(
        b.chat
            .iter()
            .any(|f| f["update"]["sessionUpdate"] == "usage_update")
    );

    // Model switch through the agent's model config option.
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "set_model", "model": "opus"}),
    )
    .await;
    b.until_chat(|f| f["t"] == "status" && f["config"][0]["currentValue"] == "opus")
        .await;

    // An agent slash command is sent as prompt text.
    b.chat.clear();
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "prompt", "text": "/review the circuit"}),
    )
    .await;
    b.until_chat(|f| f["t"] == "done").await;
    assert!(
        b.text()
            .contains("Reviewing: the circuit (model opus, mode acceptEdits)"),
        "{}",
        b.text()
    );

    // New chat, then list and reopen the first one: the agent replays it.
    let first = st["session"].as_str().unwrap().to_string();
    send(&mut b.ws, json!({"ch": "chat", "t": "new"})).await;
    b.until_chat(|f| f["t"] == "cleared").await;
    send(&mut b.ws, json!({"ch": "chat", "t": "list_sessions"})).await;
    let list = b.until_chat(|f| f["t"] == "sessions").await;
    assert!(
        list["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["sessionId"] == first.as_str())
    );
    b.chat.clear();
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "load_session", "session": first}),
    )
    .await;
    b.until_chat(|f| f["t"] == "loaded").await;
    assert!(
        b.chat
            .iter()
            .any(|f| f["update"]["sessionUpdate"] == "user_message_chunk"
                && f["update"]["content"]["text"] == "Make a class on Ohm's law")
    );
    let st = b
        .until_chat(|f| f["t"] == "status" && f["state"] == "ready")
        .await;
    assert_eq!(st["session"], first.as_str());

    // Files, lessons and objects as context.
    let mut pkg = b.pkg.clone();
    pkg.update_lesson(&b.lesson, "test", 4_000.0);
    let rpk = B64.encode(pkg.write().unwrap());
    b.chat.clear();
    send(&mut b.ws, json!({"ch": "chat", "t": "prompt", "text": "What context did you get?", "context": [
        {"kind": "object", "uri": "rupok://board/page/2/object/x", "text": "{\"items\":[1]}"},
        {"kind": "lesson", "name": "Physics", "data": rpk},
        {"kind": "file", "name": "worksheet.pdf", "mime": "application/pdf", "data": "JVBERi0xLjQ="},
        {"kind": "image", "data": "iVBORw0KGgo=", "mime": "image/png"}
    ]})).await;
    b.until_chat(|f| f["t"] == "done").await;
    let t = b.text();
    assert!(
        t.contains("rupok://library/Physics") && t.contains("file:///worksheet.pdf (file)"),
        "{t}"
    );
}

#[tokio::test]
async fn login_and_missing_agents_are_explained() {
    let Some(node) = node() else {
        eprintln!("skipped: node is not installed");
        return;
    };
    if cfg!(windows) {
        return; // uses `env`
    }
    let dir = tempfile::tempdir().unwrap();
    let r = relay(
        dir.path(),
        format!("env FAKE_ACP_AUTH=1 {node} {}", fake_agent()),
    )
    .await;
    let mut b = Board::open(r.port).await;
    b.pair(&r.code).await;
    b.until_chat(|f| f["t"] == "hello").await;
    send(
        &mut b.ws,
        json!({"ch": "chat", "t": "prompt", "text": "hello"}),
    )
    .await;
    let st = b
        .until_chat(|f| f["t"] == "status" && f["state"] == "error")
        .await;
    assert_eq!(st["code"], "login", "{st}");
    assert!(st["error"].as_str().unwrap().contains("log in"));

    // A command that does not exist is "not installed".
    let r2 = relay(dir.path(), "rupok-no-such-agent-xyz --acp".into()).await;
    let mut b2 = Board::open(r2.port).await;
    b2.pair(&r2.code).await;
    b2.until_chat(|f| f["t"] == "hello").await;
    send(
        &mut b2.ws,
        json!({"ch": "chat", "t": "start", "agent": "custom"}),
    )
    .await;
    let st = b2
        .until_chat(|f| f["t"] == "status" && f["state"] == "error")
        .await;
    assert_eq!(st["code"], "not_installed", "{st}");
}
