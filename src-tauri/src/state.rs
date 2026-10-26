//! Everything the window asks for, without Tauri, so it can be tested.
use crate::processes;
use onelibrary_sync::{Fingerprint, Plan, Report, Step};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// An error as the window shows it.
#[derive(Debug, Serialize)]
pub struct Failure {
    pub message: String,
}

impl Failure {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl From<onelibrary_sync::Error> for Failure {
    fn from(error: onelibrary_sync::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

pub type Outcome<T> = Result<T, Failure>;

/// What stands in the way of a sync, apart from the stick itself.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub rekordbox_running: bool,
}

/// A connected volume, as the list shows it.
#[allow(clippy::struct_excessive_bools, reason = "independent facts the window shows side by side")]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub path: String,
    pub name: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub removable: bool,
    pub has_one_library: bool,
    pub has_device_library: bool,
    /// An earlier write was interrupted and has to be finished first.
    pub interrupted: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub running: bool,
    /// reading, writing, verifying; then complete or failed.
    pub phase: String,
    pub path: Option<String>,
    pub error: Option<String>,
    pub report: Option<Report>,
}

pub struct AppState {
    job: Mutex<SyncProgress>,
    /// Whether rekordbox is running; replaced in tests.
    rekordbox: fn() -> bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self { job: Mutex::new(SyncProgress::default()), rekordbox: processes::rekordbox_running }
    }
}

impl AppState {
    pub fn status(&self) -> Status {
        Status { rekordbox_running: (self.rekordbox)() }
    }

    /// The connected volumes, with what each holds. A few `stat`s each.
    pub fn devices() -> Vec<Device> {
        rbl_devices::list()
            .into_iter()
            .map(|device| Device {
                has_one_library: onelibrary_sync::layout(&device.mount_point).is_some(),
                has_device_library: onelibrary_sync::has_device_library(&device.mount_point),
                interrupted: onelibrary_sync::interrupted(&device.mount_point),
                path: device.mount_point.to_string_lossy().into_owned(),
                name: device.name,
                file_system: device.file_system,
                total_bytes: device.total_bytes,
                free_bytes: device.free_bytes,
                removable: device.removable,
            })
            .collect()
    }

    /// The volume at `path`, if it is one of those listed: nothing is read
    /// from or written to any other path.
    fn connected(path: &str) -> Outcome<PathBuf> {
        let path = PathBuf::from(path);
        if rbl_devices::list().iter().any(|device| device.mount_point == path) {
            Ok(path)
        } else {
            Err(Failure::new("The USB is no longer connected."))
        }
    }

    fn syncing(&self, path: &Path) -> bool {
        let job = self.job.lock();
        job.running && job.path.as_deref().is_some_and(|p| Path::new(p) == path)
    }

    pub fn plan(&self, path: &str) -> Outcome<Plan> {
        let path = Self::connected(path)?;
        if self.syncing(&path) {
            return Err(Failure::new("This USB is being synced."));
        }
        Ok(onelibrary_sync::plan(&path)?)
    }

    /// Finishes an interrupted write on the stick.
    pub fn recover(&self, path: &str) -> Outcome<()> {
        if (self.rekordbox)() {
            return Err(Failure::new("Quit rekordbox before changing a USB."));
        }
        Ok(onelibrary_sync::recover(&Self::connected(path)?)?)
    }

    pub fn eject(&self, path: &str) -> Outcome<()> {
        let path = Self::connected(path)?;
        if self.syncing(&path) {
            return Err(Failure::new("Wait for the sync to finish before ejecting this USB."));
        }
        rbl_devices::eject::eject(&path).map_err(|e| Failure::new(format!("The USB could not be ejected: {e}")))
    }

    pub fn sync_progress(&self) -> SyncProgress {
        self.job.lock().clone()
    }

    /// Starts syncing the stick at `path`, if it is still as `expected`
    /// describes it. The job is reserved before it starts, so two requests
    /// cannot both run.
    pub fn start_sync(self: &Arc<Self>, path: &str, expected: Fingerprint) -> Outcome<()> {
        let path = Self::connected(path)?;
        if (self.rekordbox)() {
            return Err(Failure::new("Quit rekordbox before syncing. It writes to the USBs it sees."));
        }
        {
            let mut job = self.job.lock();
            if job.running {
                return Err(Failure::new("A sync is already running."));
            }
            *job = SyncProgress {
                running: true,
                phase: Step::Reading.name().into(),
                path: Some(path.to_string_lossy().into_owned()),
                ..SyncProgress::default()
            };
        }
        let state = Arc::clone(self);
        std::thread::Builder::new()
            .name("sync".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    onelibrary_sync::sync(&path, &expected, &mut |step| step.name().clone_into(&mut state.job.lock().phase))
                }))
                .unwrap_or_else(|_| Err(onelibrary_sync::Error::Verify("The sync stopped unexpectedly.".into())));
                let mut job = state.job.lock();
                job.running = false;
                match result {
                    Ok(report) => {
                        job.phase = "complete".into();
                        job.report = Some(report);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "sync failed");
                        job.phase = "failed".into();
                        job.error = Some(error.to_string());
                    }
                }
            })
            .map_err(|e| {
                *self.job.lock() = SyncProgress::default();
                Failure::new(format!("The sync could not start: {e}"))
            })?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// A stick with a one-track `OneLibrary`, listed as the only volume.
    /// Only this test lists volumes, so setting the variable is safe.
    #[test]
    fn syncs_a_listed_stick_and_refuses_while_rekordbox_runs_or_for_another_path() {
        static REKORDBOX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let dir = tempfile::tempdir().unwrap();
        let stick = dir.path().join("DJ STICK");
        let db_dir = stick.join("PIONEER/rekordbox");
        std::fs::create_dir_all(&db_dir).unwrap();
        let mut builder = rbl_onelibrary::build::Builder::create(&db_dir.join("exportLibrary.db")).unwrap();
        builder
            .add_track(&rbl_onelibrary::build::Track {
                content_id: 1,
                title: "One".into(),
                path: "/Contents/One.mp3".into(),
                file_name: "One.mp3".into(),
                ..rbl_onelibrary::build::Track::default()
            })
            .unwrap();
        builder.finish("DJ STICK", "2026-10-25", 0).unwrap();
        std::env::set_var(rbl_devices::FAKE_VOLUMES, &stick);

        let state = Arc::new(AppState { rekordbox: || REKORDBOX.load(std::sync::atomic::Ordering::Relaxed), ..AppState::default() });
        let listed = AppState::devices();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].has_one_library && !listed[0].has_device_library);
        let path = listed[0].path.clone();
        let plan = state.plan(&path).unwrap();
        assert_eq!(plan.changes.tracks_added, 1);
        assert!(state.plan(&dir.path().to_string_lossy()).is_err(), "not a listed volume");

        REKORDBOX.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(state.start_sync(&path, plan.fingerprint.clone()).is_err());
        REKORDBOX.store(false, std::sync::atomic::Ordering::Relaxed);

        state.start_sync(&path, plan.fingerprint).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while state.sync_progress().running {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let done = state.sync_progress();
        assert_eq!(done.phase, "complete", "{:?}", done.error);
        assert!(done.report.unwrap().changes.created);
        assert!(AppState::devices()[0].has_device_library);
        assert!(!state.plan(&path).unwrap().changes.any());
        std::env::remove_var(rbl_devices::FAKE_VOLUMES);
    }
}
