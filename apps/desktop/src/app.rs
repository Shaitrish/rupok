//! The Tauri shell: plugins, the main window, file commands, and files the
//! operating system asks us to open.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WindowEvent};
use tauri_plugin_fs::FsExt;
use tauri_plugin_window_state::StateFlags;

use crate::{files, overlay};

/// Label of the board window (`tauri.conf.json`).
pub const MAIN: &str = "main";

/// Tells the board that files are waiting in [`take_pending_files`]. The
/// event carries no payload, so an event sent before the board listens is
/// not lost: the board also drains the queue once at start-up.
const OPEN_FILES_EVENT: &str = "rupok://open-files";

/// Files the OS asked us to open (argv, file association, second launch),
/// until the board takes them.
#[derive(Default)]
struct PendingFiles(Mutex<Vec<PathBuf>>);

pub fn run() {
    crate::prefer_x11_on_wayland();

    tauri::Builder::default()
        // First, so a second launch hands its files over and exits early.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let args = argv.get(1..).unwrap_or_default();
            queue_files(app, files::paths_from_args(args, Path::new(&cwd)));
            focus_main(app);
        }))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    StateFlags::SIZE
                        | StateFlags::POSITION
                        | StateFlags::MAXIMIZED
                        | StateFlags::FULLSCREEN,
                )
                .with_denylist(&[overlay::INK, overlay::TOOLBAR])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(overlay::on_shortcut)
                .build(),
        )
        .manage(PendingFiles::default())
        .manage(overlay::OverlayState::default())
        .invoke_handler(tauri::generate_handler![
            take_pending_files,
            save_lesson,
            save_pdf,
            overlay::overlay_show,
            overlay::overlay_toggle,
            overlay::overlay_hide,
            overlay::overlay_set_click_through,
            overlay::overlay_toolbar_fit,
            overlay::overlay_capture,
            overlay::overlay_deliver_page,
            overlay::take_overlay_page,
        ])
        .setup(|app| {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let cwd = std::env::current_dir().unwrap_or_default();
            queue_files(app.handle(), files::paths_from_args(&args, &cwd));
            overlay::register_shortcuts(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // The overlay windows are only hidden, never closed; quit with the board.
            if window.label() == MAIN && matches!(event, WindowEvent::Destroyed) {
                window.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Rupok");
}

fn focus_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Queue files for the board and let the webview read them (and save back to
/// `.rpk` ones): exactly these paths are added to the file-system scope.
fn queue_files<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let scope = app.fs_scope();
    for p in &paths {
        let _ = scope.allow_file(p);
        if let Ok(real) = std::fs::canonicalize(p) {
            let _ = scope.allow_file(real);
        }
    }
    let state = app.state::<PendingFiles>();
    state
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(paths);
    let _ = app.emit_to(MAIN, OPEN_FILES_EVENT, ());
}

/// The files waiting to be opened, oldest first; empties the queue.
#[tauri::command]
fn take_pending_files(state: State<'_, PendingFiles>) -> Vec<String> {
    let paths = std::mem::take(&mut *state.0.lock().unwrap_or_else(|e| e.into_inner()));
    paths
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Read a percent-encoded header value (raw-body commands carry their
/// arguments in headers).
pub(crate) fn header(request: &Request<'_>, name: &str) -> Result<String, String> {
    let raw = request
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| format!("missing {name} header"))?;
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|e| e.to_string())
}

/// Only write where the teacher pointed: a path from a save/open dialog (the
/// dialog plugin adds it to the scope), a file the OS opened, or that path
/// plus `.rpk` (GTK save dialogs do not append the extension).
fn may_write<R: Runtime>(app: &AppHandle<R>, path: &Path) -> bool {
    let scope = app.fs_scope();
    scope.is_allowed(path) || scope.is_allowed(path.with_extension(""))
}

/// Save a lesson: body = `.rpk` bytes, header `x-rupok-path` = target path.
/// Validated and written atomically (see [`files::save_lesson`]).
#[tauri::command]
async fn save_lesson<R: Runtime>(app: AppHandle<R>, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the lesson bytes as the request body".into());
    };
    let path = PathBuf::from(header(&request, "x-rupok-path")?);
    if !files::is_lesson_path(&path) {
        return Err(files::FileError::Extension.to_string());
    }
    if !may_write(&app, &path) {
        return Err(format!(
            "{} was not chosen in a file dialog",
            path.display()
        ));
    }
    let bytes = bytes.clone();
    let target = path.clone();
    tauri::async_runtime::spawn_blocking(move || files::save_lesson(&target, &bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    // Later saves and re-opens of this exact file need no dialog.
    let _ = app.fs_scope().allow_file(&path);
    Ok(())
}

/// Save an exported PDF: body = PDF bytes, header `x-rupok-path` = target
/// path chosen in the save dialog. Checked and written atomically (see
/// [`files::save_pdf`]).
#[tauri::command]
async fn save_pdf<R: Runtime>(app: AppHandle<R>, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the PDF bytes as the request body".into());
    };
    let path = PathBuf::from(header(&request, "x-rupok-path")?);
    if !files::is_pdf_path(&path) {
        return Err(files::FileError::PdfExtension.to_string());
    }
    if !may_write(&app, &path) {
        return Err(format!(
            "{} was not chosen in a file dialog",
            path.display()
        ));
    }
    let bytes = bytes.clone();
    tauri::async_runtime::spawn_blocking(move || files::save_pdf(&path, &bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
