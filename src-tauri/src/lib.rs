//! OneLibrary Sync: keeps the Device Library on each connected USB stick in
//! line with the stick's OneLibrary, so players that read only the Device
//! Library see the same tracks and playlists. The sync itself is
//! `onelibrary_sync`'s; this is the window around it.
mod commands;
mod processes;
mod state;
mod update;

use std::sync::Arc;
use tauri::{Emitter as _, Manager as _};

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(Arc::new(state::AppState::default()));
            app.manage(Arc::new(update::Updates::default()));
            update::Updates::clear_stale(app.handle());
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
            update::check_for_update,
            update::download_update,
            update::restart_to_update,
        ])
        .build(tauri::generate_context!());
    match built {
        // An update staged on Windows installs as the app quits.
        Ok(app) => app.run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                update::on_exit(app);
            }
        }),
        Err(error) => tracing::error!(%error, "OneLibrary Sync could not start"),
    }
}
