//! The Rupok MCP tools over loopback streamable HTTP, for an agent that
//! `rupok agent` spawned (ACP `session/new` lists this server with its URL
//! and a bearer token). Binds 127.0.0.1 on a free port; requests need the
//! token, and rmcp checks the `Host` header (loopback names only).

use std::convert::Infallible;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;

use crate::mcp::McpServer;

/// A running MCP HTTP endpoint.
#[derive(Clone, Debug)]
pub struct McpHttp {
    pub url: String,
    pub token: String,
}

impl McpHttp {
    /// The `Authorization` header value the agent must send.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

/// Serve `server` on `127.0.0.1:<port>` (0: any free port) at `/mcp`.
pub async fn serve(server: McpServer, port: u16) -> std::io::Result<McpHttp> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let port = listener.local_addr()?.port();
    let token = crate::live::random_token();
    let svc = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let expected = Arc::new(format!("Bearer {token}"));
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let svc = svc.clone();
            let expected = expected.clone();
            tokio::spawn(async move {
                let handler = service_fn(move |req: Request<Incoming>| {
                    let svc = svc.clone();
                    let expected = expected.clone();
                    async move {
                        let ok = req
                            .headers()
                            .get(hyper::header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .is_some_and(|v| crate::live::ct_eq(v.as_bytes(), expected.as_bytes()));
                        if !ok || req.uri().path() != "/mcp" {
                            return Ok::<_, Infallible>(plain(
                                if ok {
                                    StatusCode::NOT_FOUND
                                } else {
                                    StatusCode::UNAUTHORIZED
                                },
                                "rupok: not found or not authorised",
                            ));
                        }
                        Ok(svc.handle(req).await)
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), handler)
                    .await;
            });
        }
    });
    Ok(McpHttp {
        url: format!("http://127.0.0.1:{port}/mcp"),
        token,
    })
}

fn plain(status: StatusCode, text: &'static str) -> Response<BoxBody<Bytes, Infallible>> {
    let mut r = Response::new(Full::new(Bytes::from_static(text.as_bytes())).boxed());
    *r.status_mut() = status;
    r
}
