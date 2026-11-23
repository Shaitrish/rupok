//! Screen capture for the overlay's "Save to lesson".
//!
//! - **Windows**: `xcap` (GDI / DXGI, physical pixels).
//! - **macOS**: `xcap` (CoreGraphics). Needs the Screen Recording permission
//!   (System Settings → Privacy & Security → Screen Recording); without it
//!   macOS silently returns only the wallpaper, so [`available`] checks first
//!   and returns [`CaptureError::Permission`].
//! - **Linux X11**: one core-protocol `GetImage` of the root window through
//!   `x11rb`, which speaks the protocol in pure Rust (no libxcb, no
//!   PipeWire), so the desktop app needs no extra system packages.
//! - **Linux Wayland**: apps cannot read the screen without the desktop
//!   portal's interactive prompt, so this returns [`CaptureError::Wayland`]
//!   and the overlay saves the ink alone.
//!
//! The result is a packed frame (see [`crate::geometry::pack_frame`]).

use crate::geometry::Rect;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("screen capture is not available on Wayland")]
    Wayland,
    /// Matched by the overlay page (`SCREEN_PERMISSION_ERROR` in
    /// `apps/board/src/overlay/ink.ts`) to show how to turn it on.
    #[error("Screen Recording permission is off for Rupok")]
    Permission,
    #[error("screen capture is not supported on this platform")]
    Unsupported,
    #[error("screen capture failed: {0}")]
    Failed(String),
}

fn failed(e: impl std::fmt::Display) -> CaptureError {
    CaptureError::Failed(e.to_string())
}

/// Capture the monitor under the screen area `rect` (physical pixels,
/// desktop coordinates). `scale` is that monitor's scale factor: macOS finds
/// displays in points, not pixels.
pub fn capture(rect: Rect, scale: f64) -> Result<Vec<u8>, CaptureError> {
    imp::capture(rect, scale)
}

/// Whether [`capture`] can work in this session, checked before the overlay
/// hides itself so a Wayland session (or a Mac without the Screen Recording
/// permission) does not flicker for nothing.
pub fn available() -> Result<(), CaptureError> {
    imp::available()
}

/// macOS: ask for the Screen Recording permission. The first call shows the
/// system prompt and adds Rupok to the list in System Settings; later calls
/// only return the current state. macOS applies a new grant after the app
/// restarts. Always `true` elsewhere.
pub fn request_permission() -> bool {
    imp::request_permission()
}

#[cfg(windows)]
mod imp {
    use super::{CaptureError, Rect, failed};
    use crate::geometry::pack_frame;

    pub fn available() -> Result<(), CaptureError> {
        Ok(())
    }

    pub fn request_permission() -> bool {
        true
    }

    pub fn capture(rect: Rect, _scale: f64) -> Result<Vec<u8>, CaptureError> {
        let (cx, cy) = rect.center();
        let monitor = xcap::Monitor::from_point(cx, cy).map_err(failed)?;
        let img = monitor.capture_image().map_err(failed)?;
        let (w, h) = img.dimensions();
        pack_frame(w, h, img.into_raw()).ok_or_else(|| failed("unexpected image size"))
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{CaptureError, Rect, failed};
    use crate::geometry::{logical_point, pack_frame};

    // CoreGraphics, macOS 10.15+ (the app's minimum is 11). xcap links the
    // framework already; declared here to avoid another dependency.
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    pub fn available() -> Result<(), CaptureError> {
        // SAFETY: no arguments, no preconditions; never shows UI.
        if unsafe { CGPreflightScreenCaptureAccess() } {
            Ok(())
        } else {
            Err(CaptureError::Permission)
        }
    }

    pub fn request_permission() -> bool {
        // SAFETY: no arguments, no preconditions.
        unsafe { CGRequestScreenCaptureAccess() }
    }

    pub fn capture(rect: Rect, scale: f64) -> Result<Vec<u8>, CaptureError> {
        available()?;
        let (cx, cy) = logical_point(rect.center(), scale);
        let monitor = xcap::Monitor::from_point(cx, cy).map_err(failed)?;
        // Pixels at the display's backing scale (2× on Retina screens).
        let img = monitor.capture_image().map_err(failed)?;
        let (w, h) = img.dimensions();
        pack_frame(w, h, img.into_raw()).ok_or_else(|| failed("unexpected image size"))
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{CaptureError, Rect, failed};
    use crate::geometry::{bgrx_to_rgba, pack_frame};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, ImageOrder};

    /// Wayland sessions (including X11 apps running under XWayland, where the
    /// root window does not contain Wayland windows). `RUPOK_CAPTURE_X11=1`
    /// forces the X11 path, for debugging.
    fn is_wayland() -> bool {
        if std::env::var_os("RUPOK_CAPTURE_X11").is_some_and(|v| v == "1") {
            return false;
        }
        std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
    }

    pub fn available() -> Result<(), CaptureError> {
        if is_wayland() {
            Err(CaptureError::Wayland)
        } else {
            Ok(())
        }
    }

    pub fn request_permission() -> bool {
        true
    }

    pub fn capture(rect: Rect, _scale: f64) -> Result<Vec<u8>, CaptureError> {
        available()?;
        let (conn, screen) = x11rb::connect(None).map_err(failed)?;
        let setup = conn.setup();
        let root = setup
            .roots
            .get(screen)
            .ok_or_else(|| failed("no X11 screen"))?;

        // Clip to the root window; X11 coordinates are 16-bit.
        let x0 = rect.x.clamp(0, i32::from(root.width_in_pixels));
        let y0 = rect.y.clamp(0, i32::from(root.height_in_pixels));
        let x1 = (rect.x + rect.w as i32).clamp(x0, i32::from(root.width_in_pixels));
        let y1 = (rect.y + rect.h as i32).clamp(y0, i32::from(root.height_in_pixels));
        let (w, h) = ((x1 - x0) as u16, (y1 - y0) as u16);
        if w == 0 || h == 0 {
            return Err(failed("the overlay is off screen"));
        }

        let reply = conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                root.root,
                x0 as i16,
                y0 as i16,
                w,
                h,
                !0,
            )
            .map_err(failed)?
            .reply()
            .map_err(failed)?;
        let format = setup.pixmap_formats.iter().find(|f| f.depth == reply.depth);
        let supported = format.is_some_and(|f| f.bits_per_pixel == 32 && f.scanline_pad == 32)
            && setup.image_byte_order == ImageOrder::LSB_FIRST;
        if !supported {
            return Err(failed(format!(
                "unsupported X11 pixel format (depth {})",
                reply.depth
            )));
        }
        let mut px = reply.data;
        bgrx_to_rgba(&mut px);
        pack_frame(u32::from(w), u32::from(h), px).ok_or_else(|| failed("unexpected image size"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Run with `RUPOK_CAPTURE_X11=1 cargo test -p rupok-desktop --features capture -- --ignored`
        /// on an X11 (or XWayland) desktop.
        #[test]
        #[ignore = "needs an X11 display"]
        fn captures_the_top_left_corner() {
            let frame = capture(
                Rect {
                    x: 0,
                    y: 0,
                    w: 64,
                    h: 32,
                },
                1.0,
            )
            .unwrap();
            assert_eq!(&frame[..8], &[64, 0, 0, 0, 32, 0, 0, 0]);
            assert_eq!(frame.len(), 8 + 64 * 32 * 4);
            assert!(frame[8..].as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod imp {
    use super::{CaptureError, Rect};

    pub fn available() -> Result<(), CaptureError> {
        Err(CaptureError::Unsupported)
    }

    pub fn request_permission() -> bool {
        true
    }

    pub fn capture(_rect: Rect, _scale: f64) -> Result<Vec<u8>, CaptureError> {
        Err(CaptureError::Unsupported)
    }
}
