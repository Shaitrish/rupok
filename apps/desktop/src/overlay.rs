//! "Draw over any app": a transparent, undecorated, always-on-top ink window
//! covering one monitor (`/board/?overlay`), plus a small floating toolbar
//! window (`/board/?overlay=toolbar`) that never ignores the cursor, so the
//! toolbar stays usable while the ink window lets clicks through to the apps
//! underneath (`set_ignore_cursor_events`).
//!
//! The windows are created on first use and afterwards only hidden, so the
//! ink survives toggling the overlay off and on.
//!
//! macOS (beta): both windows also join every Space, float over full-screen
//! apps and sit above the menu bar and the Dock (see [`macos`]); capture
//! needs the Screen Recording permission ([`overlay_open_screen_settings`]).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, State,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::app::MAIN;
use crate::capture;
use crate::geometry::{self, Rect};

/// Label of the ink window.
pub const INK: &str = "overlay";
/// Label of the floating toolbar window.
pub const TOOLBAR: &str = "overlay-toolbar";

/// Global shortcut that shows or hides the overlay.
const TOGGLE: &str = "ctrl+shift+d";
/// Sent to both overlay windows with the click-through state (`bool`).
const CLICK_THROUGH_EVENT: &str = "rupok://overlay-click-through";
/// Sent to the board window when a snapshot waits in [`take_overlay_page`].
const PAGE_EVENT: &str = "rupok://overlay-page";

/// Toolbar size (logical px) until the toolbar page measures itself.
const TOOLBAR_DEFAULT: (f64, f64) = (640.0, 80.0);
/// Extra gap between the toolbar window and the bottom of the work area
/// (logical px). The page already pads the bar inside its window (room for
/// the shadow), which gives the visible gap.
const TOOLBAR_MARGIN: f64 = 0.0;
/// Time for the compositor to remove our windows before the screenshot.
const HIDE_SETTLE: Duration = Duration::from_millis(250);
/// Upper bound for one snapshot sent to the board (an 8K PNG fits).
const MAX_PAGE_BYTES: usize = 128 << 20;

#[derive(Default)]
pub struct OverlayState {
    click_through: AtomicBool,
    toolbar_size: Mutex<Option<(f64, f64)>>,
    /// Snapshots (PNG or WebP) waiting for the board, oldest first.
    pages: Mutex<VecDeque<Vec<u8>>>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ------------------------------------------------------------------ windows

fn url(query: &str) -> WebviewUrl {
    // A leading slash makes `Url::join` replace the path of the dev URL
    // (`http://127.0.0.1:3737/board/`) and of `tauri://localhost` alike.
    WebviewUrl::App(PathBuf::from(format!("/board/?{query}")))
}

fn rect_of(m: &Monitor) -> Rect {
    let (p, s) = (m.position(), m.size());
    Rect {
        x: p.x,
        y: p.y,
        w: s.width,
        h: s.height,
    }
}

/// The monitor under the mouse, else the board's, else the primary one.
fn target_monitor(app: &AppHandle) -> Option<Monitor> {
    app.cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| {
            app.get_webview_window(MAIN)
                .and_then(|w| w.current_monitor().ok().flatten())
        })
        .or_else(|| app.primary_monitor().ok().flatten())
}

fn ensure_windows(app: &AppHandle) -> tauri::Result<(WebviewWindow, WebviewWindow)> {
    let ink = match app.get_webview_window(INK) {
        Some(w) => w,
        None => {
            let win = WebviewWindowBuilder::new(app, INK, url("overlay"))
                .title("Rupok overlay")
                .transparent(true)
                .decorations(false)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .visible(false)
                .build()?;
            #[cfg(target_os = "macos")]
            macos::float_everywhere(&win, macos::INK_LEVEL);
            win
        }
    };
    let toolbar = match app.get_webview_window(TOOLBAR) {
        Some(w) => w,
        None => {
            let (w, h) = TOOLBAR_DEFAULT;
            let win = WebviewWindowBuilder::new(app, TOOLBAR, url("overlay=toolbar"))
                .title("Rupok overlay tools")
                .transparent(true)
                .decorations(false)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .focused(false)
                .inner_size(w, h)
                .visible(false)
                // Owned by (Windows) / transient for (Linux) / a child window
                // of (macOS) the ink window, so it always stays above it, even
                // after the ink takes focus.
                .parent(&ink)?
                .build()?;
            #[cfg(target_os = "macos")]
            macos::float_everywhere(&win, macos::TOOLBAR_LEVEL);
            win
        }
    };
    Ok((ink, toolbar))
}

/// Cover the monitor `r` exactly (move, size, move again: see `place_toolbar`).
fn place_ink(ink: &WebviewWindow, r: Rect) -> tauri::Result<()> {
    let pos = PhysicalPosition::new(r.x, r.y);
    ink.set_position(pos)?;
    ink.set_size(PhysicalSize::new(r.w, r.h))?;
    ink.set_position(pos)
}

/// Size the toolbar window and centre it near the bottom of `monitor`.
fn place_toolbar(app: &AppHandle, toolbar: &WebviewWindow, monitor: &Monitor) -> tauri::Result<()> {
    let state = app.state::<OverlayState>();
    let (w, h) = state
        .toolbar_size
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or(TOOLBAR_DEFAULT);
    let scale = monitor.scale_factor();
    let (pw, ph) = ((w * scale).round() as u32, (h * scale).round() as u32);
    let margin = (TOOLBAR_MARGIN * scale).round() as u32;
    // The work area excludes the Windows taskbar and Linux panels.
    let area = monitor.work_area();
    let area = Rect {
        x: area.position.x,
        y: area.position.y,
        w: area.size.width,
        h: area.size.height,
    };
    let (x, y) = geometry::toolbar_position(area, pw, ph, margin);
    // Move first: crossing to a monitor with another scale makes the OS
    // rescale the window, so the physical size is set once it is there.
    let pos = PhysicalPosition::new(x, y);
    toolbar.set_position(pos)?;
    toolbar.set_size(PhysicalSize::new(pw, ph))?;
    toolbar.set_position(pos)
}

fn set_click_through(app: &AppHandle, on: bool) -> tauri::Result<()> {
    app.state::<OverlayState>()
        .click_through
        .store(on, Ordering::Relaxed);
    if let Some(ink) = app.get_webview_window(INK) {
        ink.set_ignore_cursor_events(on)?;
        if !on {
            let _ = ink.set_focus();
        }
    }
    let _ = app.emit_to(INK, CLICK_THROUGH_EVENT, on);
    let _ = app.emit_to(TOOLBAR, CLICK_THROUGH_EVENT, on);
    Ok(())
}

fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(INK)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// Cover the monitor under the mouse, in draw mode, with the toolbar on top.
pub fn show(app: &AppHandle) -> tauri::Result<()> {
    let (ink, toolbar) = ensure_windows(app)?;
    if let Some(m) = target_monitor(app) {
        place_ink(&ink, rect_of(&m))?;
        place_toolbar(app, &toolbar, &m)?;
    }
    ink.show()?;
    toolbar.show()?;
    set_click_through(app, false)?;
    ink.set_focus()
}

pub fn hide(app: &AppHandle) -> tauri::Result<()> {
    for label in [TOOLBAR, INK] {
        if let Some(w) = app.get_webview_window(label) {
            w.hide()?;
        }
    }
    Ok(())
}

pub fn toggle(app: &AppHandle) -> tauri::Result<()> {
    if is_visible(app) {
        hide(app)
    } else {
        show(app)
    }
}

// ---------------------------------------------------------------- shortcuts

/// Register Ctrl+Shift+D. Failure is not fatal: another app may own the
/// combination, and Wayland has no global shortcuts for apps.
pub fn register_shortcuts(app: &AppHandle) {
    if let Err(e) = app.global_shortcut().register(TOGGLE) {
        eprintln!("rupok: could not register the {TOGGLE} overlay shortcut: {e}");
    }
}

pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed
        || !shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyD)
    {
        return;
    }
    // Window creation must not run inside the event-loop callback (it would
    // wait on the loop it blocks); do it on the async runtime.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = toggle(&app) {
            eprintln!("rupok: overlay toggle failed: {e}");
        }
    });
}

// ----------------------------------------------------------------- commands
// Window-creating commands are `async`: synchronous commands run on the main
// thread, where creating a webview deadlocks on Windows.

#[tauri::command]
pub async fn overlay_show(app: AppHandle) -> Result<(), String> {
    show(&app).map_err(err)
}

#[tauri::command]
pub async fn overlay_toggle(app: AppHandle) -> Result<(), String> {
    toggle(&app).map_err(err)
}

#[tauri::command]
pub async fn overlay_hide(app: AppHandle) -> Result<(), String> {
    hide(&app).map_err(err)
}

#[tauri::command]
pub async fn overlay_set_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    set_click_through(&app, enabled).map_err(err)
}

/// The toolbar page reports its rendered size (CSS px) so the window fits it
/// exactly, whatever the density setting or the design.
#[tauri::command]
pub async fn overlay_toolbar_fit(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if !(width.is_finite() && height.is_finite()) {
        return Err("invalid toolbar size".into());
    }
    let size = (
        width.clamp(80.0, 2000.0).ceil(),
        height.clamp(40.0, 600.0).ceil(),
    );
    *app.state::<OverlayState>()
        .toolbar_size
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(size);
    let (Some(ink), Some(toolbar)) = (app.get_webview_window(INK), app.get_webview_window(TOOLBAR))
    else {
        return Ok(());
    };
    match ink.current_monitor().map_err(err)? {
        Some(m) => place_toolbar(&app, &toolbar, &m).map_err(err),
        None => toolbar
            .set_size(LogicalSize::new(size.0, size.1))
            .map_err(err),
    }
}

/// Screenshot of the monitor under the ink window, without our windows:
/// `[w: u32 LE][h: u32 LE][RGBA…]` (see [`geometry::pack_frame`]). The ink
/// window composites its ink on top and sends the page back with
/// [`overlay_deliver_page`]. Fails on Wayland; the page then saves ink only.
#[tauri::command]
pub async fn overlay_capture(
    app: AppHandle,
    state: State<'_, OverlayState>,
) -> Result<Response, String> {
    capture::available().map_err(err)?;
    let ink = app
        .get_webview_window(INK)
        .ok_or("the overlay is not open")?;
    let toolbar = app.get_webview_window(TOOLBAR);
    let (pos, size) = (
        ink.outer_position().map_err(err)?,
        ink.outer_size().map_err(err)?,
    );
    let scale = ink.scale_factor().unwrap_or(1.0);
    let rect = Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };

    if let Some(t) = &toolbar {
        let _ = t.hide();
    }
    let _ = ink.hide();
    let shot = tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(HIDE_SETTLE);
        capture::capture(rect, scale)
    })
    .await;
    let _ = ink.show();
    let _ = place_ink(&ink, rect);
    if let Some(t) = &toolbar {
        let _ = t.show();
    }
    let through = state.click_through.load(Ordering::Relaxed);
    let _ = ink.set_ignore_cursor_events(through);
    if !through {
        let _ = ink.set_focus();
    }

    let frame = shot.map_err(err)?.map_err(err)?;
    Ok(Response::new(frame))
}

/// System Settings → Privacy & Security → Screen Recording.
#[cfg(target_os = "macos")]
const SCREEN_RECORDING_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

/// macOS, after [`overlay_capture`] failed for lack of the Screen Recording
/// permission: hide the overlay (it floats above System Settings), ask for
/// the permission (which adds Rupok to the list there) and open that pane.
/// macOS applies the switch after Rupok restarts.
#[tauri::command]
pub async fn overlay_open_screen_settings(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        hide(&app).map_err(err)?;
        app.run_on_main_thread(|| {
            capture::request_permission();
        })
        .map_err(err)?;
        tauri::async_runtime::spawn_blocking(|| {
            std::process::Command::new("/usr/bin/open")
                .arg(SCREEN_RECORDING_SETTINGS)
                .status()
        })
        .await
        .map_err(err)?
        .map_err(err)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("Screen Recording settings exist only on macOS".into())
    }
}

fn is_png_or_webp(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG\r\n\x1a\n")
        || (b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP")
}

/// The overlay hands over a finished snapshot (PNG or WebP body); the board
/// window is told to fetch it with [`take_overlay_page`].
#[tauri::command]
pub async fn overlay_deliver_page(app: AppHandle, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the image as the request body".into());
    };
    if bytes.len() > MAX_PAGE_BYTES || !is_png_or_webp(bytes) {
        return Err("expected a PNG or WebP image".into());
    }
    let state = app.state::<OverlayState>();
    state
        .pages
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push_back(bytes.clone());
    app.emit_to(MAIN, PAGE_EVENT, ()).map_err(err)
}

/// The oldest waiting snapshot (PNG or WebP bytes).
#[tauri::command]
pub async fn take_overlay_page(state: State<'_, OverlayState>) -> Result<Response, String> {
    let page = state
        .pages
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pop_front();
    page.map(Response::new)
        .ok_or_else(|| "no overlay snapshot is waiting".into())
}

// -------------------------------------------------------------------- macOS

/// What the overlay needs on macOS beyond Tauri's `always_on_top`, which uses
/// the floating level (under the menu bar and the Dock) on the current Space
/// only.
#[cfg(target_os = "macos")]
mod macos {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use tauri::WebviewWindow;

    /// `NSStatusWindowLevel`: above the menu bar and the Dock.
    pub const INK_LEVEL: isize = 25;
    /// One higher, so the toolbar always stays above the ink.
    pub const TOOLBAR_LEVEL: isize = INK_LEVEL + 1;

    // `NSWindowCollectionBehavior` bits.
    const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const STATIONARY: usize = 1 << 4;
    const IGNORES_CYCLE: usize = 1 << 6;
    const FULL_SCREEN_AUXILIARY: usize = 1 << 8;

    /// Show `window` on every Space and over full-screen apps (a slideshow in
    /// Keynote or PowerPoint), keep it out of Mission Control and the ⌘`
    /// window cycle, and lift it to `level`.
    pub fn float_everywhere(window: &WebviewWindow, level: isize) {
        let w = window.clone();
        let _ = window.run_on_main_thread(move || {
            let Ok(ptr) = w.ns_window() else { return };
            let ns = ptr.cast::<AnyObject>();
            if ns.is_null() {
                return;
            }
            let behavior = CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY;
            // SAFETY: `ns` is this window's live NSWindow, used on the main
            // thread; both setters exist on every supported macOS. The
            // behaviour bits are one valid choice from each exclusive group.
            unsafe {
                let _: () = msg_send![ns, setCollectionBehavior: behavior];
                let _: () = msg_send![ns, setLevel: level];
            }
        });
    }
}
