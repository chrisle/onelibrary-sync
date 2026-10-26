//! Everything the window asks for, without Tauri, so it can be tested.
use crate::processes;
use onelibrary_sync::{Fingerprint, Plan, Report, Step};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    collections::BTreeMap,
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
    /// Each USB's latest sync, running or finished, by mount point. USBs
    /// sync side by side; one USB syncs once at a time.
    jobs: Mutex<BTreeMap<PathBuf, SyncProgress>>,
    /// Whether rekordbox is running; replaced in tests.
    rekordbox: fn() -> bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self { jobs: Mutex::new(BTreeMap::new()), rekordbox: processes::rekordbox_running }
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
        self.jobs.lock().get(path).is_some_and(|job| job.running)
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

    /// Every USB's latest sync, in mount point order.
    pub fn sync_progress(&self) -> Vec<SyncProgress> {
        self.jobs.lock().values().cloned().collect()
    }

    /// Starts syncing the stick at `path`, if it is still as `expected`
    /// describes it. Other sticks may be syncing meanwhile. The stick's job
    /// is reserved before it starts, so two requests for it cannot both run.
    pub fn start_sync(self: &Arc<Self>, path: &str, expected: Fingerprint) -> Outcome<()> {
        let path = Self::connected(path)?;
        if (self.rekordbox)() {
            return Err(Failure::new("Quit rekordbox before syncing. It writes to the USBs it sees."));
        }
        let previous = {
            let mut jobs = self.jobs.lock();
            if jobs.get(&path).is_some_and(|job| job.running) {
                return Err(Failure::new("This USB is already being synced."));
            }
            jobs.insert(
                path.clone(),
                SyncProgress {
                    running: true,
                    phase: Step::Reading.name().into(),
                    path: Some(path.to_string_lossy().into_owned()),
                    ..SyncProgress::default()
                },
            )
        };
        let state = Arc::clone(self);
        let mount = path.clone();
        std::thread::Builder::new()
            .name("sync".into())
            .spawn(move || {
                let phase = |step: Step| {
                    if let Some(job) = state.jobs.lock().get_mut(&mount) {
                        step.name().clone_into(&mut job.phase);
                    }
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    onelibrary_sync::sync(&mount, &expected, &mut |step| phase(step))
                }))
                .unwrap_or_else(|_| Err(onelibrary_sync::Error::Verify("The sync stopped unexpectedly.".into())));
                let mut jobs = state.jobs.lock();
                let Some(job) = jobs.get_mut(&mount) else { return };
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
                let mut jobs = self.jobs.lock();
                match previous {
                    Some(previous) => jobs.insert(path, previous),
                    None => jobs.remove(&path),
                };
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

    /// A stick named `name` in `dir` with a one-track `OneLibrary`.
    fn stick(dir: &Path, name: &str) -> PathBuf {
        let stick = dir.join(name);
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
        builder.finish(name, "2026-10-25", 0).unwrap();
        stick
    }

    /// Two sticks listed as the only volumes, synced side by side. Only this
    /// test lists volumes, so setting the variable is safe.
    #[test]
    fn syncs_listed_sticks_side_by_side_and_refuses_while_rekordbox_runs_or_for_another_path() {
        static REKORDBOX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let dir = tempfile::tempdir().unwrap();
        let sticks = [stick(dir.path(), "DJ STICK"), stick(dir.path(), "TOUR USB")];
        std::env::set_var(rbl_devices::FAKE_VOLUMES, std::env::join_paths(&sticks).unwrap());

        let state = Arc::new(AppState { rekordbox: || REKORDBOX.load(std::sync::atomic::Ordering::Relaxed), ..AppState::default() });
        let listed = AppState::devices();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|device| device.has_one_library && !device.has_device_library));
        let paths: Vec<String> = listed.iter().map(|device| device.path.clone()).collect();
        let plans: Vec<Plan> = paths.iter().map(|path| state.plan(path).unwrap()).collect();
        assert_eq!(plans[0].changes.tracks_added, 1);
        assert!(state.plan(&dir.path().to_string_lossy()).is_err(), "not a listed volume");

        REKORDBOX.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(state.start_sync(&paths[0], plans[0].fingerprint.clone()).is_err());
        REKORDBOX.store(false, std::sync::atomic::Ordering::Relaxed);

        // A stick already syncing is not started twice.
        let running = SyncProgress { running: true, ..SyncProgress::default() };
        state.jobs.lock().insert(PathBuf::from(&paths[0]), running);
        assert!(state.start_sync(&paths[0], plans[0].fingerprint.clone()).is_err());
        state.jobs.lock().clear();

        for (path, plan) in paths.iter().zip(&plans) {
            state.start_sync(path, plan.fingerprint.clone()).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while state.sync_progress().iter().any(|job| job.running) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let done = state.sync_progress();
        assert_eq!(done.len(), 2);
        for job in done {
            assert_eq!(job.phase, "complete", "{:?}", job.error);
            assert!(job.report.unwrap().changes.created);
        }
        assert!(AppState::devices().iter().all(|device| device.has_device_library));
        assert!(paths.iter().all(|path| !state.plan(path).unwrap().changes.any()));
        std::env::remove_var(rbl_devices::FAKE_VOLUMES);
    }
}
