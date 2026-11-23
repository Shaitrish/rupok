fn main() {
    // The Tauri build step only runs for the real app (`--features app`, which
    // `tauri dev` / `tauri build` pass); plain `cargo test` skips it.
    #[cfg(feature = "app")]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        // Generates `allow-<command>` permissions, so each window's capability
        // file lists exactly the commands it may call.
        tauri_build::AppManifest::new().commands(&[
            "take_pending_files",
            "save_lesson",
            "save_pdf",
            "take_overlay_page",
            "overlay_show",
            "overlay_toggle",
            "overlay_hide",
            "overlay_set_click_through",
            "overlay_toolbar_fit",
            "overlay_capture",
            "overlay_open_screen_settings",
            "overlay_deliver_page",
        ]),
    ))
    .expect("tauri-build failed");
}
