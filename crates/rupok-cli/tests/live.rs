//! The live board channel end to end: a real `LiveHub` WebSocket server on
//! loopback and a simulated board (a `Package` + `Lesson` running agent ops
//! with `rupok_live::apply`, exactly as the wasm board does), speaking the
//! protocol over a real WebSocket.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use futures_util::{SinkExt, StreamExt};
use rupok_cli::live::{LiveConfig, LiveHub};
use rupok_core::{ItemData, Lesson, StrokeData};
use rupok_format::Package;
use rupok_ink::StrokePoint;
use rupok_live::{BoardCtx, Op, sync};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
const ORIGIN: &str = "http://127.0.0.1:3737";
const WAIT: Duration = Duration::from_secs(5);

async fn hub_with(cfg: LiveConfig) -> (Arc<LiveHub>, u16) {
    let hub = LiveHub::new(LiveConfig {
        ports: vec![0],
        ..cfg
    });
    let port = hub.start().await.expect("bind");
    hub.set_agent("Claude Code");
    (hub, port)
}

async fn hub() -> (Arc<LiveHub>, u16) {
    hub_with(LiveConfig::default()).await
}

async fn open(
    port: u16,
    origin: Option<&str>,
    host: Option<&str>,
) -> Result<Ws, tungstenite::Error> {
    let mut req = format!("ws://127.0.0.1:{port}").into_client_request()?;
    if let Some(o) = origin {
        req.headers_mut().insert("Origin", o.parse().unwrap());
    }
    if let Some(h) = host {
        req.headers_mut().insert("Host", h.parse().unwrap());
    }
    connect_async(req).await.map(|(ws, _)| ws)
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

/// The next JSON frame, or `None` when the socket closes.
async fn recv(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(WAIT, ws.next())
            .await
            .expect("timed out waiting for the server")
        {
            Some(Ok(Message::Text(t))) => return Some(serde_json::from_str(t.as_str()).unwrap()),
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return None,
            Some(Ok(_)) => {}
        }
    }
}

/// Receive until a frame of channel `ch` and type `t` arrives.
async fn expect(ws: &mut Ws, ch: &str, t: &str) -> Value {
    loop {
        let f = recv(ws)
            .await
            .unwrap_or_else(|| panic!("closed while waiting for {ch}/{t}"));
        if f["ch"] == ch && f["t"] == t {
            return f;
        }
    }
}

/// A board: the lesson, its package and the protocol client.
struct Board {
    ws: Ws,
    pkg: Package,
    lesson: Lesson,
    page: String,
    sent: Vec<u8>,
}

impl Board {
    fn lesson(title: &str) -> (Package, Lesson) {
        let lesson = Lesson::new(title, 1_000.0);
        (Package::from_lesson(&lesson, "test", 1_000.0), lesson)
    }

    async fn connect(port: u16, pkg: Package, lesson: Lesson, token: Option<&str>) -> Self {
        let mut ws = open(port, Some(ORIGIN), None).await.expect("connect");
        let page = lesson.page_id_at(0).unwrap();
        send(
            &mut ws,
            json!({
                "ch": "session", "t": "hello", "v": 1, "client": "test-board",
                "channels": ["session", "doc", "tools"], "token": token,
                "doc": { "key": rupok_live::doc_key(&lesson), "title": lesson.title() },
            }),
        )
        .await;
        Self {
            ws,
            pkg,
            lesson,
            page,
            sent: Vec::new(),
        }
    }

    fn key(&self) -> String {
        rupok_live::doc_key(&self.lesson)
    }

    /// Answer the server's `doc/sync` like the web app does.
    async fn answer_sync(&mut self, sync_frame: &Value) {
        let (key, title) = (self.key(), self.lesson.title());
        let theirs = sync_frame["key"].as_str();
        let vv = sync_frame["vv"].as_str();
        if let (Some(k), Some(vv)) = (theirs, vv)
            && k == key
        {
            let peer = B64.decode(vv).unwrap();
            let delta = sync::export_since(&self.lesson, &peer).unwrap();
            send(
                &mut self.ws,
                json!({"ch": "doc", "t": "update", "data": B64.encode(delta)}),
            )
            .await;
            let mine = B64.encode(sync::version(&self.lesson));
            send(
                &mut self.ws,
                json!({"ch": "doc", "t": "sync", "key": key, "vv": mine}),
            )
            .await;
        } else {
            self.pkg.update_lesson(&self.lesson, "test", 2_000.0);
            let bytes = self.pkg.write().unwrap();
            send(
                &mut self.ws,
                json!({"ch": "doc", "t": "snapshot", "data": B64.encode(bytes),
                       "doc": {"key": key, "title": title}}),
            )
            .await;
        }
        self.sent = sync::version(&self.lesson);
    }

    async fn push_updates(&mut self) {
        let delta = sync::export_since(&self.lesson, &self.sent).unwrap();
        self.sent = sync::version(&self.lesson);
        send(
            &mut self.ws,
            json!({"ch": "doc", "t": "update", "data": B64.encode(delta)}),
        )
        .await;
    }

    /// Wait for one `tools/op`, run it, stream the update, then answer.
    async fn serve_op(&mut self) -> Value {
        let f = expect(&mut self.ws, "tools", "op").await;
        let id = f["id"].as_u64().unwrap();
        let agent = f["agent"].as_str().unwrap().to_string();
        let op: Op = serde_json::from_value(f["op"].clone()).unwrap();
        let out = rupok_live::apply(
            BoardCtx {
                pkg: &mut self.pkg,
                lesson: &mut self.lesson,
                page_id: &self.page,
                selection: &[],
                read_only: false,
            },
            &op,
            &agent,
            3_000.0,
        );
        match out {
            Ok(o) => {
                if o.changed {
                    self.push_updates().await;
                }
                send(
                    &mut self.ws,
                    json!({"ch": "tools", "t": "result", "id": id, "ok": true, "result": o.result}),
                )
                .await;
            }
            Err(e) => {
                let body = e.to_json();
                send(
                    &mut self.ws,
                    json!({"ch": "tools", "t": "result", "id": id, "ok": false, "error": body["error"], "data": body}),
                )
                .await;
            }
        }
        f
    }

    fn stroke(&mut self) -> String {
        let pts = (0..12)
            .map(|i| StrokePoint {
                x: 300.0 + i as f32 * 6.0,
                y: 400.0,
                p: 0.5,
                dt: i * 8,
            })
            .collect();
        let data = ItemData::Stroke(StrokeData {
            tool: "pen".into(),
            color: 0x1c2024ff,
            width: 4.0,
            opacity: 1.0,
            points: pts,
            offset: [0.0, 0.0],
            shape: None,
            geom: None,
            brush: None,
        });
        self.lesson
            .add_item(&self.page.clone(), &data, 2_500.0)
            .unwrap()
    }
}

/// Pair a fresh board with `hub` by code; returns the board after its snapshot.
async fn paired_board(hub: &LiveHub, port: u16, title: &str) -> (Board, String) {
    let (pkg, lesson) = Board::lesson(title);
    let mut b = Board::connect(port, pkg, lesson, None).await;
    let w = expect(&mut b.ws, "session", "welcome").await;
    assert_eq!(w["agent"], "Claude Code");
    assert_eq!(w["v"], 1);
    let code = w["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 6);
    let st = hub
        .connect(&format!("{} {}", &code[..3], &code[3..]))
        .expect("pair");
    assert!(st.paired);
    let p = expect(&mut b.ws, "session", "paired").await;
    let token = p["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 32);
    let s = expect(&mut b.ws, "doc", "sync").await;
    assert!(
        s["vv"].is_null(),
        "a new lesson: the server asks for a snapshot"
    );
    b.answer_sync(&s).await;
    (b, token)
}

async fn settle<F: Fn() -> bool>(f: F) {
    for _ in 0..100 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn pairing_ops_and_update_exchange() {
    let (hub, port) = hub().await;
    let st = hub.status();
    assert!(st.enabled && !st.paired);
    assert!(st.hint.contains("Connect AI"), "{}", st.hint);

    // no board yet: live calls explain how to connect
    let err = hub.call(Op::GetPage { page: None }).await.unwrap_err();
    assert!(
        err.message.contains("no board is paired"),
        "{}",
        err.message
    );

    let (mut board, _token) = paired_board(&hub, port, "Forces").await;
    settle(|| hub.mirror_version().is_some()).await;

    // the agent adds text: the board runs it, streams the update, answers
    let h = hub.clone();
    let call = tokio::spawn(async move {
        h.call(Op::AddText {
            page: None,
            text: "Newton's third law".into(),
            x: 120.0,
            y: 80.0,
            w: None,
            size: Some(56.0),
            color: None,
        })
        .await
    });
    let f = board.serve_op().await;
    assert_eq!(f["agent"], "Claude Code");
    assert_eq!(f["op"]["op"], "add_text");
    let r = call.await.unwrap().expect("add_text");
    let id = r["item_id"].as_str().unwrap().to_string();

    // the mirror got the board's Loro update, attribution included
    let page = board.page.clone();
    let mirror_has = hub
        .with_mirror(|l| l.items(&page).unwrap().iter().any(|i| i.id == id))
        .unwrap();
    assert!(mirror_has, "mirror merged the board update");
    assert_eq!(
        hub.with_mirror(|l| rupok_live::agent_items(l, &page))
            .unwrap()
            .get(&id)
            .map(String::as_str),
        Some("Claude Code")
    );
    assert_eq!(hub.mirror_version().unwrap(), sync::version(&board.lesson));

    // teacher ink streams to the mirror too
    let s = board.stroke();
    board.push_updates().await;
    settle(|| {
        hub.with_mirror(|l| l.items(&page).unwrap().len() == 2)
            .unwrap()
    })
    .await;
    assert!(hub.with_mirror(|l| l.item(&page, &s).is_ok()).unwrap());

    // a refused op comes back as a structured error
    let h = hub.clone();
    let call = tokio::spawn(async move {
        h.call(Op::AddSmartObject {
            page: None,
            html: r#"<script src="https://cdn.example/x.js"></script>"#.into(),
            title: "Bad".into(),
            width: None,
            height: None,
            steps: None,
            x: None,
            y: None,
            network_hosts: None,
        })
        .await
    });
    board.serve_op().await;
    let e = call.await.unwrap().unwrap_err();
    assert!(e.message.contains("validation"), "{}", e.message);
    assert_eq!(e.to_json()["saved"], false);
    assert!(e.to_json()["diagnostics"].is_array());

    // board state shows up in the status
    send(
        &mut board.ws,
        json!({"ch": "session", "t": "state", "page": 1, "page_count": 1, "allow_edit": false, "title": "Forces"}),
    )
    .await;
    settle(|| hub.status().board.is_some_and(|b| !b.allow_edit)).await;
    assert!(hub.status().hint.contains("Allow AI to edit live"));

    // unknown channels are ignored (the reserved chat channel included)
    send(
        &mut board.ws,
        json!({"ch": "chat", "t": "prompt", "text": "hi"}),
    )
    .await;
    let h = hub.clone();
    let call = tokio::spawn(async move { h.call(Op::GetPage { page: None }).await });
    board.serve_op().await;
    assert_eq!(
        call.await.unwrap().unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn reconnect_with_token_merges_both_ways() {
    let (hub, port) = hub().await;
    let (mut board, token) = paired_board(&hub, port, "Cells").await;
    settle(|| hub.mirror_version().is_some()).await;
    // The board's autosave from before the agent's edit.
    board.pkg.update_lesson(&board.lesson, "test", 2_000.0);
    let autosave = board.pkg.write().unwrap();

    let h = hub.clone();
    let call = tokio::spawn(async move {
        h.call(Op::AddText {
            page: None,
            text: "Mitochondria".into(),
            x: 100.0,
            y: 100.0,
            w: None,
            size: None,
            color: None,
        })
        .await
    });
    board.serve_op().await;
    let agent_item = call.await.unwrap().unwrap()["item_id"]
        .as_str()
        .unwrap()
        .to_string();
    let page = board.page.clone();

    // The tab crashes and reloads from the older autosave; the teacher
    // writes before the board reconnects.
    drop(board);
    settle(|| !hub.is_paired()).await;
    let pkg = Package::read(&autosave).unwrap();
    let lesson = pkg.lesson().unwrap();
    let mut board = Board::connect(port, pkg, lesson, Some(&token)).await;
    assert!(
        board.lesson.item(&page, &agent_item).is_err(),
        "the reload lost the agent's text"
    );
    let teacher = board.stroke();

    // The token pairs it straight away, and the server offers its version.
    let p = expect(&mut board.ws, "session", "paired").await;
    assert_eq!(p["token"], token.as_str());
    let s = expect(&mut board.ws, "doc", "sync").await;
    assert_eq!(s["key"], board.key().as_str());
    assert!(s["vv"].is_string());
    board.answer_sync(&s).await;
    // The mirror sends back what the board lost.
    let u = expect(&mut board.ws, "doc", "update").await;
    board
        .lesson
        .import(&B64.decode(u["data"].as_str().unwrap()).unwrap())
        .unwrap();

    assert!(
        board.lesson.item(&page, &agent_item).is_ok(),
        "agent edit restored"
    );
    assert!(board.lesson.item(&page, &teacher).is_ok());
    settle(|| hub.mirror_version().unwrap() == sync::version(&board.lesson)).await;
    assert!(
        hub.with_mirror(|l| l.item(&page, &teacher).is_ok())
            .unwrap()
    );
    assert_eq!(
        hub.mirror_outline().unwrap()["pages"][0]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn rejects_foreign_origins_hosts_and_bad_hellos() {
    let (hub, port) = hub().await;
    for origin in [
        Some("https://evil.example"),
        Some("https://rupok.app.evil.example"),
        Some("null"),
        None,
    ] {
        let e = open(port, origin, None).await.unwrap_err();
        assert!(
            matches!(&e, tungstenite::Error::Http(r) if r.status() == 403),
            "{origin:?}: {e:?}"
        );
    }
    let e = open(port, Some(ORIGIN), Some("rebind.evil.example:1234"))
        .await
        .unwrap_err();
    assert!(
        matches!(&e, tungstenite::Error::Http(r) if r.status() == 403),
        "{e:?}"
    );
    // the hosted app and the desktop shell are welcome
    for origin in [
        "https://rupok.app",
        "tauri://localhost",
        "http://localhost:5173",
    ] {
        let mut ws = open(port, Some(origin), None).await.unwrap();
        ws.close(None).await.ok();
    }

    // anything but a hello first is refused and closed
    let mut ws = open(port, Some(ORIGIN), None).await.unwrap();
    send(
        &mut ws,
        json!({"ch": "tools", "t": "result", "id": 1, "ok": true}),
    )
    .await;
    let f = expect(&mut ws, "session", "error").await;
    assert_eq!(f["code"], "bad_hello");
    assert!(recv(&mut ws).await.is_none(), "closed after a bad hello");

    // a newer protocol version is refused with an explanation
    let mut ws = open(port, Some(ORIGIN), None).await.unwrap();
    send(&mut ws, json!({"ch": "session", "t": "hello", "v": 99})).await;
    let f = expect(&mut ws, "session", "error").await;
    assert!(f["error"].as_str().unwrap().contains("protocol"));

    // a forged token gets a pairing code, not a session
    let (pkg, lesson) = Board::lesson("X");
    let mut b = Board::connect(port, pkg, lesson, Some("00000000000000000000000000000000")).await;
    let w = expect(&mut b.ws, "session", "welcome").await;
    let code = w["code"].as_str().unwrap().to_string();
    assert!(!hub.is_paired());

    // wrong codes are refused; five in a row replace the code
    let wrong = if code == "000000" { "111111" } else { "000000" };
    assert!(hub.connect("12").is_err());
    for _ in 0..4 {
        assert!(
            hub.connect(wrong)
                .unwrap_err()
                .message
                .contains("does not match")
        );
    }
    let e = hub.connect(wrong).unwrap_err();
    assert!(e.message.contains("new one"), "{}", e.message);
    let w2 = expect(&mut b.ws, "session", "welcome").await;
    let code2 = w2["code"].as_str().unwrap().to_string();
    assert!(
        hub.connect(&code).is_err() || code == code2,
        "the old code is dead"
    );
    hub.connect(&code2).expect("the new code pairs");
    expect(&mut b.ws, "session", "paired").await;
    // a waiting board can do nothing before pairing: covered by `frame`
    // ignoring unpaired connections; once paired the session works.
    assert!(hub.is_paired());
}

#[tokio::test]
async fn one_board_at_a_time_and_timeouts() {
    let (hub, port) = hub_with(LiveConfig {
        op_timeout: Duration::from_millis(300),
        ..Default::default()
    })
    .await;
    let (mut first, _) = paired_board(&hub, port, "One").await;
    // the board never answers: the agent gets a clear timeout
    let e = hub.call(Op::GetOutline).await.unwrap_err();
    assert!(e.message.contains("did not answer"), "{}", e.message);
    expect(&mut first.ws, "tools", "op").await;

    let (_second, _) = paired_board(&hub, port, "Two").await;
    let f = expect(&mut first.ws, "session", "error").await;
    assert_eq!(f["code"], "replaced");
    assert!(recv(&mut first.ws).await.is_none());
    assert!(hub.is_paired());
    assert!(hub.disconnect());
    assert!(!hub.is_paired());
}

#[test]
fn disabled_hub_explains_itself() {
    let hub = LiveHub::disabled("the server was started with --no-live");
    let st = hub.status();
    assert!(!st.enabled);
    assert!(st.hint.contains("--no-live"));
    assert!(hub.connect("123456").is_err());
}
