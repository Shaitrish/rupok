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
            "library_info",
            "library_list",
            "library_stat",
            "library_read",
            "library_read_range",
            "library_write",
            "library_mkdir",
            "library_rename",
            "library_copy",
            "library_remove",
            "library_choose_root",
            "library_reset_root",
            "library_save_zip",
            "cloud_auth_begin",
            "cloud_auth_wait",
            "agent_start",
            "voice_status",
            "voice_model_download",
            "voice_model_cancel",
            "voice_model_remove",
            "voice_transcribe",
            "open_donate_page",
            "save_speed_report",
        ]),
    ))
    .expect("tauri-build failed");
}
