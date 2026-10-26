//! The window's calls. Anything that touches the disk runs off the main
//! thread, and a panic comes back as an error rather than ending the app.
use crate::state::{AppState, Device, Failure, Outcome, Status, SyncProgress};
use onelibrary_sync::{Fingerprint, Plan};
use std::sync::Arc;
use tauri::State;
use tauri_plugin_opener::OpenerExt as _;

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Outcome<T> + Send + 'static) -> Outcome<T> {
    tauri::async_runtime::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
            .unwrap_or_else(|_| Err(Failure::new("Something went wrong inside OneLibrary 4 All.")))
    })
    .await
    .unwrap_or_else(|_| Err(Failure::new("Something went wrong inside OneLibrary 4 All.")))
}

#[tauri::command]
pub async fn status(state: State<'_, Arc<AppState>>) -> Outcome<Status> {
    let state = Arc::clone(&state);
    blocking(move || Ok(state.status())).await
}

#[tauri::command]
pub async fn list_devices() -> Outcome<Vec<Device>> {
    blocking(|| Ok(AppState::devices())).await
}

#[tauri::command]
pub async fn plan_device(state: State<'_, Arc<AppState>>, path: String) -> Outcome<Plan> {
    let state = Arc::clone(&state);
    blocking(move || state.plan(&path)).await
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "Tauri's State extractor is injected by value")]
pub fn start_sync(state: State<'_, Arc<AppState>>, path: String, fingerprint: Fingerprint) -> Outcome<()> {
    state.start_sync(&path, fingerprint)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "Tauri's State extractor is injected by value")]
pub fn sync_progress(state: State<'_, Arc<AppState>>) -> Vec<SyncProgress> {
    state.sync_progress()
}

#[tauri::command]
pub async fn recover_device(state: State<'_, Arc<AppState>>, path: String) -> Outcome<()> {
    let state = Arc::clone(&state);
    blocking(move || state.recover(&path)).await
}

#[tauri::command]
pub async fn eject_device(state: State<'_, Arc<AppState>>, path: String) -> Outcome<()> {
    let state = Arc::clone(&state);
    blocking(move || state.eject(&path)).await
}

#[tauri::command]
pub async fn open_device<R: tauri::Runtime>(app: tauri::AppHandle<R>, path: String) -> Outcome<()> {
    blocking(move || {
        app.opener()
            .open_path(path, None::<&str>)
            .map_err(|e| Failure::new(format!("The USB could not be opened: {e}")))
    })
    .await
}
