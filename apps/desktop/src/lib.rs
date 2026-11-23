//! Rupok desktop shell (Tauri 2) for Windows and Linux.
//!
//! The board itself (SolidJS + the `rupok-wasm` engine) runs unchanged in the
//! webview; this crate only adds what a browser cannot do: native file
//! dialogs and crash-safe saving, `.rpk` file association and single-instance
//! file opening, and the "draw over any app" overlay.
//!
//! The Tauri parts are behind the `app` feature (see `Cargo.toml`); the file
//! and geometry helpers are plain Rust and always built and tested.

pub mod files;
pub mod geometry;

#[cfg(feature = "capture")]
pub mod capture;

#[cfg(feature = "app")]
mod app;
#[cfg(feature = "app")]
mod overlay;

#[cfg(feature = "app")]
pub use app::run;

/// Wayland does not let apps place windows or keep them above others, which
/// the overlay needs (and global shortcuts need X11). On a Wayland session,
/// unless the user opts out (`RUPOK_WAYLAND=1`) or picked a GTK backend, run
/// under XWayland. Call first thing in `main`, before any thread is spawned.
pub fn prefer_x11_on_wayland() {
    #[cfg(target_os = "linux")]
    {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
        if set("WAYLAND_DISPLAY") && set("DISPLAY") && !set("GDK_BACKEND") && !set("RUPOK_WAYLAND")
        {
            // SAFETY: documented precondition: no other thread exists yet.
            unsafe { std::env::set_var("GDK_BACKEND", "x11") };
        }
    }
}
