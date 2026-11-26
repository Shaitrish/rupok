//! Commands of the native wet-ink layer (native_ink.rs; the board's side is
//! apps/board/src/nativeInk.ts). Plain (not async) commands: Tauri runs them
//! on the main thread, which owns the layer's window and its graphics
//! objects. Every error goes back to the board, which then switches the
//! layer off and draws wet ink itself; on other systems they all say no.

use tauri::{Runtime, WebviewWindow};

#[cfg(not(windows))]
const WINDOWS_ONLY: &str = "the native wet-ink layer is for Windows";

/// Make the layer over the board window (hidden until a stroke starts).
#[tauri::command]
pub fn native_ink_start<R: Runtime>(window: WebviewWindow<R>) -> Result<(), String> {
    #[cfg(windows)]
    {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        crate::native_ink::start(hwnd.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Err(WINDOWS_ONLY.into())
    }
}

/// A stroke starts: `point` is x, y, radius in pixels of the board window's
/// client area; `clip` the page on screen (left, top, right, bottom).
#[tauri::command]
pub fn native_ink_begin(
    seq: u32,
    rgb: [u8; 3],
    clip: [f32; 4],
    point: [f32; 3],
) -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::native_ink::begin(seq, rgb, clip, point)
    }
    #[cfg(not(windows))]
    {
        let _ = (seq, rgb, clip, point);
        Err(WINDOWS_ONLY.into())
    }
}

/// More samples of stroke `seq`: flat `[x, y, radius, …]`.
#[tauri::command]
pub fn native_ink_points(seq: u32, pts: Vec<f32>) -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::native_ink::points(seq, &pts)
    }
    #[cfg(not(windows))]
    {
        let _ = (seq, pts);
        Err(WINDOWS_ONLY.into())
    }
}

/// The board shows stroke `seq` itself now: the layer drops its copy.
#[tauri::command]
pub fn native_ink_end(seq: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::native_ink::end(seq)
    }
    #[cfg(not(windows))]
    {
        let _ = seq;
        Err(WINDOWS_ONLY.into())
    }
}

/// Destroy the layer (the setting was switched off, or after an error).
#[tauri::command]
pub fn native_ink_stop() -> Result<(), String> {
    #[cfg(windows)]
    crate::native_ink::stop();
    Ok(())
}
