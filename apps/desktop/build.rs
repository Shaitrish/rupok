fn main() {
    // whisper.cpp's Metal backend uses Objective-C `@available` checks, which
    // call `__isPlatformVersionAtLeast` from clang's runtime library. rustc
    // links with `cc` but not that library, so link it here (feature
    // `metal`, macOS targets only).
    if std::env::var_os("CARGO_FEATURE_METAL").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        link_clang_runtime();
    }
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
            "native_ink_start",
            "native_ink_begin",
            "native_ink_points",
            "native_ink_end",
            "native_ink_stop",
        ]),
    ))
    .expect("tauri-build failed");
}

/// Add `libclang_rt.osx.a` from the active Xcode clang to the link.
fn link_clang_runtime() {
    let out = std::process::Command::new("xcrun")
        .args(["clang", "--print-resource-dir"])
        .output()
        .expect("xcrun clang --print-resource-dir");
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let lib = std::path::Path::new(&dir).join("lib/darwin");
    assert!(
        lib.join("libclang_rt.osx.a").exists(),
        "libclang_rt.osx.a not found in {}",
        lib.display()
    );
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
