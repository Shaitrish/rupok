//! Rupok Cloud sign-in for the desktop app (RFC 8252: system browser, loopback
//! redirect, PKCE). Cookies don't work from the app's webview origin, so the
//! app gets a bearer token instead. See `apps/board/src/cloud/desktop.ts` for
//! the whole flow.
//!
//! This module is the Rust half: listen on `127.0.0.1:<random port>`, build
//! the start URL (only ever `<cloud origin>/api/v1/auth/native/start`, so web
//! content can't make the app open anything else), and wait for the browser
//! to come back with `?code=…&state=…`. The `state` must match what the app
//! sent; the code is useless without the PKCE verifier, which never leaves
//! the app. Nothing here depends on Tauri, so it is unit-tested everywhere.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Where the desktop app signs in. `RUPOK_CLOUD_ORIGIN` overrides it for a
/// local cloud during development.
pub fn cloud_origin() -> String {
    std::env::var("RUPOK_CLOUD_ORIGIN")
        .ok()
        .filter(|o| {
            o.starts_with("https://")
                || o.starts_with("http://127.0.0.1")
                || o.starts_with("http://localhost")
        })
        .unwrap_or_else(|| "https://rupok.app".into())
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid sign-in parameter: {0}")]
    BadParam(&'static str),
    #[error("the sign-in reply did not match this request")]
    StateMismatch,
    #[error("the sign-in reply had no code")]
    NoCode,
    #[error("sign-in was cancelled in the browser")]
    Cancelled,
    #[error("{0}")]
    Io(String),
}

fn b64url(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `<origin>/api/v1/auth/native/start?challenge=…&port=…&state=…`.
pub fn start_url(
    origin: &str,
    challenge: &str,
    port: u16,
    state: &str,
) -> Result<String, AuthError> {
    if challenge.len() != 43 || !b64url(challenge) {
        return Err(AuthError::BadParam("challenge"));
    }
    if !(16..=128).contains(&state.len()) || !b64url(state) {
        return Err(AuthError::BadParam("state"));
    }
    if port < 1024 {
        return Err(AuthError::BadParam("port"));
    }
    Ok(format!(
        "{}/api/v1/auth/native/start?challenge={challenge}&port={port}&state={state}",
        origin.trim_end_matches('/')
    ))
}

fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// The code from `GET /callback?code=…&state=… HTTP/1.1`. `Ok(None)` for
/// other paths (a browser asking for /favicon.ico).
pub fn parse_callback(
    request_line: &str,
    expected_state: &str,
) -> Result<Option<String>, AuthError> {
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return Ok(None);
    }
    let target = parts.next().unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/callback" {
        return Ok(None);
    }
    if query_param(query, "error").is_some() {
        return Err(AuthError::Cancelled);
    }
    if query_param(query, "state") != Some(expected_state) {
        return Err(AuthError::StateMismatch);
    }
    match query_param(query, "code") {
        Some(c) if b64url(c) && c.len() <= 128 => Ok(Some(c.to_string())),
        _ => Err(AuthError::NoCode),
    }
}

/// A listener on a random loopback port (never on other interfaces).
pub fn listen() -> Result<TcpListener, AuthError> {
    let l = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| AuthError::Io(e.to_string()))?;
    l.set_nonblocking(true)
        .map_err(|e| AuthError::Io(e.to_string()))?;
    Ok(l)
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Rupok</title>\
<body style=\"font:16px system-ui,sans-serif;display:grid;place-items:center;height:90vh;color:#1c2024\">\
<div style=\"text-align:center\"><h1 style=\"font-size:22px\">You're signed in to Rupok</h1>\
<p>You can close this tab and go back to the app.</p></div>";

fn reply(mut s: &TcpStream, status: &str, body: &str) {
    let _ = write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// Wait for the browser's redirect; `Ok(None)` if `timeout` passes first.
pub fn wait_for_code(
    listener: &TcpListener,
    state: &str,
    timeout: Duration,
) -> Result<Option<String>, AuthError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut buf = [0u8; 4096];
                let n = (&stream).read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]);
                let line = head.lines().next().unwrap_or("");
                match parse_callback(line, state) {
                    Ok(Some(code)) => {
                        reply(&stream, "200 OK", DONE_PAGE);
                        return Ok(Some(code));
                    }
                    Ok(None) => reply(&stream, "404 Not Found", ""),
                    Err(e) => {
                        reply(
                            &stream,
                            "400 Bad Request",
                            "Sign-in failed. Go back to Rupok and try again.",
                        );
                        return Err(e);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(e) => return Err(AuthError::Io(e.to_string())),
        }
    }
    Ok(None)
}

/// Open `url` in the default browser.
pub fn open_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        // Not `cmd /c start`: `&` in the URL would split the command.
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CH: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    const ST: &str = "abcdefghijklmnop";

    #[test]
    fn start_url_is_only_ever_the_native_start_endpoint() {
        assert_eq!(
            start_url("https://rupok.app/", CH, 51234, ST).unwrap(),
            format!(
                "https://rupok.app/api/v1/auth/native/start?challenge={CH}&port=51234&state={ST}"
            )
        );
        assert_eq!(
            start_url("https://rupok.app", "short", 51234, ST),
            Err(AuthError::BadParam("challenge"))
        );
        assert_eq!(
            start_url("https://rupok.app", CH, 51234, "x&evil=1"),
            Err(AuthError::BadParam("state"))
        );
        assert_eq!(
            start_url("https://rupok.app", CH, 80, ST),
            Err(AuthError::BadParam("port"))
        );
    }

    #[test]
    fn parses_the_callback_and_checks_state() {
        let ok = format!("GET /callback?code=ABC_def-123&state={ST} HTTP/1.1");
        assert_eq!(parse_callback(&ok, ST), Ok(Some("ABC_def-123".into())));
        assert_eq!(parse_callback("GET /favicon.ico HTTP/1.1", ST), Ok(None));
        assert_eq!(parse_callback("POST /callback HTTP/1.1", ST), Ok(None));
        assert_eq!(
            parse_callback(
                "GET /callback?code=ABC&state=other-state-value HTTP/1.1",
                ST
            ),
            Err(AuthError::StateMismatch)
        );
        assert_eq!(
            parse_callback(&format!("GET /callback?state={ST} HTTP/1.1"), ST),
            Err(AuthError::NoCode)
        );
        assert_eq!(
            parse_callback(&format!("GET /callback?code=a%20b&state={ST} HTTP/1.1"), ST),
            Err(AuthError::NoCode)
        );
        assert_eq!(
            parse_callback("GET /callback?error=access_denied HTTP/1.1", ST),
            Err(AuthError::Cancelled)
        );
    }

    #[test]
    fn waits_for_the_browser_on_loopback_only() {
        let l = listen().unwrap();
        let addr = l.local_addr().unwrap();
        assert!(addr.ip().is_loopback());
        let t = std::thread::spawn(move || {
            // A stray request first, then the real redirect.
            let mut s = TcpStream::connect(addr).unwrap();
            s.write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n")
                .unwrap();
            let mut s = TcpStream::connect(addr).unwrap();
            s.write_all(
                format!("GET /callback?code=CODE123&state={ST} HTTP/1.1\r\nHost: x\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        });
        assert_eq!(
            wait_for_code(&l, ST, Duration::from_secs(5)).unwrap(),
            Some("CODE123".into())
        );
        assert!(t.join().unwrap().contains("signed in to Rupok"));
    }

    #[test]
    fn times_out_quietly() {
        let l = listen().unwrap();
        assert_eq!(
            wait_for_code(&l, ST, Duration::from_millis(200)).unwrap(),
            None
        );
    }
}
