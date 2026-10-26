//! OneLibrary 4 All: keeps the Device Library on each connected USB stick in
//! line with the stick's OneLibrary, so players that read only the Device
//! Library see the same tracks and playlists. The sync itself is
//! `onelibrary_sync`'s; this is the window around it.
mod commands;
mod processes;
mod state;

use std::sync::Arc;
use tauri::{Emitter as _, Manager as _};

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(Arc::new(state::AppState::default()));
            // A stick plugged in or pulled out refreshes the list at once,
            // whether or not the window has focus.
            let handle = app.handle().clone();
            let watcher = rbl_devices::MountWatcher::start(rbl_devices::mounts::INTERVAL, move || {
                if let Err(error) = handle.emit("devices-changed", ()) {
                    tracing::warn!(%error, "could not tell the window the devices changed");
                }
            });
            // Kept for the life of the app; dropping it stops the thread.
            app.manage(watcher);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::list_devices,
            commands::plan_device,
            commands::start_sync,
            commands::sync_progress,
            commands::recover_device,
            commands::eject_device,
            commands::open_device,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        tracing::error!(%error, "OneLibrary 4 All could not start");
    }
}
