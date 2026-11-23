//! Tauri commands for the lesson library (`library.rs`): the board's desktop
//! provider (`apps/board/src/library/desktop.ts`).
//!
//! - The root is `Documents/Rupok` unless the teacher picked another folder
//!   with `library_choose_root` (a native dialog, so web content can never
//!   point the library at an arbitrary path). The choice is kept in
//!   `<app config>/library.json`.
//! - The folder is watched; changes made by agents (`rupok mcp`), sync
//!   clients or the file manager reach the board as `rupok://library-changed`
//!   (debounced, relative paths).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::app::{MAIN, header, may_write};
use crate::files;
use crate::library::{Library, RawEntry, default_root};

/// Sent to the board when files in the library change on disk.
pub const CHANGED_EVENT: &str = "rupok://library-changed";

#[derive(Default)]
pub struct LibraryState {
    lib: Mutex<Option<Arc<Library>>>,
    custom: Mutex<bool>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    root: String,
    default_root: String,
    custom: bool,
}

fn config_file<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("library.json"))
}

fn default_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let base = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    default_root(&base)
}

fn saved_root<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let text = std::fs::read_to_string(config_file(app)?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("root")?.as_str().map(PathBuf::from)
}

fn save_root<R: Runtime>(app: &AppHandle<R>, root: Option<&Path>) -> Result<(), String> {
    let Some(file) = config_file(app) else {
        return Err("no config folder".into());
    };
    match root {
        Some(r) => {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let json = serde_json::json!({ "root": r.to_string_lossy() });
            files::write_atomic(&file, json.to_string().as_bytes()).map_err(|e| e.to_string())
        }
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    }
}

fn info<R: Runtime>(app: &AppHandle<R>, lib: &Library, custom: bool) -> Info {
    Info {
        root: lib.root().to_string_lossy().into_owned(),
        default_root: default_dir(app).to_string_lossy().into_owned(),
        custom,
    }
}

/// Open the library (the saved folder, else the default) and watch it.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<Info, String> {
    let saved = saved_root(app);
    let (lib, custom) = match saved.as_ref().map(Library::open) {
        Some(Ok(lib)) => (lib, true),
        // A chosen folder that is gone (unplugged drive): fall back to the default.
        _ => (
            Library::open(default_dir(app))
                .map_err(|e| format!("cannot open the library folder: {e}"))?,
            false,
        ),
    };
    let state = app.state::<LibraryState>();
    let watcher = watch(app, lib.root());
    let out = info(app, &lib, custom);
    *state.lib.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(lib));
    *state.custom.lock().unwrap_or_else(|e| e.into_inner()) = custom;
    *state.watcher.lock().unwrap_or_else(|e| e.into_inner()) = watcher;
    Ok(out)
}

fn lib<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, LibraryState>,
) -> Result<Arc<Library>, String> {
    if let Some(l) = state.lib.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(l);
    }
    init(app)?;
    state
        .lib
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| "the library is not available".into())
}

/// Watch `root` recursively; emit changed relative paths, debounced.
fn watch<R: Runtime>(app: &AppHandle<R>, root: &Path) -> Option<notify::RecommendedWatcher> {
    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
    let mut w = notify::recommended_watcher(tx).ok()?;
    w.watch(root, RecursiveMode::Recursive).ok()?;
    let app = app.clone();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let collect = |ev: notify::Result<notify::Event>, out: &mut BTreeSet<String>| {
            let Ok(ev) = ev else { return };
            // Reads (ours, when listing) must not look like changes.
            if matches!(ev.kind, EventKind::Access(_)) {
                return;
            }
            for p in ev.paths {
                let Ok(rel) = p.strip_prefix(&root) else {
                    continue;
                };
                let rel = rel.to_string_lossy().replace('\\', "/");
                if !rel.ends_with(".tmp") {
                    out.insert(rel);
                }
            }
        };
        // Ends when the watcher (and with it the sender) is dropped.
        while let Ok(first) = rx.recv() {
            let mut paths = BTreeSet::new();
            collect(first, &mut paths);
            let until = Instant::now() + Duration::from_millis(300);
            while let Ok(ev) = rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                collect(ev, &mut paths);
            }
            if !paths.is_empty() {
                let _ = app.emit_to(MAIN, CHANGED_EVENT, paths.into_iter().collect::<Vec<_>>());
            }
        }
    });
    Some(w)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, crate::library::LibraryError> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_info<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
) -> Result<Info, String> {
    let l = lib(&app, &state)?;
    let custom = *state.custom.lock().unwrap_or_else(|e| e.into_inner());
    Ok(info(&app, &l, custom))
}

#[tauri::command]
pub async fn library_list<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    dir: String,
) -> Result<Vec<RawEntry>, String> {
    let l = lib(&app, &state)?;
    blocking(move || l.list(&dir)).await
}

#[tauri::command]
pub async fn library_stat<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    path: String,
) -> Result<Option<RawEntry>, String> {
    let l = lib(&app, &state)?;
    blocking(move || l.stat(&path)).await
}

#[tauri::command]
pub async fn library_read<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    path: String,
) -> Result<Response, String> {
    let l = lib(&app, &state)?;
    Ok(Response::new(blocking(move || l.read(&path)).await?))
}

#[tauri::command]
pub async fn library_read_range<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    path: String,
    start: u64,
    end: u64,
) -> Result<Response, String> {
    let l = lib(&app, &state)?;
    Ok(Response::new(
        blocking(move || l.read_range(&path, start, end)).await?,
    ))
}

/// Body = file bytes; header `x-rupok-path` = library path.
#[tauri::command]
pub async fn library_write<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    request: Request<'_>,
) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the file bytes as the request body".into());
    };
    let path = header(&request, "x-rupok-path")?;
    let l = lib(&app, &state)?;
    let bytes = bytes.clone();
    blocking(move || l.write(&path, &bytes)).await
}

#[tauri::command]
pub async fn library_mkdir<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    path: String,
) -> Result<(), String> {
    let l = lib(&app, &state)?;
    blocking(move || l.mkdir(&path)).await
}

#[tauri::command]
pub async fn library_rename<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    from: String,
    to: String,
) -> Result<(), String> {
    let l = lib(&app, &state)?;
    blocking(move || l.rename(&from, &to)).await
}

#[tauri::command]
pub async fn library_copy<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    from: String,
    to: String,
) -> Result<(), String> {
    let l = lib(&app, &state)?;
    blocking(move || l.copy(&from, &to)).await
}

#[tauri::command]
pub async fn library_remove<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    path: String,
) -> Result<(), String> {
    let l = lib(&app, &state)?;
    blocking(move || l.remove(&path)).await
}

/// Pick another library folder (native dialog). Resolves to null if cancelled.
#[tauri::command]
pub async fn library_choose_root<R: Runtime>(app: AppHandle<R>) -> Result<Option<Info>, String> {
    let start = app
        .state::<LibraryState>()
        .lib
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|l| l.root().to_path_buf());
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut d = dialog_app
            .dialog()
            .file()
            .set_title("Choose the folder for your Rupok library");
        if let Some(s) = start {
            d = d.set_directory(s);
        }
        d.blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    Library::open(&path).map_err(|e| format!("cannot use {}: {e}", path.display()))?;
    save_root(&app, Some(&path))?;
    init(&app).map(Some)
}

/// Back to `Documents/Rupok`.
#[tauri::command]
pub fn library_reset_root<R: Runtime>(app: AppHandle<R>) -> Result<Info, String> {
    save_root(&app, None)?;
    init(&app)
}

/// Save an exported folder zip to a path chosen in the save dialog.
#[tauri::command]
pub async fn library_save_zip<R: Runtime>(
    app: AppHandle<R>,
    request: Request<'_>,
) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the zip bytes as the request body".into());
    };
    let path = PathBuf::from(header(&request, "x-rupok-path")?);
    let is_zip = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !is_zip {
        return Err("exported folders must end in .zip".into());
    }
    if !(bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")) {
        return Err("not a zip archive, refusing to write it".into());
    }
    if !may_write(&app, &path) {
        return Err(format!(
            "{} was not chosen in a file dialog",
            path.display()
        ));
    }
    let bytes = bytes.clone();
    tauri::async_runtime::spawn_blocking(move || files::write_atomic(&path, &bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
