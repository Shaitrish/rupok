//! The Tauri shell: plugins, the main window, file commands, files the
//! operating system asks us to open, and the macOS menu bar.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WindowEvent};
use tauri_plugin_fs::FsExt;
use tauri_plugin_window_state::StateFlags;

use crate::{
    agent_cmd, cloud_cmd, files, library_cmd, native_ink_cmd, overlay, perf_cmd, voice_cmd,
};

/// Label of the board window (`tauri.conf.json`).
pub const MAIN: &str = "main";

/// Tells the board that files are waiting in [`take_pending_files`]. The
/// event carries no payload, so an event sent before the board listens is
/// not lost: the board also drains the queue once at start-up.
const OPEN_FILES_EVENT: &str = "rupok://open-files";

/// Rupok's donate page. Fixed here, so web content can't make the app open
/// any other address.
const DONATE_URL: &str = "https://rupok.app/donate/";

/// Open the donate page in the system browser ("Support Rupok" in Settings,
/// the ⋯ menu and the first-launch card).
#[tauri::command]
fn open_donate_page() -> Result<(), String> {
    crate::cloud_auth::open_in_browser(DONATE_URL)
        .map_err(|e| format!("could not open the browser: {e}"))
}

/// Files the OS asked us to open (argv, file association, second launch),
/// until the board takes them.
#[derive(Default)]
struct PendingFiles(Mutex<Vec<PathBuf>>);

pub fn run() {
    crate::prefer_x11_on_wayland();

    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.menu(macos_menu);

    builder
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
        .manage(library_cmd::LibraryState::default())
        .manage(cloud_cmd::CloudAuthState::default())
        .manage(voice_cmd::VoiceState::default())
        .manage(agent_cmd::AgentState::default())
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
            overlay::overlay_open_screen_settings,
            overlay::overlay_deliver_page,
            overlay::take_overlay_page,
            library_cmd::library_info,
            library_cmd::library_list,
            library_cmd::library_stat,
            library_cmd::library_read,
            library_cmd::library_read_range,
            library_cmd::library_write,
            library_cmd::library_mkdir,
            library_cmd::library_rename,
            library_cmd::library_copy,
            library_cmd::library_remove,
            library_cmd::library_choose_root,
            library_cmd::library_reset_root,
            library_cmd::library_save_zip,
            cloud_cmd::cloud_auth_begin,
            cloud_cmd::cloud_auth_wait,
            agent_cmd::agent_start,
            voice_cmd::voice_status,
            voice_cmd::voice_model_download,
            voice_cmd::voice_model_cancel,
            voice_cmd::voice_model_remove,
            voice_cmd::voice_transcribe,
            open_donate_page,
            perf_cmd::save_speed_report,
            native_ink_cmd::native_ink_start,
            native_ink_cmd::native_ink_begin,
            native_ink_cmd::native_ink_points,
            native_ink_cmd::native_ink_end,
            native_ink_cmd::native_ink_stop,
        ])
        .setup(|app| {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let cwd = std::env::current_dir().unwrap_or_default();
            queue_files(app.handle(), files::paths_from_args(&args, &cwd));
            overlay::register_shortcuts(app.handle());
            voice_cmd::init(app.handle()); // the board may use the microphone (voice input)
            // The lesson library (Documents/Rupok) and its folder watcher.
            if let Err(e) = library_cmd::init(app.handle()) {
                eprintln!("library: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // The overlay windows are only hidden, never closed; quit with the board.
            if window.label() == MAIN && matches!(event, WindowEvent::Destroyed) {
                window.app_handle().exit(0);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while starting Rupok")
        .run(|app, event| {
            // macOS hands files over as events, not argv: a double-clicked
            // lesson in Finder, "Open With", a drop on the Dock icon, at
            // launch or while running.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = event {
                let paths = urls.iter().filter_map(|u| u.to_file_path().ok());
                queue_files(app, files::openable_paths(paths));
                focus_main(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}

/// The macOS menu bar: the app menu (About, Hide ⌘H, Quit ⌘Q), Edit (which
/// also makes ⌘C/⌘V/⌘X/⌘A work in text fields: WKWebView only gets them
/// through the menu), View (full screen) and Window. The board handles ⌘Z,
/// ⌘C, ⌘S… itself first; the menu acts only on keys the page leaves alone.
#[cfg(target_os = "macos")]
fn macos_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID};

    let name = app.package_info().name.clone();
    let about = AboutMetadata {
        name: Some(name.clone()),
        version: Some(app.package_info().version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        credits: Some("Open-source whiteboard for teachers. https://rupok.app".into()),
        ..Default::default()
    };
    let sep = || PredefinedMenuItem::separator(app);
    let app_menu = Submenu::with_items(
        app,
        &name,
        true,
        &[
            &PredefinedMenuItem::about(app, Some(&format!("About {name}")), Some(about))?,
            &sep()?,
            &PredefinedMenuItem::services(app, None)?,
            &sep()?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &sep()?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &sep()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app, None)?],
    )?;
    // This id makes it the system Window menu, which lists the open windows.
    let window = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
            &sep()?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit, &view, &window])
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
pub(crate) fn may_write<R: Runtime>(app: &AppHandle<R>, path: &Path) -> bool {
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
