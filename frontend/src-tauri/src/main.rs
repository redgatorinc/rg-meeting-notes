#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use log;
use env_logger;

fn main() {
    // Respect an existing RUST_LOG (dev workflow) but default to a baseline
    // that silences the xcap crate — it logs ERROR for every window it can't
    // open via GetModuleBaseNameW / GetFileVersionInfoSizeW (elevated or
    // system-owned processes), which is noise, not a real failure.
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info,xcap=off");
    }
    env_logger::init();

    // Async logger will be initialized lazily when first needed (after Tauri runtime starts)
    log::info!("Starting application...");
    app_lib::run();
}
