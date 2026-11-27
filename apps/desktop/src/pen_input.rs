//! Linux (WebKitGTK): let every pointer sample reach the board.
//!
//! GTK merges pointer motion: while a window is being painted, the motion
//! events that arrive are dropped except the last ("event compression", on
//! by default). The board repaints on every frame while the teacher writes,
//! so it got about one sample per frame, and fewer when frames were late: a
//! 125 Hz mouse or a smartboard's touch digitiser arrived at 30 to 60 Hz and
//! fast curves had 30 px between samples. GTK's own advice for drawing apps
//! is to turn the compression off; WebKit then hands the page every sample
//! (`getCoalescedEvents`), which is what `controller.ts` already reads.
//!
//! Measured in the web view with a 125 Hz pointer on the board
//! (docs/performance.md, "WebKitGTK notes"): 57 to 86 samples a second with
//! compression, all 121 without, at the same frame rate.
//!
//! Hardware-accelerated compositing is also asked for outright rather than
//! left to the library's default (which was "on demand" before WebKitGTK
//! 2.42): the board is one large WebGL canvas.

use tauri::{Runtime, WebviewWindow};

/// Tune a window that takes ink: the board and the draw-over-apps layer.
pub fn tune<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    {
        let _ = window.with_webview(|wv| {
            use gtk::prelude::WidgetExt;
            use webkit2gtk::{HardwareAccelerationPolicy, SettingsExt, WebViewExt};

            let view = wv.inner();
            if let Some(s) = WebViewExt::settings(&view) {
                s.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);
            }
            // The web view has its own GDK window, made when it is realized.
            let every_sample = |v: &webkit2gtk::WebView| {
                if let Some(w) = v.window() {
                    w.set_event_compression(false);
                }
            };
            every_sample(&view);
            view.connect_realize(every_sample);
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = window;
}
