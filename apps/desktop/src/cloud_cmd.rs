//! Tauri commands for the Rupok Cloud desktop sign-in (`cloud_auth.rs`).

use std::net::TcpListener;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::cloud_auth;

/// The loopback listener of the sign-in in progress, and its `state`.
#[derive(Default)]
pub struct CloudAuthState(Mutex<Option<(TcpListener, String)>>);

/// Listen on a random loopback port and open the system browser at the
/// cloud's native sign-in start (the URL is built here, never taken from JS).
#[tauri::command]
pub fn cloud_auth_begin(
    auth: State<'_, CloudAuthState>,
    challenge: String,
    state: String,
) -> Result<(), String> {
    let listener = cloud_auth::listen().map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let url = cloud_auth::start_url(&cloud_auth::cloud_origin(), &challenge, port, &state)
        .map_err(|e| e.to_string())?;
    *auth.0.lock().unwrap_or_else(|e| e.into_inner()) = Some((listener, state));
    cloud_auth::open_in_browser(&url).map_err(|e| format!("could not open the browser: {e}"))
}

#[derive(Serialize)]
pub struct CodeOut {
    code: String,
}

/// Wait (up to 3 minutes) for the browser to come back with the one-time code.
#[tauri::command]
pub async fn cloud_auth_wait(
    auth: State<'_, CloudAuthState>,
    state: String,
) -> Result<Option<CodeOut>, String> {
    let Some((listener, expected)) = auth.0.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return Err("no sign-in in progress".into());
    };
    if expected != state {
        return Err("sign-in state mismatch".into());
    }
    let code = tauri::async_runtime::spawn_blocking(move || {
        cloud_auth::wait_for_code(&listener, &expected, Duration::from_secs(180))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(code.map(|code| CodeOut { code }))
}
