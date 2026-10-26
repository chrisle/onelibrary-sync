//! Software updates, as RBXport takes them: found, fetched and put in place
//! without asking.
//!
//! The release workflow publishes `onelibrary-sync-latest.json` beside
//! RBXport's feed, in the updater's own format: the version, the date and a
//! signed URL per platform. The plugin fetches it, compares the version with
//! this build's and verifies the signature against the public key in
//! `tauri.conf.json`; this module turns the result into what the window shows
//! and does what it asks.
//!
//! How "in place" happens depends on what the plugin can do while the app
//! runs:
//!
//! - macOS and a Linux `AppImage`: the bundle on disk is swapped straight
//!   after the download. The running process is not touched; the next launch
//!   runs the new one.
//! - Windows: the installer has to close the app to write over it, so the
//!   download is staged on disk and the installer runs silently when the app
//!   quits ([`on_exit`]). Restart Now runs it at once instead.
//! - A Linux package (`.deb`): installing needs a password prompt, which is
//!   not quiet, so the download is staged and installed only from Restart Now.
//!
//! The download reports through one event, `update:progress`, at most about
//! ten times a second.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::utils::config::BundleType;
use tauri::utils::platform::bundle_type;
use tauri::{AppHandle, Emitter as _, Manager as _};
use tauri_plugin_updater::{Update, UpdaterExt as _};

use crate::state::{Failure, Outcome};

/// The event a download's progress goes out on.
pub const PROGRESS_EVENT: &str = "update:progress";

/// The least time between two progress events.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// Where a downloaded update stands.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Placement {
    /// On disk in the app's place; a restart runs it.
    Installed,
    /// An installer at this path, to run when the app quits (Windows) or on
    /// request (a Linux package).
    Staged(PathBuf),
}

/// One update as the check found it and as far as it has got.
struct Pending {
    /// The plugin's `Update` carries the URL and signature the check found;
    /// the download must use that same one rather than check again, or the
    /// version taken could differ from the version shown.
    update: Update,
    placement: Option<Placement>,
}

/// The update the last check found, held across the download and whatever
/// follows it.
#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Pending>>,
    /// One download at a time: a second request while the first runs would
    /// fetch the installer twice and put it in place twice.
    busy: AtomicBool,
}

impl Updates {
    /// Where a staged installer is kept between the download and the quit.
    fn staging_dir(app: &AppHandle) -> Option<PathBuf> {
        app.path().app_cache_dir().ok().map(|dir| dir.join("update"))
    }

    /// Clears what an earlier run staged and did not use: a crash, or an
    /// installer the quit already ran. The next check downloads afresh, so the
    /// signature is checked on bytes that just came down, not on a file that
    /// has sat in a cache since.
    pub fn clear_stale(app: &AppHandle) {
        if let Some(dir) = Self::staging_dir(app) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Whether the bundle this process runs from can be swapped on disk while it
/// runs, so an update is in place the moment it is downloaded.
fn swaps_in_place() -> bool {
    matches!(bundle_type(), Some(BundleType::App | BundleType::AppImage))
}

/// Why this build cannot replace itself, if it cannot. A release Linux build
/// without a bundle marker was installed some other way and cannot write its
/// own executable, so it says so rather than download and fail every launch.
fn self_update_refusal(debug: bool, bundle: Option<&BundleType>) -> Option<Failure> {
    if debug {
        return Some(Failure::new("A development build cannot be updated in place."));
    }
    // `cfg!` rather than an attribute keeps `bundle` used on every platform.
    if cfg!(target_os = "linux") && bundle.is_none() {
        return Some(Failure::new(
            "This build does not manage its own updates; use the package manager it was installed with.",
        ));
    }
    None
}

/// What a check found.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    /// The version on offer, or `None` when this build is the newest.
    pub version: Option<String>,
    /// The version on offer is already downloaded this run.
    pub ready: Option<UpdateReady>,
}

/// A downloaded update, and whether it is already in the app's place.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReady {
    pub version: String,
    /// `true` when the next launch runs it as things stand; `false` when an
    /// installer still has to run, at the quit or on request.
    pub installed: bool,
}

/// How far a download has got.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub downloaded: u64,
    /// `None` when the server did not say how big the file is.
    pub total: Option<u64>,
}

/// An updater failure, as the window shows it and as the log keeps it. The
/// plugin's own messages are below the level the log keeps, so without this
/// a report of an update that would not install says nothing about why.
fn updater_error(message: &str, error: impl std::fmt::Display) -> Failure {
    tracing::warn!(%error, bundle = bundle_name(bundle_type().as_ref()), "{message}");
    Failure::new(message)
}

/// The bundle this process runs from, as a log field: the updater takes a
/// different route for each one.
fn bundle_name(bundle: Option<&BundleType>) -> &'static str {
    match bundle {
        Some(BundleType::App) => "app",
        Some(BundleType::Dmg) => "dmg",
        Some(BundleType::AppImage) => "appimage",
        Some(BundleType::Deb) => "deb",
        Some(BundleType::Rpm) => "rpm",
        Some(BundleType::Nsis) => "nsis",
        Some(BundleType::Msi) => "msi",
        None => "none",
    }
}

/// Asks the download server what the newest version is. A build that cannot
/// reach it says so rather than "up to date".
#[tauri::command]
pub async fn check_for_update(app: AppHandle, updates: tauri::State<'_, Arc<Updates>>) -> Outcome<UpdateCheck> {
    let current_version = app.package_info().version.to_string();
    let found = app
        .updater()
        .map_err(|e| updater_error("The updater is not configured.", e))?
        .check()
        .await
        .map_err(|e| updater_error("The update check could not reach the download server.", e))?;
    let Some(update) = found else {
        *updates.pending.lock() = None;
        return Ok(UpdateCheck { current_version, version: None, ready: None });
    };
    // A check that finds the version already taken this run keeps that rather
    // than fetching it again.
    let mut pending = updates.pending.lock();
    let placement = pending.as_ref().filter(|p| p.update.version == update.version).and_then(|p| p.placement.clone());
    let ready = placement
        .as_ref()
        .map(|p| UpdateReady { version: update.version.clone(), installed: *p == Placement::Installed });
    let check = UpdateCheck { current_version, version: Some(update.version.clone()), ready };
    *pending = Some(Pending { update, placement });
    Ok(check)
}

/// Downloads the update the last check found and puts it in place, or stages
/// it for [`on_exit`] or [`restart_to_update`] where the bundle cannot be
/// swapped while the app runs.
#[tauri::command]
pub async fn download_update(app: AppHandle, updates: tauri::State<'_, Arc<Updates>>) -> Outcome<UpdateReady> {
    if let Some(refusal) = self_update_refusal(cfg!(debug_assertions), bundle_type().as_ref()) {
        return Err(refusal);
    }
    let (update, placement) = {
        let pending = updates.pending.lock();
        let Some(pending) = pending.as_ref() else {
            return Err(Failure::new("There is no update to download. Check for updates first."));
        };
        (pending.update.clone(), pending.placement.clone())
    };
    if let Some(placement) = placement {
        return Ok(UpdateReady { version: update.version, installed: placement == Placement::Installed });
    }
    if updates.busy.swap(true, Ordering::SeqCst) {
        return Err(Failure::new("The update is already downloading."));
    }

    let handle = app.clone();
    let downloaded = AtomicU64::new(0);
    // `None` until the first chunk, so the first event goes out at once.
    let mut last_sent: Option<Instant> = None;
    let bytes = update
        .download(
            |chunk, total| {
                let so_far = downloaded.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                let now = Instant::now();
                if last_sent.is_some_and(|at| now.duration_since(at) < PROGRESS_EVERY) {
                    return;
                }
                last_sent = Some(now);
                let _ = handle.emit(PROGRESS_EVENT, Progress { downloaded: so_far, total });
            },
            || {
                // The last event carries the full size, so a bar that missed
                // the final chunk still reaches its end.
                let all = downloaded.load(Ordering::Relaxed);
                let _ = app.emit(PROGRESS_EVENT, Progress { downloaded: all, total: Some(all) });
            },
        )
        .await;
    let placed = match bytes {
        Ok(bytes) => place(&app, &update, bytes).await,
        Err(e) => Err(updater_error("The update could not be downloaded.", e)),
    };
    updates.busy.store(false, Ordering::SeqCst);
    let placement = placed?;

    let installed = placement == Placement::Installed;
    if let Some(pending) = updates.pending.lock().as_mut() {
        if pending.update.version == update.version {
            pending.placement = Some(placement);
        }
    }
    Ok(UpdateReady { version: update.version, installed })
}

/// Puts downloaded bytes where the next launch, the quit or a request finds
/// them. The install is file I/O the plugin does synchronously, so it runs on
/// a blocking worker.
async fn place(app: &AppHandle, update: &Update, bytes: Vec<u8>) -> Outcome<Placement> {
    if swaps_in_place() {
        let update = update.clone();
        tauri::async_runtime::spawn_blocking(move || update.install(bytes))
            .await
            .map_err(|e| updater_error("The update could not be installed.", e))?
            .map_err(|e| updater_error("The update could not be installed.", e))?;
        return Ok(Placement::Installed);
    }
    let dir = Updates::staging_dir(app).ok_or_else(|| Failure::new("There is nowhere to keep the update."))?;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| updater_error("The update could not be kept.", e))?;
    let path = dir.join(format!("{}.update", update.version));
    std::fs::write(&path, &bytes).map_err(|e| updater_error("The update could not be kept.", e))?;
    Ok(Placement::Staged(path))
}

/// Runs the update that is ready: restarts into an installed one, or runs a
/// staged installer, which relaunches the app itself when it is done.
///
/// A success never returns, since the process ends either way. Returning is
/// failure, and the window treats it as one.
#[tauri::command]
pub async fn restart_to_update(app: AppHandle, updates: tauri::State<'_, Arc<Updates>>) -> Outcome<()> {
    let (update, placement) = {
        let pending = updates.pending.lock();
        let Some(pending) = pending.as_ref() else {
            return Err(Failure::new("There is no update to restart into."));
        };
        match pending.placement.clone() {
            Some(placement) => (pending.update.clone(), placement),
            None => return Err(Failure::new("The update has not been downloaded yet.")),
        }
    };
    tracing::info!(
        version = %update.version,
        bundle = bundle_name(bundle_type().as_ref()),
        staged = matches!(placement, Placement::Staged(_)),
        "restarting into the update"
    );
    match placement {
        Placement::Installed => app.restart(),
        Placement::Staged(path) => {
            // Taken out first: `install` ends the process on Windows, and the
            // quit that follows must not run the same installer again.
            if let Some(pending) = updates.pending.lock().as_mut() {
                pending.placement = None;
            }
            let bytes = std::fs::read(&path).map_err(|e| updater_error("The update could not be read back.", e))?;
            // On Windows this starts the installer and ends the process; a
            // Linux package is installed in place and the process is still
            // here, so a restart runs it.
            let to_install = update.clone();
            let outcome = tauri::async_runtime::spawn_blocking(move || to_install.install(bytes))
                .await
                .map_err(|e| updater_error("The update could not be installed.", e))?
                .map_err(|e| updater_error("The update could not be installed.", e));
            if let Err(error) = outcome {
                // Still downloaded: a refused password prompt (Linux) can be
                // answered by pressing Restart Now again, without a second
                // download.
                keep_staged(&updates, &update.version, path);
                return Err(error);
            }
            app.restart()
        }
    }
}

/// Puts a staged installer back as the pending update's placement after an
/// install that did not happen, unless a newer check replaced the update.
fn keep_staged(updates: &Updates, version: &str, path: PathBuf) {
    if let Some(pending) = updates.pending.lock().as_mut() {
        if pending.update.version == version && pending.placement.is_none() {
            pending.placement = Some(Placement::Staged(path));
        }
    }
}

/// The quit: a staged installer runs now, silently, and does not bring the
/// app back, since the person closed it. Only Windows stages an installer the
/// quit can run; a Linux package would prompt for a password, and waits for
/// [`restart_to_update`].
pub fn on_exit(app: &AppHandle) {
    if !cfg!(windows) {
        return;
    }
    let Some(updates) = app.try_state::<Arc<Updates>>() else { return };
    let staged = {
        let mut pending = updates.pending.lock();
        pending.as_mut().and_then(|p| match p.placement.take() {
            Some(Placement::Staged(path)) => Some((p.update.clone(), path)),
            _ => None,
        })
    };
    let Some((update, path)) = staged else { return };
    match std::fs::read(&path) {
        Ok(bytes) => {
            // `install` starts the installer and ends this process itself.
            if let Err(error) = update.restart_after_install(false).install(bytes) {
                tracing::warn!(%error, "the staged update could not be installed at quit");
            }
        }
        Err(error) => tracing::warn!(%error, "the staged update could not be read back at quit"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_bundle_has_its_own_log_name() {
        let all = [
            BundleType::App,
            BundleType::Dmg,
            BundleType::AppImage,
            BundleType::Deb,
            BundleType::Rpm,
            BundleType::Nsis,
            BundleType::Msi,
        ];
        let mut names: Vec<&str> = all.iter().map(|b| bundle_name(Some(b))).collect();
        names.push(bundle_name(None));
        assert_eq!(names, ["app", "dmg", "appimage", "deb", "rpm", "nsis", "msi", "none"]);
    }

    #[test]
    fn a_development_build_or_a_bundleless_linux_build_refuses_to_update_in_place() {
        assert!(self_update_refusal(true, None).is_some());
        assert!(self_update_refusal(true, Some(&BundleType::App)).is_some());
        assert!(self_update_refusal(false, Some(&BundleType::App)).is_none());
        assert!(self_update_refusal(false, Some(&BundleType::AppImage)).is_none());
        assert_eq!(self_update_refusal(false, None).is_some(), cfg!(target_os = "linux"));
    }

    #[test]
    fn what_the_window_reads_is_camel_case() {
        let check = UpdateCheck {
            current_version: "0.1.0".into(),
            version: Some("0.2.0".into()),
            ready: Some(UpdateReady { version: "0.2.0".into(), installed: true }),
        };
        let json = serde_json::to_value(&check).unwrap();
        assert_eq!(json["currentVersion"], "0.1.0");
        assert_eq!(json["ready"]["installed"], true);
        let progress = serde_json::to_value(Progress { downloaded: 5, total: None }).unwrap();
        assert_eq!(progress, serde_json::json!({ "downloaded": 5, "total": null }));
    }
}
