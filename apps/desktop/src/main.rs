// No console window behind the app in Windows release builds.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "app")]
    rupok_desktop_lib::run();

    #[cfg(not(feature = "app"))]
    {
        eprintln!(
            "rupok-desktop was built without the `app` feature.\n\
             Run it with `bun run dev` in apps/desktop (see apps/desktop/README.md)."
        );
        std::process::exit(2);
    }
}
