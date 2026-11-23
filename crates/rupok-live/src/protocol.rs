//! The live protocol, version 1: JSON text frames on one WebSocket that the
//! board opens to `ws://127.0.0.1:<port>`. Normative description:
//! `docs/ai-agents.md` § "Live protocol".
//!
//! Every frame names a **channel** (`ch`) and a message type (`t`):
//!
//! | `ch`      | Purpose                                                   |
//! |-----------|-----------------------------------------------------------|
//! | `session` | hello, pairing, presence, board state, errors             |
//! | `doc`     | Loro sync of the open lesson (snapshot, updates, versions) |
//! | `tools`   | agent operations run on the board, and their results      |
//! | `chat`    | reserved: in-app chat with the teacher's agent (ACP relay)  |
//!
//! ```text
//! board                                          server (rupok mcp --live)
//!   session/hello {v, client, channels, token?, doc}  ──▶
//!                                       ◀──  session/welcome {code, agent, channels}  (pairing)
//!     … the teacher tells the agent the code; the agent calls connect_board(code) …
//!                                       ◀──  session/paired {token, agent, channels}
//!                                       ◀──  doc/sync {key?, vv?}         (mirror version)
//!   doc/snapshot | doc/update + doc/sync {key, vv}  ──▶
//!                                       ◀──  doc/update                    (what the board lacks)
//!   doc/update  ──▶  after every board change (teacher or agent)
//!                                       ◀──  tools/op {id, agent, op}
//!   tools/result {id, ok, result | error, data?}  ──▶
//!   session/state {page, page_count, allow_edit, …}  ──▶  when it changes
//! ```
//!
//! Versioning: `hello.v` is the protocol version and `hello.channels` the
//! channels the board speaks; `welcome`/`paired` answer with the channels the
//! server will use. A receiver ignores frames on channels it does not know
//! and message types it does not know, so a channel (e.g. `chat`) can be
//! added without breaking older peers. Binary payloads (Loro updates,
//! version vectors, `.rpk` snapshots, PNGs) are base64 strings.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::op::Op;

pub const PROTOCOL_VERSION: u32 = 1;

/// Ports the server tries, in order, and the board probes. The first free
/// one is used; a second agent on the same computer gets the next.
pub const PORTS: [u16; 4] = [47373, 47374, 47375, 47376];

/// The default (first) port.
pub const DEFAULT_PORT: u16 = PORTS[0];

/// Largest WebSocket message either side accepts (a `.rpk` snapshot with
/// images can be large; this is loopback only).
pub const MAX_MESSAGE_BYTES: usize = 64 << 20;

/// The hosted board.
pub const HOSTED_ORIGIN: &str = "https://rupok.app";

/// Channel names.
pub mod ch {
    pub const SESSION: &str = "session";
    pub const DOC: &str = "doc";
    pub const TOOLS: &str = "tools";
    /// Reserved for the in-app chat (ACP relay). Not implemented in v1.
    pub const CHAT: &str = "chat";
}

/// Channels this build speaks.
pub const CHANNELS: [&str; 3] = [ch::SESSION, ch::DOC, ch::TOOLS];

/// True when a WebSocket `Origin` header belongs to a Rupok board:
/// `https://rupok.app`, `http://127.0.0.1[:port]`, `http://localhost[:port]`,
/// `http://[::1][:port]`, the Tauri shell (`tauri://localhost`,
/// `http(s)://tauri.localhost`), or one of `extra` (exact match).
pub fn origin_allowed(origin: &str, extra: &[String]) -> bool {
    let o = origin.trim().to_ascii_lowercase();
    if o == HOSTED_ORIGIN
        || o == "tauri://localhost"
        || o == "http://tauri.localhost"
        || o == "https://tauri.localhost"
    {
        return true;
    }
    if extra
        .iter()
        .any(|e| e.trim().trim_end_matches('/').eq_ignore_ascii_case(&o))
    {
        return true;
    }
    for base in ["http://127.0.0.1", "http://localhost", "http://[::1]"] {
        if let Some(rest) = o.strip_prefix(base) {
            return rest.is_empty() || is_port_suffix(rest);
        }
    }
    false
}

/// True when a `Host` header names this loopback server (`127.0.0.1:port`,
/// `localhost:port` or `[::1]:port`). Rejecting other hosts defeats DNS
/// rebinding even before the origin check.
pub fn host_allowed(host: &str, port: u16) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let suffix = format!(":{port}");
    match h.strip_suffix(&suffix) {
        Some(name) => matches!(name, "127.0.0.1" | "localhost" | "[::1]"),
        None => false,
    }
}

fn is_port_suffix(s: &str) -> bool {
    s.strip_prefix(':')
        .is_some_and(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Identity of the lesson open on the board (see [`crate::doc_key`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DocInfo {
    pub key: String,
    #[serde(default)]
    pub title: String,
}

// ------------------------------------------------------------- channels

/// `session` channel, board → server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum BoardSession {
    Hello {
        v: u32,
        #[serde(default)]
        client: String,
        /// Channels the board speaks.
        #[serde(default)]
        channels: Vec<String>,
        /// Session token from an earlier `paired`, to reconnect without a code.
        #[serde(default)]
        token: Option<String>,
        #[serde(default)]
        doc: Option<DocInfo>,
    },
    /// What the board shows. Sent after pairing and whenever it changes.
    State {
        /// 1-based page number.
        page: usize,
        page_count: usize,
        allow_edit: bool,
        #[serde(default)]
        read_only: bool,
        #[serde(default)]
        title: String,
    },
    Bye,
}

/// `session` channel, server → board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerSession {
    /// Pairing: show `code` on the board; the teacher gives it to the agent.
    Welcome {
        v: u32,
        agent: String,
        code: String,
        expires_in: u64,
        channels: Vec<String>,
    },
    Paired {
        v: u32,
        token: String,
        agent: String,
        channels: Vec<String>,
    },
    /// The agent's display name changed (the MCP client identified itself).
    Agent { agent: String },
    Error { error: String, code: String },
}

/// `doc` channel, both directions: Loro sync of the open lesson.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum DocMsg {
    /// The whole lesson as `.rpk` bytes (base64), board → server: the first
    /// sync of a lesson the server has not seen, or another lesson opened.
    Snapshot { data: String, doc: DocInfo },
    /// A Loro update (base64).
    Update { data: String },
    /// "Here is my version of lesson `key`; send what I lack." From the
    /// server, `vv: None` asks the board for a snapshot.
    Sync {
        #[serde(default)]
        key: Option<String>,
        #[serde(default)]
        vv: Option<String>,
    },
}

/// `tools` channel, board → server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum BoardTools {
    /// Reply to an `op`.
    Result {
        id: u64,
        ok: bool,
        #[serde(default)]
        result: Option<Value>,
        #[serde(default)]
        error: Option<String>,
        /// Structured error details (e.g. Smart Object diagnostics).
        #[serde(default)]
        data: Option<Value>,
    },
}

/// `tools` channel, server → board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerTools {
    Op { id: u64, agent: String, op: Op },
}

// ------------------------------------------------------------- frames

/// A frame from the board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ch", rename_all = "snake_case")]
pub enum BoardMsg {
    Session(BoardSession),
    Doc(DocMsg),
    Tools(BoardTools),
}

/// A frame from the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ch", rename_all = "snake_case")]
pub enum ServerMsg {
    Session(ServerSession),
    Doc(DocMsg),
    Tools(ServerTools),
}

/// A decoded frame: a known message, a frame on a channel this build does
/// not handle (kept raw for an extension such as chat), or garbage.
#[derive(Clone, Debug, PartialEq)]
pub enum Frame<T> {
    Known(T),
    /// `(channel, raw frame)`: an unknown channel, or an unknown `t` on a known one.
    Other(String, Value),
}

/// Parse a frame, separating unknown channels and message types (which a
/// receiver must ignore or hand to an extension) from malformed JSON.
pub fn decode<T: for<'de> Deserialize<'de>>(text: &str) -> Result<Frame<T>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let Some(ch) = v.get("ch").and_then(Value::as_str).map(str::to_string) else {
        return Err("frame has no `ch` channel".into());
    };
    if v.get("t").and_then(Value::as_str).is_none() {
        return Err("frame has no `t` type".into());
    }
    match serde_json::from_value::<T>(v.clone()) {
        Ok(m) => Ok(Frame::Known(m)),
        Err(_) if !CHANNELS.contains(&ch.as_str()) || !known_type(&ch, &v) => {
            Ok(Frame::Other(ch, v))
        }
        Err(e) => Err(format!("bad `{ch}` frame: {e}")),
    }
}

/// True when `t` is a message type this build knows on channel `ch` (in
/// either direction), i.e. a decode failure is a malformed frame.
fn known_type(ch: &str, v: &Value) -> bool {
    let t = v.get("t").and_then(Value::as_str).unwrap_or("");
    match ch {
        ch::SESSION => matches!(
            t,
            "hello" | "state" | "bye" | "welcome" | "paired" | "agent" | "error"
        ),
        ch::DOC => matches!(t, "snapshot" | "update" | "sync"),
        ch::TOOLS => matches!(t, "op" | "result"),
        _ => false,
    }
}

/// Serialise a frame.
pub fn encode<T: Serialize>(msg: &T) -> String {
    serde_json::to_string(msg).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        let extra = vec!["https://board.school.example".to_string()];
        for ok in [
            "https://rupok.app",
            "HTTPS://RUPOK.APP",
            "http://127.0.0.1:3737",
            "http://127.0.0.1",
            "http://localhost:5173",
            "http://[::1]:8080",
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
            "https://board.school.example",
        ] {
            assert!(origin_allowed(ok, &extra), "{ok}");
        }
        for bad in [
            "",
            "null",
            "https://evil.example",
            "https://rupok.app.evil.example",
            "https://evil.rupok.app",
            "http://rupok.app",
            "http://127.0.0.1.evil.example",
            "http://127.0.0.1:3737/path",
            "http://127.0.0.1:",
            "http://127.0.0.1:123456",
            "http://localhost.evil:80",
            "http://localhostx",
            "https://localhost:3000",
            "file://",
        ] {
            assert!(!origin_allowed(bad, &extra), "{bad}");
        }
    }

    #[test]
    fn hosts() {
        assert!(host_allowed("127.0.0.1:47373", 47373));
        assert!(host_allowed("localhost:47373", 47373));
        assert!(host_allowed("[::1]:47373", 47373));
        assert!(!host_allowed("127.0.0.1:47374", 47373));
        assert!(!host_allowed("127.0.0.1", 47373));
        assert!(!host_allowed("evil.example:47373", 47373));
        assert!(!host_allowed("rebind.evil.example:47373", 47373));
    }

    #[test]
    fn frames_are_channelled() {
        let m = decode::<BoardMsg>(
            r#"{"ch":"session","t":"hello","v":1,"client":"rupok-board","channels":["session","doc","tools"],"doc":{"key":"k","title":"T"}}"#,
        )
        .unwrap();
        assert_eq!(
            m,
            Frame::Known(BoardMsg::Session(BoardSession::Hello {
                v: 1,
                client: "rupok-board".into(),
                channels: vec!["session".into(), "doc".into(), "tools".into()],
                token: None,
                doc: Some(DocInfo {
                    key: "k".into(),
                    title: "T".into()
                })
            }))
        );
        let s = serde_json::to_value(ServerMsg::Tools(ServerTools::Op {
            id: 7,
            agent: "Claude Code".into(),
            op: Op::GoToPage { page: 2 },
        }))
        .unwrap();
        assert_eq!(s["ch"], "tools");
        assert_eq!(s["t"], "op");
        assert_eq!(s["op"]["op"], "go_to_page");
        assert_eq!(s["op"]["page"], 2);
        let r = decode::<BoardMsg>(r#"{"ch":"tools","t":"result","id":7,"ok":false,"error":"no"}"#)
            .unwrap();
        assert!(matches!(
            r,
            Frame::Known(BoardMsg::Tools(BoardTools::Result { id: 7, ok: false, .. }))
        ));
        let d = serde_json::to_value(ServerMsg::Doc(DocMsg::Sync { key: None, vv: None })).unwrap();
        assert_eq!(d, serde_json::json!({"ch": "doc", "t": "sync", "key": null, "vv": null}));
    }

    #[test]
    fn unknown_channels_and_types_are_ignorable() {
        let c = decode::<BoardMsg>(r#"{"ch":"chat","t":"prompt","text":"make a class"}"#).unwrap();
        assert!(matches!(c, Frame::Other(ref ch, _) if ch == "chat"));
        let t = decode::<BoardMsg>(r#"{"ch":"doc","t":"presence","x":1}"#).unwrap();
        assert!(matches!(t, Frame::Other(ref ch, _) if ch == "doc"));
        assert!(decode::<BoardMsg>(r#"{"ch":"doc","t":"update"}"#).is_err());
        assert!(decode::<BoardMsg>("not json").is_err());
        assert!(decode::<BoardMsg>(r#"{"t":"hello"}"#).is_err());
    }
}
