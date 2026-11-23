//! The in-app AI chat in the desktop app: the `rupok agent` relay
//! (rupok_cli::chat) run inside the app, so the teacher needs no terminal.
//! The board calls `agent_start` and pairs with the returned code at once.

use serde::Serialize;
use tauri::State;
use tauri::async_runtime::Mutex;

/// The relay, once started (one per app run).
#[derive(Default)]
pub struct AgentState(Mutex<Option<Relay>>);

#[derive(Clone, Serialize)]
pub struct Relay {
    /// Live channel port on 127.0.0.1.
    pub port: u16,
    /// Pairing code: only this window gets it.
    pub code: String,
}

/// Start the relay if needed; returns where the board connects and the code.
#[tauri::command]
pub async fn agent_start(state: State<'_, AgentState>) -> Result<Relay, String> {
    let mut slot = state.0.lock().await;
    if let Some(r) = slot.as_ref() {
        return Ok(r.clone());
    }
    let workspace = std::env::var_os("RUPOK_LIBRARY")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(rupok_cli::cli::library_dir)
        .ok_or("no Documents folder for the lessons")?;
    std::fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;
    let started = rupok_cli::chat::start(rupok_cli::chat::AgentOptions {
        workspace,
        ..Default::default()
    })
    .await
    .map_err(|e| format!("could not start the AI relay: {e:#}"))?;
    let relay = Relay {
        port: started.port,
        code: started.code.clone(),
    };
    *slot = Some(relay.clone());
    Ok(relay)
}
