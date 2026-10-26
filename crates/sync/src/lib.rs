//! One-way sync on a USB stick, from its `OneLibrary` (`exportLibrary.db`)
//! to its Device Library (`export.pdb` and `exportExt.pdb`), so players that
//! read only the Device Library see what the `OneLibrary` holds.
//!
//! The audio, analysis and artwork files are already on the stick, and both
//! libraries name the same files, so only the Device Library's databases are
//! written. Of those, only what differs: a table of `export.pdb` that would
//! read the same is left byte for byte as it is ([`device`]), and
//! `exportExt.pdb` is written only when its My Tags change ([`ext`]). A
//! stick already in step is not written at all.
//!
//! Writes go through rbxport's publication journal and write lock on the
//! stick (`.rbxport-publication`, `.rbxport-write.lock`), so this app and
//! rbxport never write a stick at the same time, and either can finish the
//! other's interrupted write.

pub mod device;
pub mod ext;
pub mod source;

use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

pub use device::Table;

/// rbxport's publication journal on a stick.
pub const PUBLICATION: &str = ".rbxport-publication";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("This USB has no OneLibrary to sync from.")]
    NoOneLibrary,
    #[error("An earlier write to this USB was interrupted. Finish it before syncing.")]
    Interrupted,
    #[error("{0}")]
    Recovery(String),
    #[error("The USB changed since it was checked. Nothing was changed; check it again and sync.")]
    Changed,
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    TooLong(String),
    #[error("The USB's library could not be read: {0}")]
    Unreadable(String),
    #[error("{0}")]
    Verify(String),
    #[error("The USB's OneLibrary could not be read: {0}")]
    OneLibrary(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    pub(crate) fn one(error: impl std::fmt::Display) -> Self {
        Self::OneLibrary(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Where a stick keeps its libraries: `PIONEER`, or `.PIONEER`, which
/// rekordbox 7 can write on HFS+.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub mount: PathBuf,
    pub root_name: &'static str,
    pub db_dir: PathBuf,
}

impl Layout {
    pub fn one_library(&self) -> PathBuf {
        self.db_dir.join("exportLibrary.db")
    }

    pub fn device_library(&self) -> PathBuf {
        self.db_dir.join("export.pdb")
    }

    pub fn ext(&self) -> PathBuf {
        self.db_dir.join("exportExt.pdb")
    }

    /// A database's path relative to the stick, as the journal names it.
    fn relative(&self, file: &str) -> PathBuf {
        Path::new(self.root_name).join("rekordbox").join(file)
    }
}

/// The stick's `OneLibrary`, or `None` when it has none. A few `stat`s:
/// cheap enough for every volume in a list.
pub fn layout(mount: &Path) -> Option<Layout> {
    ["PIONEER", ".PIONEER"].into_iter().find_map(|root_name| {
        let db_dir = mount.join(root_name).join("rekordbox");
        db_dir.join("exportLibrary.db").is_file().then(|| Layout { mount: mount.to_owned(), root_name, db_dir })
    })
}

/// Whether the stick has a Device Library at all.
pub fn has_device_library(mount: &Path) -> bool {
    ["PIONEER", ".PIONEER"].iter().any(|root| mount.join(root).join("rekordbox/export.pdb").is_file())
}

/// The files a sync reads, by size and modification time: what tells
/// whether the stick changed between the plan and the sync.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    pub files: Vec<Option<(u64, u64)>>,
}

fn stamp(path: &Path) -> Result<Option<(u64, u64)>> {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            Ok(Some((meta.len(), modified.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)))))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn fingerprint(layout: &Layout) -> Result<Fingerprint> {
    let one = layout.one_library();
    Ok(Fingerprint {
        files: vec![
            stamp(&one)?,
            stamp(&one.with_file_name("exportLibrary.db-wal"))?,
            stamp(&layout.device_library())?,
            stamp(&layout.ext())?,
        ],
    })
}

/// How much a library holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub tracks: usize,
    pub playlists: usize,
    pub folders: usize,
    pub artists: usize,
    pub albums: usize,
    pub genres: usize,
    pub labels: usize,
    pub keys: usize,
    pub artwork: usize,
    pub my_tags: usize,
    pub histories: usize,
}

/// What a sync changes in the Device Library.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    pub tracks_added: usize,
    pub tracks_updated: usize,
    pub tracks_removed: usize,
    pub playlists_added: usize,
    pub playlists_updated: usize,
    pub playlists_removed: usize,
    /// The tables of `export.pdb` that are written; the rest stay as they are.
    pub tables: Vec<Table>,
    /// `export.pdb` is created, or written whole because it could not be
    /// edited in place.
    pub created: bool,
    pub rebuilt: bool,
    /// `exportExt.pdb`, the My Tags, is written.
    pub my_tags: bool,
}

impl Changes {
    pub fn any(&self) -> bool {
        !self.tables.is_empty() || self.my_tags
    }
}

/// What a stick holds and what syncing it would do.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub fingerprint: Fingerprint,
    pub device_name: String,
    pub one_library: Counts,
    /// `None` when the stick has no Device Library yet.
    pub device_library: Option<Counts>,
    pub changes: Changes,
    /// Tracks whose audio file is not on the stick: a player lists them but
    /// cannot load them, from either library.
    pub missing_audio: usize,
    /// Tracks with no analysis file: a player shows no waveform or beat grid
    /// for them.
    pub missing_analysis: usize,
}

/// Where a sync has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Reading,
    Writing,
    Verifying,
}

impl Step {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Writing => "writing",
            Self::Verifying => "verifying",
        }
    }
}

/// What a sync did.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub changes: Changes,
    pub elapsed_ms: u64,
}

/// Whether an interrupted write is waiting on the stick.
pub fn interrupted(mount: &Path) -> bool {
    mount.join(PUBLICATION).exists()
}

/// Finishes an interrupted write: one of this app's, or rbxport's.
///
/// An rbxport export that still has to verify what it published, or one
/// whose files were changed by another program since, is rbxport's to
/// settle (`rbl_export::recover`); it is left for rbxport.
pub fn recover(mount: &Path) -> Result<()> {
    if !interrupted(mount) {
        return Ok(());
    }
    if rbl_core::durable::Publication::validation_pending(mount, PUBLICATION) {
        return Err(Error::Recovery(
            "RBXport has an export on this USB that it hasn't finished. Open RBXport with the USB connected so it can finish it, then sync.".into(),
        ));
    }
    match rbl_core::durable::Publication::recover(mount, PUBLICATION) {
        Err(error) if rbl_core::durable::is_device_changed(&error) => Err(Error::Recovery(
            "An interrupted write on this USB can't be finished because another program changed the USB since. Open RBXport with the USB connected so it can settle it, then sync.".into(),
        )),
        other => Ok(other?),
    }
}

/// Whether a library path names a file on the stick. macOS lists FAT and
/// exFAT names decomposed, and a library may name them either way.
fn on_stick(mount: &Path, relative: &str) -> bool {
    use unicode_normalization::UnicodeNormalization as _;
    let relative = relative.trim_start_matches(['/', '\\']);
    if relative.is_empty() {
        return false;
    }
    let nfc: String = relative.nfc().collect();
    let nfd: String = relative.nfd().collect();
    [relative, nfc.as_str(), nfd.as_str()].iter().any(|r| mount.join(r).is_file())
}

fn one_counts(one: &source::OneLibrary) -> Counts {
    Counts {
        tracks: one.tracks.len(),
        playlists: one.playlists.iter().filter(|p| !p.folder).count(),
        folders: one.playlists.iter().filter(|p| p.folder).count(),
        artists: one.artists.len(),
        albums: one.albums.len(),
        genres: one.genres.len(),
        labels: one.labels.len(),
        keys: one.keys.len(),
        artwork: one.images.len(),
        my_tags: one.my_tags.iter().filter(|t| t.attribute != 1).count(),
        histories: one.histories,
    }
}

fn device_counts(contents: &device::Contents, ext: Option<&ext::Contents>) -> Counts {
    Counts {
        tracks: contents.tracks.len(),
        playlists: contents.tree.values().filter(|n| !n.folder).count(),
        folders: contents.tree.values().filter(|n| n.folder).count(),
        artists: contents.artists.len(),
        albums: contents.albums.len(),
        genres: contents.genres.len(),
        labels: contents.labels.len(),
        keys: contents.keys.len(),
        artwork: contents.artwork.len(),
        my_tags: ext.map_or(0, |e| e.tags.iter().filter(|t| !t.3).count()),
        histories: contents.history.len(),
    }
}

fn changes(update: Option<&device::Update>, my_tags: bool) -> Changes {
    let mut out = Changes { my_tags, ..Changes::default() };
    let Some(update) = update else { return out };
    out.tables.clone_from(&update.tables);
    out.created = update.before.is_none();
    out.rebuilt = update.rebuilt && update.before.is_some();
    let empty = device::Contents::default();
    let (before, after) = (update.before.as_ref().unwrap_or(&empty), &update.after);
    for (id, track) in &after.tracks {
        match before.tracks.get(id) {
            None => out.tracks_added += 1,
            Some(was) if was != track => out.tracks_updated += 1,
            Some(_) => {}
        }
    }
    out.tracks_removed = before.tracks.keys().filter(|id| !after.tracks.contains_key(id)).count();
    // Folders count with the playlists: a folder made on a player is as
    // much the stick's as a playlist.
    let lists = |c: &device::Contents| -> std::collections::BTreeMap<u32, (device::Node, Vec<u32>)> {
        c.tree
            .iter()
            .map(|(id, n)| (*id, (n.clone(), c.entries.get(id).cloned().unwrap_or_default())))
            .collect()
    };
    let (was, now) = (lists(before), lists(after));
    for (id, list) in &now {
        match was.get(id) {
            None => out.playlists_added += 1,
            Some(old) if old != list => out.playlists_updated += 1,
            Some(_) => {}
        }
    }
    out.playlists_removed = was.keys().filter(|id| !now.contains_key(id)).count();
    out
}

/// The artwork paths for the Device Library.
///
/// rekordbox writes each image twice, `a<id>.jpg` and `b<id>.jpg` with
/// their `_m` sizes, and names the `a` files in `export.pdb` and the `b`
/// files in `exportLibrary.db` [OBS: a rekordbox 7 stick with both
/// libraries, `/.PIONEER/Artwork/00001/a1.jpg` against `b1.jpg`]. Where the
/// `OneLibrary` names a `b` file whose `a` file is on the stick, the Device
/// Library names the `a` file, as rekordbox's does.
fn device_artwork(mount: &Path, images: std::collections::BTreeMap<u32, String>) -> std::collections::BTreeMap<u32, String> {
    images
        .into_iter()
        .map(|(id, path)| {
            let a = match path.rsplit_once('/') {
                Some((dir, name)) if name.starts_with('b') && dir.contains("/Artwork/") => format!("{dir}/a{}", &name[1..]),
                _ => return (id, path),
            };
            if on_stick(mount, &a) { (id, a) } else { (id, path) }
        })
        .collect()
}

/// The stick's `OneLibrary`, with its artwork named as the Device Library
/// names it.
pub fn read_one_library(layout: &Layout) -> Result<source::OneLibrary> {
    let mut one = source::read(&layout.one_library())?;
    one.images = device_artwork(&layout.mount, std::mem::take(&mut one.images));
    Ok(one)
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Everything a plan or a sync works out from the stick as it is.
struct Worked {
    fingerprint: Fingerprint,
    one: source::OneLibrary,
    existing: Option<Vec<u8>>,
    existing_ext: Option<Vec<u8>>,
    update: Option<device::Update>,
    ext: Option<Vec<u8>>,
}

fn work_out(layout: &Layout) -> Result<Worked> {
    let fingerprint = fingerprint(layout)?;
    let one = read_one_library(layout)?;
    let existing = read_optional(&layout.device_library())?;
    let existing_ext = read_optional(&layout.ext())?;
    let update = device::update(&one, existing.as_deref(), &rbl_core::time::local_date())?;
    let tracks: std::collections::BTreeSet<u32> = one.tracks.iter().map(|t| t.id).collect();
    let tagged: Vec<(u32, &[u64])> =
        one.tagged.iter().filter(|(id, _)| tracks.contains(id)).map(|(id, tags)| (*id, tags.as_slice())).collect();
    let ext = ext::update(&one.my_tags, &tagged, one.my_tag_master_db_id, existing_ext.as_deref());
    Ok(Worked { fingerprint, one, existing, existing_ext, update, ext })
}

/// Reads the stick at `mount` and works out what a sync would change,
/// without writing anything.
pub fn plan(mount: &Path) -> Result<Plan> {
    let layout = layout(mount).ok_or(Error::NoOneLibrary)?;
    if interrupted(mount) {
        return Err(Error::Interrupted);
    }
    // Shared with readers, exclusive of rbxport's writes, which hold the
    // lock while they publish.
    let _lease = rbl_core::durable::read_lease(mount)?;
    let worked = work_out(&layout)?;
    let before = match &worked.update {
        Some(update) => update.before.clone(),
        None => worked.existing.as_deref().map(device::decode).transpose()?,
    };
    let ext_now = worked.existing_ext.as_deref().and_then(ext::decode);
    Ok(Plan {
        fingerprint: worked.fingerprint.clone(),
        device_name: worked.one.settings.device_name.clone(),
        one_library: one_counts(&worked.one),
        device_library: before.as_ref().map(|b| device_counts(b, ext_now.as_ref())),
        changes: changes(worked.update.as_ref(), worked.ext.is_some()),
        missing_audio: worked.one.tracks.iter().filter(|t| !on_stick(mount, &t.path)).count(),
        missing_analysis: worked.one.tracks.iter().filter(|t| !on_stick(mount, &t.analysis_path)).count(),
    })
}

/// Brings the Device Library in line with the `OneLibrary`, if the stick is
/// still as `expected` describes it: the fingerprint of the [`plan`] the
/// person agreed to.
pub fn sync(mount: &Path, expected: &Fingerprint, step: &mut dyn FnMut(Step)) -> Result<Report> {
    let started = Instant::now();
    let layout = layout(mount).ok_or(Error::NoOneLibrary)?;
    recover(mount)?;
    step(Step::Reading);
    // Held to the end: rbxport neither exports to nor edits the stick
    // meanwhile.
    let publication = rbl_core::durable::Publication::new(mount, PUBLICATION)?;
    let worked = work_out(&layout)?;
    if worked.fingerprint != *expected {
        return Err(Error::Changed);
    }
    let changes = changes(worked.update.as_ref(), worked.ext.is_some());
    let elapsed = |started: Instant| u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if !changes.any() {
        return Ok(Report { changes, elapsed_ms: elapsed(started) });
    }

    step(Step::Writing);
    let staged = publication.stage().join(layout.root_name).join("rekordbox");
    rbl_core::durable::create_dir_all(&staged)?;
    let mut files = Vec::new();
    for (name, bytes) in [("export.pdb", worked.update.as_ref().map(|u| &u.bytes)), ("exportExt.pdb", worked.ext.as_ref())] {
        if let Some(bytes) = bytes {
            rbl_core::durable::write(&staged.join(name), bytes)?;
            files.push(layout.relative(name));
        }
    }
    // A player, rekordbox or another program may have written the stick
    // while this staged; their write wins and nothing is published.
    if fingerprint(&layout)? != worked.fingerprint {
        return Err(Error::Changed);
    }
    publication.commit(&files)?;
    drop(publication);

    step(Step::Verifying);
    for (path, bytes) in [(layout.device_library(), worked.update.as_ref().map(|u| &u.bytes)), (layout.ext(), worked.ext.as_ref())] {
        if let Some(bytes) = bytes {
            if std::fs::read(&path)? != *bytes {
                return Err(Error::Verify(format!(
                    "{} did not read back as written. Sync again; if this repeats, the USB may be failing.",
                    path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                )));
            }
        }
    }
    tracing::info!(mount = %mount.display(), ?changes, "synced the Device Library");
    Ok(Report { changes, elapsed_ms: elapsed(started) })
}
