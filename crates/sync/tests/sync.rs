//! A stick with a `OneLibrary`, synced to its Device Library and synced
//! again after the `OneLibrary` changes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::pedantic)]

use std::path::{Path, PathBuf};

use onelibrary_sync::{device, plan, sync, Error, Records, Setting, Table};
use rbl_onelibrary::build::{Builder, LookupTable, Track};

struct Stick {
    dir: tempfile::TempDir,
}

impl Stick {
    fn mount(&self) -> &Path {
        self.dir.path()
    }
    fn db_dir(&self) -> PathBuf {
        self.mount().join("PIONEER/rekordbox")
    }
    fn one_library(&self) -> PathBuf {
        self.db_dir().join("exportLibrary.db")
    }
    fn pdb(&self) -> Vec<u8> {
        std::fs::read(self.db_dir().join("export.pdb")).unwrap()
    }
    fn contents(&self) -> device::Contents {
        device::decode(&self.pdb()).unwrap()
    }
    /// Runs SQL against the `OneLibrary`, as rekordbox or a player would
    /// change it, and moves its modification time on so the change shows.
    fn edit(&self, sql: &str) {
        let conn = rbl_onelibrary::playlists::open_read_write(&self.one_library()).unwrap();
        conn.execute_batch(sql).unwrap();
        drop(conn);
        let file = std::fs::File::options().write(true).open(self.one_library()).unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
    }
    fn sync(&self) -> onelibrary_sync::Report {
        let planned = plan(self.mount()).unwrap();
        sync(self.mount(), &planned.fingerprint, &mut |_| {}).unwrap()
    }
}

/// `tracks` tracks, each by its own artist on one album, in a folder holding
/// two playlists, with an image and two My Tags. The first two tracks have
/// their audio on the stick, the first its analysis too.
fn stick(tracks: i64) -> Stick {
    let dir = tempfile::tempdir().unwrap();
    let db_dir = dir.path().join("PIONEER/rekordbox");
    std::fs::create_dir_all(&db_dir).unwrap();
    let mut builder = Builder::create(&db_dir.join("exportLibrary.db")).unwrap();
    builder.add_image(1, "/PIONEER/Artwork/00001/a1.jpg").unwrap();
    let genre = builder.intern(LookupTable::Genre, "Techno").unwrap();
    let key = builder.intern(LookupTable::Key, "8A").unwrap();
    let album = builder.intern(LookupTable::Album, "Night Drive EP").unwrap();
    let label = builder.intern(LookupTable::Label, "Drumcode").unwrap();
    for i in 1..=tracks {
        let artist = builder.intern(LookupTable::Artist, &format!("Artist {i}")).unwrap();
        builder
            .add_track(&Track {
                content_id: i,
                title: format!("Track {i} – Ünïcode"),
                artist_id: Some(artist),
                album_id: Some(album),
                genre_id: Some(genre),
                label_id: Some(label),
                key_id: Some(key),
                color_id: Some(i % 9),
                bpm_x100: 12_800 + i,
                length: 300,
                track_no: i,
                path: format!("/Contents/Artist {i}/Night Drive EP/Track {i}.mp3"),
                file_name: format!("Track {i}.mp3"),
                file_size: 9_000_000,
                analysis_path: format!("/PIONEER/USBANLZ/P001/{i:08X}/ANLZ0000.DAT"),
                rating: i % 6,
                comment: "warm-up".into(),
                date_added: "2026-10-01".into(),
                image_id: (i == 1).then_some(1),
                year: 2026,
                bitrate: 320,
                sample_rate: 44_100,
                ..Track::default()
            })
            .unwrap();
    }
    builder.add_playlist_node(1, "Gigs", 0, 0, true).unwrap();
    builder.add_playlist_node(2, "Friday", 1, 0, false).unwrap();
    builder.add_playlist_node(3, "Saturday", 1, 1, false).unwrap();
    for i in 1..=tracks {
        builder.add_to_playlist(2, i, i).unwrap();
    }
    builder.add_to_playlist(3, 1, 1).unwrap();
    builder.add_my_tag(10, 1, "Mood", 1, 0).unwrap();
    builder.add_my_tag(11, 1, "Dark", 0, 10).unwrap();
    builder.tag_track(11, 1).unwrap();
    builder.finish("DJ STICK", "2026-10-01", 77).unwrap();
    for (i, analysis) in [(1, true), (2, false)] {
        let audio = dir.path().join(format!("Contents/Artist {i}/Night Drive EP/Track {i}.mp3"));
        std::fs::create_dir_all(audio.parent().unwrap()).unwrap();
        std::fs::write(&audio, b"audio").unwrap();
        if analysis {
            let anlz = dir.path().join(format!("PIONEER/USBANLZ/P001/{i:08X}/ANLZ0000.DAT"));
            std::fs::create_dir_all(anlz.parent().unwrap()).unwrap();
            std::fs::write(anlz, b"PMAI").unwrap();
        }
    }
    Stick { dir }
}

#[test]
fn a_stick_with_only_a_one_library_gets_a_device_library_holding_the_same() {
    let stick = stick(3);
    let planned = plan(stick.mount()).unwrap();
    assert!(planned.device_library.is_none());
    assert!(planned.changes.created && planned.changes.my_tags);
    assert_eq!(planned.changes.tracks_added, 3);
    assert_eq!(planned.changes.playlists_added, 3, "two playlists and their folder");
    assert_eq!((planned.one_library.tracks, planned.one_library.playlists, planned.one_library.folders), (3, 2, 1));
    assert_eq!((planned.missing_audio, planned.missing_analysis), (1, 2));
    assert_eq!(planned.device_name, "DJ STICK");

    let mut steps = Vec::new();
    let report = sync(stick.mount(), &planned.fingerprint, &mut |step| steps.push(step.name())).unwrap();
    assert_eq!(steps, ["reading", "writing", "verifying"]);
    assert!(report.changes.created);

    let contents = stick.contents();
    assert_eq!(contents.tracks.len(), 3);
    let first = &contents.tracks[&1];
    assert_eq!(first.title, "Track 1 – Ünïcode");
    assert_eq!(contents.artists[&first.artist_id], "Artist 1");
    assert_eq!(contents.albums[&first.album_id].1, "Night Drive EP");
    assert_eq!(contents.genres[&first.genre_id], "Techno");
    assert_eq!(contents.keys[&first.key_id], "8A");
    assert_eq!(contents.labels[&first.label_id], "Drumcode");
    assert_eq!(contents.artwork[&first.artwork_id], "/PIONEER/Artwork/00001/a1.jpg");
    assert_eq!(first.file_path, "/Contents/Artist 1/Night Drive EP/Track 1.mp3");
    assert_eq!(first.analyze_path, "/PIONEER/USBANLZ/P001/00000001/ANLZ0000.DAT");
    assert_eq!((first.rating, first.tempo_x100, first.color_id), (1, 12_801, 1));
    assert_eq!(contents.tree[&1].name, "Gigs");
    assert!(contents.tree[&1].folder);
    assert_eq!(contents.tree[&2].parent, 1);
    assert_eq!(contents.entries[&2], vec![1, 2, 3]);
    assert_eq!(contents.entries[&3], vec![1]);
    assert_eq!(contents.property, Some(("DJ STICK".to_owned(), 3, 0)));
    let ext = onelibrary_sync::ext::decode(&std::fs::read(stick.db_dir().join("exportExt.pdb")).unwrap()).unwrap();
    assert_eq!(ext.tagged, vec![(1, 11)]);
    assert_eq!(ext.master_db_id, Some(77));

    let again = plan(stick.mount()).unwrap();
    assert!(!again.changes.any(), "{:?}", again.changes);
    assert_eq!(again.device_library.as_ref().map(|c| (c.tracks, c.playlists, c.my_tags)), Some((3, 2, 1)));
}

#[test]
fn a_stick_already_in_step_is_not_written() {
    let stick = stick(3);
    stick.sync();
    let pdb = stick.db_dir().join("export.pdb");
    let before = (std::fs::read(&pdb).unwrap(), std::fs::metadata(&pdb).unwrap().modified().unwrap());
    let report = stick.sync();
    assert!(!report.changes.any());
    assert_eq!((std::fs::read(&pdb).unwrap(), std::fs::metadata(&pdb).unwrap().modified().unwrap()), before);
    assert!(!stick.mount().join(onelibrary_sync::PUBLICATION).exists());
}

/// The page type of each page of a file.
fn page_types(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks(device::PAGE_SIZE).map(|p| u32::from_le_bytes(p[8..12].try_into().unwrap())).collect()
}

#[test]
fn a_playlist_change_rewrites_only_the_playlist_tables() {
    let stick = stick(400);
    stick.sync();
    let before = stick.pdb();
    stick.edit(
        "DELETE FROM playlist_content WHERE playlist_id = 2 AND content_id = 5;
         UPDATE playlist SET name = 'Friday late' WHERE playlist_id = 2;
         INSERT INTO playlist (playlist_id, sequenceNo, name, attribute, playlist_id_parent) VALUES (4, 2, 'Sunday', 0, 1);
         INSERT INTO playlist_content (playlist_id, content_id, sequenceNo) VALUES (4, 7, 1);",
    );
    let planned = plan(stick.mount()).unwrap();
    assert_eq!(planned.changes.tables, vec![Table::Playlists, Table::PlaylistEntries]);
    assert_eq!((planned.changes.playlists_added, planned.changes.playlists_updated, planned.changes.playlists_removed), (1, 1, 0));
    assert_eq!(planned.changes.tracks_added + planned.changes.tracks_updated + planned.changes.tracks_removed, 0);
    assert!(!planned.changes.my_tags);
    let ext_before = std::fs::read(stick.db_dir().join("exportExt.pdb")).unwrap();

    sync(stick.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    let after = stick.pdb();
    let (types_before, types_after) = (page_types(&before), page_types(&after));
    for (i, page) in after.chunks(device::PAGE_SIZE).enumerate().skip(1) {
        let playlist_page = |types: &[u32]| types.get(i).is_some_and(|t| *t == 7 || *t == 8);
        if playlist_page(&types_before) || playlist_page(&types_after) {
            continue;
        }
        let old = before.chunks(device::PAGE_SIZE).nth(i);
        assert!(old.is_none_or(|old| old == page) || page.iter().all(|&b| b == 0), "page {i} changed");
    }
    let contents = stick.contents();
    assert_eq!(contents.tree[&2].name, "Friday late");
    assert_eq!(contents.entries[&2].len(), 399);
    assert!(!contents.entries[&2].contains(&5));
    assert_eq!(contents.entries[&4], vec![7]);
    assert_eq!(contents.tracks.len(), 400);
    assert_eq!(std::fs::read(stick.db_dir().join("exportExt.pdb")).unwrap(), ext_before, "My Tags did not change");
    assert!(!plan(stick.mount()).unwrap().changes.any());
}

#[test]
fn tracks_added_changed_and_removed_follow_the_one_library() {
    let stick = stick(3);
    stick.sync();
    stick.edit(
        "DELETE FROM content WHERE content_id = 3;
         DELETE FROM playlist_content WHERE content_id = 3;
         UPDATE content SET title = 'Renamed', rating = 5 WHERE content_id = 2;
         INSERT INTO content (content_id, title, path, fileName, artist_id_artist, bpmx100, rating)
           VALUES (9, 'New', '/Contents/New.mp3', 'New.mp3', 1, 12000, 3);
         INSERT INTO playlist_content (playlist_id, content_id, sequenceNo) VALUES (3, 9, 2);
         UPDATE property SET numberOfContents = 3;",
    );
    let planned = plan(stick.mount()).unwrap();
    assert_eq!((planned.changes.tracks_added, planned.changes.tracks_updated, planned.changes.tracks_removed), (1, 1, 1));
    assert!(planned.changes.tables.contains(&Table::Tracks));
    // Still three tracks, under the same name: the property row stays.
    assert!(!planned.changes.tables.contains(&Table::Property));
    assert!(!planned.changes.my_tags, "only track 1 carries a tag");
    assert_eq!(planned.changes.records, vec![Records { table: Table::Artists, added: 0, updated: 0, removed: 1 }]);
    assert!(planned.changes.settings.is_empty());
    sync(stick.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    let contents = stick.contents();
    assert_eq!(contents.tracks.keys().copied().collect::<Vec<_>>(), vec![1, 2, 9]);
    assert_eq!(contents.tracks[&2].title, "Renamed");
    assert_eq!(contents.tracks[&2].rating, 5);
    assert_eq!(contents.entries[&2], vec![1, 2]);
    assert_eq!(contents.entries[&3], vec![1, 9]);
    assert_eq!(contents.property.as_ref().map(|p| p.1), Some(3));
    // Artist 3 belonged to the removed track only.
    assert!(!contents.artists.values().any(|a| a == "Artist 3"));
}

#[test]
fn a_setting_change_says_what_the_device_library_has_and_what_it_becomes() {
    let stick = stick(3);
    stick.sync();
    stick.edit("UPDATE sort SET isSelectedAsSubColumn = (menuItem_id = 5);");
    let planned = plan(stick.mount()).unwrap();
    assert_eq!(planned.changes.tables, vec![Table::Sorts]);
    assert!(planned.changes.records.is_empty());
    assert_eq!(
        planned.changes.settings,
        vec![Setting { name: "Column beside the track name".into(), now: "Comments".into(), becomes: "BPM".into() }]
    );
    stick.sync();
    assert!(!plan(stick.mount()).unwrap().changes.any());
}

#[test]
fn the_play_history_a_player_recorded_is_kept() {
    let stick = stick(3);
    stick.sync();
    // A player recorded a session in the Device Library.
    let mut contents = stick.contents();
    contents.history = vec![device::History { id: 1, name: "HISTORY 001".into(), tracks: vec![2, 1] }];
    let one = onelibrary_sync::source::read(&stick.one_library()).unwrap();
    let rows = device::rows(&one, Some(&contents), 0, "2026-10-02").unwrap();
    std::fs::write(stick.db_dir().join("export.pdb"), device::build(&rows, &contents.history)).unwrap();
    assert!(!plan(stick.mount()).unwrap().changes.tables.contains(&Table::Tracks));

    stick.edit("UPDATE content SET title = 'Changed' WHERE content_id = 1;");
    stick.sync();
    let after = stick.contents();
    assert_eq!(after.tracks[&1].title, "Changed");
    assert_eq!(after.history, contents.history);
}

#[test]
fn a_stick_that_changed_after_its_plan_is_not_written() {
    let stick = stick(3);
    let planned = plan(stick.mount()).unwrap();
    stick.edit("UPDATE content SET title = 'Late edit' WHERE content_id = 1;");
    let refused = sync(stick.mount(), &planned.fingerprint, &mut |_| {});
    assert!(matches!(refused, Err(Error::Changed)), "{refused:?}");
    assert!(!stick.db_dir().join("export.pdb").exists());
}

#[test]
fn a_write_in_the_same_second_as_the_plan_is_noticed() {
    let stick = stick(3);
    stick.sync();
    let pdb = stick.db_dir().join("export.pdb");
    let modified = std::fs::metadata(&pdb).unwrap().modified().unwrap();
    let planned = plan(stick.mount()).unwrap();
    // A player saves to the stick within the same second, on a file system
    // that keeps whole seconds (HFS+, FAT): same length, same time.
    let mut bytes = stick.pdb();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&pdb, &bytes).unwrap();
    std::fs::File::options().write(true).open(&pdb).unwrap().set_modified(modified).unwrap();
    assert!(matches!(sync(stick.mount(), &planned.fingerprint, &mut |_| {}), Err(Error::Changed)));
    assert_eq!(stick.pdb(), bytes);
}

#[test]
fn an_interrupted_write_is_reported_and_finished() {
    let stick = stick(3);
    stick.sync();
    let journal = stick.mount().join(onelibrary_sync::PUBLICATION);
    std::fs::create_dir(&journal).unwrap();
    std::fs::write(journal.join("publication.json"), b"[]").unwrap();
    assert!(matches!(plan(stick.mount()), Err(Error::Interrupted)));
    onelibrary_sync::recover(stick.mount()).unwrap();
    assert!(!journal.exists());
    assert!(!plan(stick.mount()).unwrap().changes.any());
}

#[test]
fn a_stick_without_a_one_library_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(plan(dir.path()), Err(Error::NoOneLibrary)));
}

#[test]
fn a_hidden_root_is_synced_where_it_is() {
    let stick = stick(2);
    std::fs::rename(stick.mount().join("PIONEER"), stick.mount().join(".PIONEER")).unwrap();
    let planned = plan(stick.mount()).unwrap();
    sync(stick.mount(), &planned.fingerprint, &mut |_| {}).unwrap();
    assert!(stick.mount().join(".PIONEER/rekordbox/export.pdb").is_file());
    assert!(!stick.mount().join("PIONEER").exists());
}
