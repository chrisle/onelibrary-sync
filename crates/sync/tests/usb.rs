//! Syncs on a real USB stick that rekordbox exported, run by hand rather
//! than in CI. Each test changes the stick's libraries and puts the files
//! back as they were when it ends, pass or fail.
//!
//! ```sh
//! ONELIBRARY_TEST_USB="/Volumes/USB B" cargo test -p onelibrary-sync --test usb -- --ignored
//! ```
//!
//! Use a stick whose libraries may be rewritten; quit rekordbox first.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::pedantic)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use onelibrary_sync::{device, plan, sync, Error, Setting, Table};

/// The files a test may change, all in the database folder.
const FILES: [&str; 5] = ["exportLibrary.db", "exportLibrary.db-wal", "exportLibrary.db-shm", "export.pdb", "exportExt.pdb"];

/// One test at a time: they share the stick.
static STICK: Mutex<()> = Mutex::new(());

/// The stick, with its database files saved to put back on drop.
struct Usb {
    mount: PathBuf,
    db_dir: PathBuf,
    saved: tempfile::TempDir,
    _turn: MutexGuard<'static, ()>,
}

impl Usb {
    fn new() -> Self {
        let turn = STICK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mount = PathBuf::from(std::env::var_os("ONELIBRARY_TEST_USB").expect("set ONELIBRARY_TEST_USB to the stick's mount point"));
        let layout = onelibrary_sync::layout(&mount).expect("the stick has a OneLibrary");
        assert!(!onelibrary_sync::interrupted(&mount), "the stick has an interrupted write; finish it in the app first");
        let saved = tempfile::tempdir().unwrap();
        for name in FILES {
            let from = layout.db_dir.join(name);
            if from.exists() {
                std::fs::copy(&from, saved.path().join(name)).unwrap();
            }
        }
        Self { mount, db_dir: layout.db_dir, saved, _turn: turn }
    }
    fn mount(&self) -> &Path {
        &self.mount
    }
    fn pdb_path(&self) -> PathBuf {
        self.db_dir.join("export.pdb")
    }
    fn pdb(&self) -> Vec<u8> {
        std::fs::read(self.pdb_path()).unwrap()
    }
    fn contents(&self) -> device::Contents {
        device::decode(&self.pdb()).unwrap()
    }
    /// Changes the `OneLibrary` as rekordbox would, and moves its
    /// modification time on: HFS+ and FAT keep whole seconds at best.
    fn edit(&self, sql: &str) {
        let conn = rbl_onelibrary::playlists::open_read_write(&self.db_dir.join("exportLibrary.db")).unwrap();
        conn.execute_batch(sql).unwrap();
        drop(conn);
        self.move_on("exportLibrary.db");
    }
    fn move_on(&self, name: &str) {
        let file = std::fs::File::options().write(true).open(self.db_dir.join(name)).unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
    }
    fn query(&self, sql: &str) -> i64 {
        let conn = rbl_onelibrary::playlists::open_read_write(&self.db_dir.join("exportLibrary.db")).unwrap();
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }
    fn sync(&self) -> onelibrary_sync::Report {
        let planned = plan(self.mount()).unwrap();
        sync(self.mount(), &planned.fingerprint, &mut |_| {}).unwrap()
    }
}

impl Drop for Usb {
    fn drop(&mut self) {
        for name in FILES {
            let (kept, on_stick) = (self.saved.path().join(name), self.db_dir.join(name));
            if kept.exists() {
                std::fs::copy(&kept, &on_stick).unwrap();
            } else if on_stick.exists() {
                std::fs::remove_file(&on_stick).unwrap();
            }
        }
        let journal = self.mount.join(onelibrary_sync::PUBLICATION);
        if journal.exists() {
            std::fs::remove_dir_all(&journal).or_else(|_| std::fs::remove_file(&journal)).unwrap();
        }
    }
}

/// The page type of each page of a file.
fn page_types(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks(device::PAGE_SIZE).map(|p| u32::from_le_bytes(p[8..12].try_into().unwrap())).collect()
}

/// Every page outside `tables` reads byte for byte as it did.
fn only_pages_of(tables: &[Table], before: &[u8], after: &[u8]) {
    let owned: Vec<u32> = tables.iter().map(|t| t.page_type()).collect();
    let (types_before, types_after) = (page_types(before), page_types(after));
    let changed = |types: &[u32], i: usize| types.get(i).is_some_and(|t| owned.contains(t));
    for (i, page) in after.chunks(device::PAGE_SIZE).enumerate().skip(1) {
        if changed(&types_before, i) || changed(&types_after, i) {
            continue;
        }
        let old = before.chunks(device::PAGE_SIZE).nth(i);
        assert!(old.is_none_or(|old| old == page) || page.iter().all(|&b| b == 0), "page {i} (type {}) changed", types_after[i]);
    }
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn reads_both_libraries_rekordbox_wrote() {
    let usb = Usb::new();
    let planned = plan(usb.mount()).unwrap();
    let device = planned.device_library.as_ref().expect("the stick has a Device Library");
    let one = &planned.one_library;
    let comparable = |c: &onelibrary_sync::Counts| {
        (c.tracks, c.playlists, c.folders, c.artists, c.albums, c.genres, c.labels, c.keys, c.artwork, c.my_tags)
    };
    assert_eq!(comparable(one), comparable(device), "rekordbox wrote both libraries from one collection");
    assert_eq!((planned.missing_audio, planned.missing_analysis), (0, 0));
    assert!(!planned.changes.created && !planned.changes.rebuilt);
    for table in &planned.changes.tables {
        assert!(matches!(table, Table::Sorts | Table::Categories | Table::Colors | Table::Property), "{table:?} differs");
    }
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn a_sync_rewrites_only_what_differs_and_then_nothing() {
    let usb = Usb::new();
    let before = usb.pdb();
    let ext_before = std::fs::read(usb.db_dir.join("exportExt.pdb")).unwrap();
    let planned = plan(usb.mount()).unwrap();
    let report = sync(usb.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    assert_eq!(report.changes, planned.changes);
    only_pages_of(&planned.changes.tables, &before, &usb.pdb());
    if !planned.changes.my_tags {
        assert_eq!(std::fs::read(usb.db_dir.join("exportExt.pdb")).unwrap(), ext_before);
    }
    assert!(!usb.mount().join(onelibrary_sync::PUBLICATION).exists());

    let again = plan(usb.mount()).unwrap();
    assert!(!again.changes.any(), "{:?}", again.changes);
    let written = (usb.pdb(), std::fs::metadata(usb.pdb_path()).unwrap().modified().unwrap());
    assert!(!usb.sync().changes.any());
    assert_eq!((usb.pdb(), std::fs::metadata(usb.pdb_path()).unwrap().modified().unwrap()), written, "a stick in sync is not written");
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn a_renamed_playlist_rewrites_only_the_playlist_tree() {
    let usb = Usb::new();
    usb.sync();
    let before = usb.pdb();
    let id = usb.query("SELECT MIN(playlist_id) FROM playlist WHERE attribute = 0");
    usb.edit(&format!("UPDATE playlist SET name = 'Renamed on the stick' WHERE playlist_id = {id}"));
    let planned = plan(usb.mount()).unwrap();
    assert_eq!(planned.changes.tables, vec![Table::Playlists]);
    assert_eq!((planned.changes.playlists_added, planned.changes.playlists_updated, planned.changes.playlists_removed), (0, 1, 0));
    sync(usb.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    only_pages_of(&[Table::Playlists], &before, &usb.pdb());
    assert_eq!(usb.contents().tree[&u32::try_from(id).unwrap()].name, "Renamed on the stick");
    assert!(!plan(usb.mount()).unwrap().changes.any());
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn the_column_beside_the_track_name_follows_the_one_library() {
    let usb = Usb::new();
    usb.sync();
    usb.edit("UPDATE sort SET isSelectedAsSubColumn = (menuItem_id = 5)");
    let planned = plan(usb.mount()).unwrap();
    assert_eq!(planned.changes.tables, vec![Table::Sorts]);
    let now = usb.contents().settings()[0].1.clone();
    assert_eq!(planned.changes.settings, vec![Setting { name: "Column beside the track name".into(), now, becomes: "BPM".into() }]);
    sync(usb.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    assert_eq!(usb.contents().settings()[0].1, "BPM");
    assert!(!plan(usb.mount()).unwrap().changes.any());
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn a_track_taken_out_of_the_one_library_leaves_the_device_library() {
    let usb = Usb::new();
    usb.sync();
    let id = usb.query("SELECT MAX(content_id) FROM content");
    let tracks = usb.contents().tracks.len();
    usb.edit(&format!(
        "DELETE FROM playlist_content WHERE content_id = {id};
         DELETE FROM content WHERE content_id = {id};
         UPDATE property SET numberOfContents = numberOfContents - 1;"
    ));
    let planned = plan(usb.mount()).unwrap();
    assert_eq!((planned.changes.tracks_added, planned.changes.tracks_updated, planned.changes.tracks_removed), (0, 0, 1));
    assert!(planned.changes.tables.contains(&Table::Tracks));
    sync(usb.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    let contents = usb.contents();
    assert_eq!(contents.tracks.len(), tracks - 1);
    let id = u32::try_from(id).unwrap();
    assert!(!contents.tracks.contains_key(&id));
    assert!(contents.entries.values().all(|list| !list.contains(&id)));
    assert!(!plan(usb.mount()).unwrap().changes.any());
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn a_device_library_made_from_nothing_reads_as_the_one_rekordbox_wrote() {
    let usb = Usb::new();
    let rekordbox = usb.contents();
    std::fs::remove_file(usb.pdb_path()).unwrap();
    std::fs::remove_file(usb.db_dir.join("exportExt.pdb")).unwrap();
    let planned = plan(usb.mount()).unwrap();
    assert!(planned.changes.created && planned.changes.my_tags);
    assert!(planned.device_library.is_none());
    sync(usb.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    assert!(!plan(usb.mount()).unwrap().changes.any());

    let ours = usb.contents();
    assert_eq!(ours.tree, rekordbox.tree);
    assert_eq!(ours.entries, rekordbox.entries);
    assert_eq!(ours.artists, rekordbox.artists);
    assert_eq!(ours.albums, rekordbox.albums);
    assert_eq!(ours.genres, rekordbox.genres);
    assert_eq!(ours.labels, rekordbox.labels);
    assert_eq!(ours.keys, rekordbox.keys);
    assert_eq!(ours.colors, rekordbox.colors);
    assert_eq!(ours.artwork, rekordbox.artwork);
    assert_eq!(ours.categories, rekordbox.categories);
    assert_eq!(ours.tracks.keys().collect::<Vec<_>>(), rekordbox.tracks.keys().collect::<Vec<_>>());
    for (id, track) in &ours.tracks {
        let theirs = &rekordbox.tracks[id];
        fn shown(t: &device::TrackFacts) -> (&str, u32, u32, u32, u32, u32, u16, &str, &str) {
            (&t.title, t.artist_id, t.album_id, t.genre_id, t.key_id, t.tempo_x100, t.duration_sec, &t.file_path, &t.analyze_path)
        }
        assert_eq!(shown(track), shown(theirs), "track {id}");
    }
}

#[test]
#[ignore = "needs a USB stick: ONELIBRARY_TEST_USB"]
fn a_device_library_written_after_the_plan_is_left_alone() {
    let usb = Usb::new();
    let modified = std::fs::metadata(usb.pdb_path()).unwrap().modified().unwrap();
    let planned = plan(usb.mount()).unwrap();
    // A player saves a playlist within the same second: same length, same
    // time on a file system that keeps whole seconds, other bytes.
    let mut bytes = usb.pdb();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(usb.pdb_path(), &bytes).unwrap();
    std::fs::File::options().write(true).open(usb.pdb_path()).unwrap().set_modified(modified).unwrap();
    assert!(matches!(sync(usb.mount(), &planned.fingerprint, &mut |_| {}), Err(Error::Changed)));
    assert_eq!(usb.pdb(), bytes);
    assert!(!usb.mount().join(onelibrary_sync::PUBLICATION).exists());
}
