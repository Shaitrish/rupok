//! The loopback WebSocket server boards connect to.
//!
//! Each connection: handshake (Host + Origin checks), `session/hello` within
//! the hello timeout, then frames routed to the [`LiveHub`]. A writer task
//! owns the socket's sending half and drains the connection's queue.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use rupok_live::protocol::{
    BoardMsg, BoardSession, Frame, MAX_MESSAGE_BYTES, PROTOCOL_VERSION, ServerMsg, ServerSession,
    decode, encode, host_allowed, origin_allowed,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};

use super::{LiveHub, Outgoing};

/// Bind `127.0.0.1` on the first free port of `ports`.
pub async fn bind(ports: &[u16]) -> std::io::Result<(TcpListener, u16)> {
    let mut last = std::io::Error::new(std::io::ErrorKind::InvalidInput, "no port to try");
    for &p in ports {
        match TcpListener::bind(("127.0.0.1", p)).await {
            Ok(l) => {
                let port = l.local_addr()?.port();
                return Ok((l, port));
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Accept boards until the process exits.
pub async fn serve(listener: TcpListener, port: u16, hub: Arc<LiveHub>) {
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            continue;
        };
        if !peer.ip().is_loopback() {
            continue;
        }
        let _ = stream.set_nodelay(true);
        tokio::spawn(connection(stream, port, hub.clone()));
    }
}

fn refuse(status: StatusCode, why: &str) -> ErrorResponse {
    let mut r = ErrorResponse::new(Some(why.to_string()));
    *r.status_mut() = status;
    r
}

async fn connection(stream: TcpStream, port: u16, hub: Arc<LiveHub>) {
    let extra = hub.config().extra_origins.clone();
    // The handshake callback's signature (and its large error type) is tungstenite's.
    #[allow(clippy::result_large_err)]
    let check = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let header = |name: &str| {
            req.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string()
        };
        if !host_allowed(&header("host"), port) {
            return Err(refuse(StatusCode::FORBIDDEN, "host not allowed"));
        }
        let origin = header("origin");
        if !origin_allowed(&origin, &extra) {
            return Err(refuse(
                StatusCode::FORBIDDEN,
                "origin not allowed: only a Rupok board may connect",
            ));
        }
        Ok(resp)
    };
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let Ok(ws) = tokio_tungstenite::accept_hdr_async_with_config(stream, check, Some(cfg)).await
    else {
        return;
    };
    let (mut sink, mut source) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Outgoing>();
    let writer = tokio::spawn(async move {
        while let Some(out) = rx.recv().await {
            match out {
                Outgoing::Text(t) => {
                    if sink.send(Message::text(t)).await.is_err() {
                        break;
                    }
                }
                Outgoing::Close(code, reason) => {
                    let _ = sink
                        .send(Message::Close(Some(CloseFrame {
                            code: CloseCode::from(code),
                            reason: reason.into(),
                        })))
                        .await;
                    break;
                }
            }
        }
        let _ = sink.close().await;
    });

    let conn = hub.next_conn();
    let hello = tokio::time::timeout(hub.config().hello_timeout, next_text(&mut source)).await;
    let refuse_hello = |why: &str| {
        let _ = tx.send(Outgoing::Text(encode(&ServerMsg::Session(
            ServerSession::Error {
                error: why.to_string(),
                code: "bad_hello".into(),
            },
        ))));
        let _ = tx.send(Outgoing::Close(4001, "bad_hello".into()));
    };
    let accepted = match hello {
        Ok(Some(text)) => match decode::<BoardMsg>(&text) {
            Ok(Frame::Known(BoardMsg::Session(BoardSession::Hello {
                v,
                client,
                token,
                doc,
                ..
            }))) if v == PROTOCOL_VERSION => {
                hub.hello(conn, tx.clone(), client, token, doc);
                true
            }
            Ok(Frame::Known(BoardMsg::Session(BoardSession::Hello { v, .. }))) => {
                refuse_hello(&format!(
                    "this AI assistant speaks Rupok live protocol {PROTOCOL_VERSION}, the board {v}; update Rupok or the rupok CLI"
                ));
                false
            }
            _ => {
                refuse_hello("the first message must be session/hello");
                false
            }
        },
        _ => {
            refuse_hello("no hello");
            false
        }
    };

    while accepted && let Some(text) = next_text(&mut source).await {
        match decode::<BoardMsg>(&text) {
            Ok(Frame::Known(msg)) => hub.frame(conn, msg),
            Ok(Frame::Other(ch, v)) => hub.other(conn, ch, v),
            Err(e) => eprintln!("rupok live: ignoring a bad frame: {e}"),
        }
    }
    hub.closed(conn);
    drop(tx);
    let _ = writer.await;
}

/// The next text frame, or `None` when the socket closes.
async fn next_text<S>(source: &mut S) -> Option<String>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(msg) = source.next().await {
        match msg {
            Ok(Message::Text(t)) => return Some(t.as_str().to_string()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
    None
}
