//! Reading a stick's `OneLibrary` (`exportLibrary.db`): everything the
//! Device Library carries, read once, read-only.
//!
//! The columns are rekordbox's `OneLibrary` schema (`rbl_onelibrary::build`).
//! Other programs write the same file, so every value is read leniently: a
//! `NULL`, or a number stored as text, reads as the value it stands for
//! rather than failing the whole read.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, Row};

use crate::{Error, Result};

/// A track as the Device Library will carry it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Track {
    pub id: u32,
    pub title: String,
    pub artist_id: u32,
    pub remixer_id: u32,
    pub original_artist_id: u32,
    pub composer_id: u32,
    pub album_id: u32,
    pub genre_id: u32,
    pub label_id: u32,
    pub key_id: u32,
    pub color_id: u8,
    pub image_id: u32,
    pub comment: String,
    /// Stars, 0 to 5.
    pub rating: u8,
    pub bpm_x100: u32,
    pub length: u16,
    pub track_no: u32,
    pub disc_no: u16,
    pub year: u16,
    pub release_date: String,
    pub date_created: String,
    pub date_added: String,
    pub path: String,
    pub file_name: String,
    pub file_size: u32,
    pub bitrate: u32,
    pub bit_depth: u16,
    pub sample_rate: u32,
    pub isrc: String,
    pub subtitle: String,
    pub play_count: u16,
    pub hot_cue_auto_load: bool,
    pub master_db_id: u32,
    pub master_content_id: u32,
    pub analysis_path: String,
}

/// A playlist or folder, with its tracks in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Playlist {
    pub id: u32,
    pub parent: u32,
    pub sequence: u32,
    pub folder: bool,
    pub name: String,
    pub tracks: Vec<u32>,
}

/// One My Tag: a category (`attribute` 1) or a tag under one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyTag {
    pub id: u64,
    pub seq: u32,
    pub name: String,
    pub attribute: u8,
    pub parent: u64,
}

/// What a `OneLibrary` holds.
#[derive(Debug, Clone, Default)]
pub struct OneLibrary {
    /// By id.
    pub tracks: Vec<Track>,
    pub artists: BTreeMap<u32, String>,
    /// Name and artist.
    pub albums: BTreeMap<u32, (String, u32)>,
    pub genres: BTreeMap<u32, String>,
    pub labels: BTreeMap<u32, String>,
    pub keys: BTreeMap<u32, String>,
    pub images: BTreeMap<u32, String>,
    pub playlists: Vec<Playlist>,
    pub my_tags: Vec<MyTag>,
    /// Each tagged track's tags.
    pub tagged: BTreeMap<u32, Vec<u64>>,
    pub my_tag_master_db_id: u32,
    /// The device name, colour comments, browse menus and background colour.
    pub settings: rbl_onelibrary::settings::StickSettings,
    /// History sessions, counted for display; they are not carried.
    pub histories: usize,
}

/// The `dbVersion` this app reads. Another version may change what a column
/// means, so a stick carrying one is left alone.
pub const DB_VERSION: &str = rbl_onelibrary::build::DB_VERSION;

fn int(row: &Row<'_>, at: usize) -> rusqlite::Result<i64> {
    Ok(match row.get_ref(at)? {
        ValueRef::Integer(v) => v,
        #[allow(clippy::cast_possible_truncation, reason = "a whole number stored as a float")]
        ValueRef::Real(v) => v as i64,
        ValueRef::Text(t) => std::str::from_utf8(t).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0),
        ValueRef::Null | ValueRef::Blob(_) => 0,
    })
}

fn text(row: &Row<'_>, at: usize) -> rusqlite::Result<String> {
    Ok(match row.get_ref(at)? {
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Null | ValueRef::Blob(_) => String::new(),
    })
}

/// A number that has to fit its field. An id out of range would name
/// another row, so it is refused; a figure out of range is clamped.
fn id(row: &Row<'_>, at: usize, what: &str) -> rusqlite::Result<std::result::Result<u32, String>> {
    let value = int(row, at)?;
    Ok(u32::try_from(value.max(0)).map_err(|_| format!("{what} {value} is too large for the Device Library")))
}

fn clamp_u16(value: i64) -> u16 {
    u16::try_from(value.clamp(0, i64::from(u16::MAX))).unwrap_or(u16::MAX)
}

fn clamp_u32(value: i64) -> u32 {
    u32::try_from(value.clamp(0, i64::from(u32::MAX))).unwrap_or(u32::MAX)
}

/// Stars from `content.rating`: rekordbox stores 0 to 5, and rbxport once
/// stored multiples of 51, the collection XML's scale. Both read back to the
/// same stars (rbxport `snapshot::onelibrary_stars`, #284).
fn stars(raw: i64) -> u8 {
    let stars = if raw > 5 { raw / 51 } else { raw };
    u8::try_from(stars.clamp(0, 5)).unwrap_or(0)
}

/// Reads the `OneLibrary` at `path`.
pub fn read(path: &Path) -> Result<OneLibrary> {
    let db = rbl_onelibrary::ExportLibrary::open_read_only(path).map_err(Error::one)?;
    let conn = db.connection();
    // One read transaction, so every table is read from the same moment.
    conn.execute_batch("BEGIN").map_err(Error::one)?;
    let version: String = conn
        .query_row("SELECT COALESCE(dbVersion, '') FROM property LIMIT 1", [], |r| text(r, 0))
        .map_err(Error::one)?;
    if version != DB_VERSION {
        return Err(Error::Unsupported(format!(
            "This USB's OneLibrary is version {version}; this app reads version {DB_VERSION}. Nothing was changed."
        )));
    }
    let mut library = OneLibrary {
        settings: rbl_onelibrary::settings::StickSettings::read(path).map_err(Error::one)?,
        my_tag_master_db_id: conn
            .query_row("SELECT COALESCE(myTagMasterDBID, 0) FROM property LIMIT 1", [], |r| int(r, 0))
            .map(clamp_u32)
            .map_err(Error::one)?,
        ..OneLibrary::default()
    };
    read_tracks(conn, &mut library)?;
    read_names(conn, &mut library)?;
    read_playlists(conn, &mut library)?;
    read_tags(conn, &mut library)?;
    library.histories = conn
        .query_row("SELECT COUNT(*) FROM history WHERE COALESCE(attribute, 0) != 1", [], |r| int(r, 0))
        .map(|n| usize::try_from(n).unwrap_or(0))
        .map_err(Error::one)?;
    conn.execute_batch("COMMIT").map_err(Error::one)?;
    Ok(library)
}

const TRACK_COLUMNS: &str = "content_id, title, artist_id_artist, artist_id_remixer, artist_id_originalArtist,
    artist_id_composer, album_id, genre_id, label_id, key_id, color_id, image_id, djComment, rating,
    bpmx100, length, trackNo, discNo, releaseYear, releaseDate, dateCreated, dateAdded, path, fileName,
    fileSize, bitrate, bitDepth, samplingRate, isrc, subtitle, djPlayCount, isHotCueAutoLoadOn,
    masterDbId, masterContentId, analysisDataFilePath";

fn read_tracks(conn: &Connection, library: &mut OneLibrary) -> Result<()> {
    let mut statement = conn
        .prepare(&format!("SELECT {TRACK_COLUMNS} FROM content ORDER BY content_id"))
        .map_err(Error::one)?;
    let rows = statement
        .query_map([], |r| {
            let ids = [
                id(r, 0, "Track id")?,
                id(r, 2, "Artist id")?,
                id(r, 3, "Artist id")?,
                id(r, 4, "Artist id")?,
                id(r, 5, "Artist id")?,
                id(r, 6, "Album id")?,
                id(r, 7, "Genre id")?,
                id(r, 8, "Label id")?,
                id(r, 9, "Key id")?,
                id(r, 11, "Artwork id")?,
            ];
            let ids: std::result::Result<Vec<u32>, String> = ids.into_iter().collect();
            Ok(ids.map(|ids| Track {
                id: ids[0],
                title: text(r, 1).unwrap_or_default(),
                artist_id: ids[1],
                remixer_id: ids[2],
                original_artist_id: ids[3],
                composer_id: ids[4],
                album_id: ids[5],
                genre_id: ids[6],
                label_id: ids[7],
                key_id: ids[8],
                color_id: u8::try_from(int(r, 10).unwrap_or(0).clamp(0, 8)).unwrap_or(0),
                image_id: ids[9],
                comment: text(r, 12).unwrap_or_default(),
                rating: stars(int(r, 13).unwrap_or(0)),
                bpm_x100: clamp_u32(int(r, 14).unwrap_or(0)),
                length: clamp_u16(int(r, 15).unwrap_or(0)),
                track_no: clamp_u32(int(r, 16).unwrap_or(0)),
                disc_no: clamp_u16(int(r, 17).unwrap_or(0)),
                year: clamp_u16(int(r, 18).unwrap_or(0)),
                release_date: text(r, 19).unwrap_or_default(),
                date_created: text(r, 20).unwrap_or_default(),
                date_added: text(r, 21).unwrap_or_default(),
                path: text(r, 22).unwrap_or_default(),
                file_name: text(r, 23).unwrap_or_default(),
                file_size: clamp_u32(int(r, 24).unwrap_or(0)),
                bitrate: clamp_u32(int(r, 25).unwrap_or(0)),
                bit_depth: clamp_u16(int(r, 26).unwrap_or(0)),
                sample_rate: clamp_u32(int(r, 27).unwrap_or(0)),
                isrc: text(r, 28).unwrap_or_default(),
                subtitle: text(r, 29).unwrap_or_default(),
                play_count: clamp_u16(int(r, 30).unwrap_or(0)),
                hot_cue_auto_load: int(r, 31).unwrap_or(0) != 0,
                // As rbxport writes them: a number past 32 bits is left blank.
                master_db_id: u32::try_from(int(r, 32).unwrap_or(0)).unwrap_or(0),
                master_content_id: u32::try_from(int(r, 33).unwrap_or(0)).unwrap_or(0),
                analysis_path: text(r, 34).unwrap_or_default(),
            }))
        })
        .map_err(Error::one)?;
    for row in rows {
        let track = row.map_err(Error::one)?.map_err(Error::Unsupported)?;
        if track.id == 0 {
            return Err(Error::Unsupported("A OneLibrary track has id 0, which the Device Library cannot hold.".into()));
        }
        library.tracks.push(track);
    }
    Ok(())
}

fn read_names(conn: &Connection, library: &mut OneLibrary) -> Result<()> {
    let names = |table: &str, id_column: &str, name_column: &str| -> Result<BTreeMap<u32, String>> {
        let mut statement = conn
            .prepare(&format!("SELECT {id_column}, {name_column} FROM {table} ORDER BY {id_column}"))
            .map_err(Error::one)?;
        let rows = statement.query_map([], |r| Ok((int(r, 0)?, text(r, 1)?))).map_err(Error::one)?;
        let mut out = BTreeMap::new();
        for row in rows {
            let (id, name) = row.map_err(Error::one)?;
            if let Ok(id) = u32::try_from(id) {
                if id != 0 {
                    out.insert(id, name);
                }
            }
        }
        Ok(out)
    };
    library.artists = names("artist", "artist_id", "name")?;
    library.genres = names("genre", "genre_id", "name")?;
    library.labels = names("label", "label_id", "name")?;
    library.keys = names("key", "key_id", "name")?;
    library.images = names("image", "image_id", "path")?;
    let album_artists = names("album", "album_id", "artist_id")?;
    library.albums = names("album", "album_id", "name")?
        .into_iter()
        .map(|(id, name)| {
            let artist = album_artists.get(&id).and_then(|a| a.parse().ok()).unwrap_or(0);
            (id, (name, artist))
        })
        .collect();
    Ok(())
}

fn read_playlists(conn: &Connection, library: &mut OneLibrary) -> Result<()> {
    let mut statement = conn
        .prepare(
            "SELECT playlist_id, playlist_id_parent, sequenceNo, attribute, name
             FROM playlist ORDER BY playlist_id",
        )
        .map_err(Error::one)?;
    let rows = statement
        .query_map([], |r| {
            Ok((id(r, 0, "Playlist id")?, id(r, 1, "Playlist id")?, int(r, 2)?, int(r, 3)?, text(r, 4)?))
        })
        .map_err(Error::one)?;
    let mut by_id = BTreeMap::new();
    for row in rows {
        let (id, parent, sequence, attribute, name) = row.map_err(Error::one)?;
        let (id, parent) = (id.map_err(Error::Unsupported)?, parent.map_err(Error::Unsupported)?);
        if id == 0 {
            continue;
        }
        by_id.insert(id, Playlist { id, parent, sequence: clamp_u32(sequence), folder: attribute == 1, name, tracks: Vec::new() });
    }
    let mut statement = conn
        .prepare("SELECT playlist_id, content_id FROM playlist_content ORDER BY playlist_id, sequenceNo, rowid")
        .map_err(Error::one)?;
    let known: std::collections::BTreeSet<u32> = library.tracks.iter().map(|t| t.id).collect();
    let rows = statement.query_map([], |r| Ok((int(r, 0)?, int(r, 1)?))).map_err(Error::one)?;
    for row in rows {
        let (playlist, content) = row.map_err(Error::one)?;
        let (Ok(playlist), Ok(content)) = (u32::try_from(playlist), u32::try_from(content)) else { continue };
        // An entry naming a track the library does not hold has nothing a
        // player could load.
        if let Some(list) = by_id.get_mut(&playlist).filter(|p| !p.folder && known.contains(&content)) {
            list.tracks.push(content);
        }
    }
    // A playlist whose parent is gone, or is not a folder, goes to the top,
    // where a player can still reach it.
    let folders: std::collections::BTreeSet<u32> = by_id.values().filter(|p| p.folder).map(|p| p.id).collect();
    for playlist in by_id.values_mut() {
        if playlist.parent != 0 && (!folders.contains(&playlist.parent) || playlist.parent == playlist.id) {
            playlist.parent = 0;
        }
    }
    library.playlists = by_id.into_values().collect();
    Ok(())
}

fn read_tags(conn: &Connection, library: &mut OneLibrary) -> Result<()> {
    let mut statement = conn
        .prepare("SELECT myTag_id, sequenceNo, name, attribute, myTag_id_parent FROM myTag ORDER BY myTag_id")
        .map_err(Error::one)?;
    let rows = statement
        .query_map([], |r| {
            Ok(MyTag {
                id: u64::try_from(int(r, 0)?).unwrap_or(0),
                seq: clamp_u32(int(r, 1)?),
                name: text(r, 2)?,
                attribute: u8::from(int(r, 3)? == 1),
                parent: u64::try_from(int(r, 4)?).unwrap_or(0),
            })
        })
        .map_err(Error::one)?;
    for row in rows {
        let tag = row.map_err(Error::one)?;
        if tag.id != 0 {
            library.my_tags.push(tag);
        }
    }
    number_tags(&mut library.my_tags);
    let mut statement =
        conn.prepare("SELECT content_id, myTag_id FROM myTag_content ORDER BY content_id, myTag_id").map_err(Error::one)?;
    let rows = statement.query_map([], |r| Ok((int(r, 0)?, int(r, 1)?))).map_err(Error::one)?;
    for row in rows {
        let (content, tag) = row.map_err(Error::one)?;
        if let (Ok(content), Ok(tag)) = (u32::try_from(content), u64::try_from(tag)) {
            library.tagged.entry(content).or_default().push(tag);
        }
    }
    Ok(())
}

/// Numbers the categories, and the tags under each, from 1 in their order.
///
/// `exportExt.pdb` carries a tag's place among its siblings from 0, and
/// [`crate::ext`] writes `Seq − 1`, as the master library numbers them from
/// one. A `OneLibrary` rekordbox wrote numbers them from 0 and one rbxport
/// wrote from 1 [OBS: rekordbox 7 and rbxport sticks, against the
/// `exportExt.pdb` beside each: 0 for the first tag in all of them].
fn number_tags(tags: &mut [MyTag]) {
    let mut order: Vec<(bool, u64, u32, u64, usize)> =
        tags.iter().enumerate().map(|(at, t)| (t.attribute == 1, t.parent, t.seq, t.id, at)).collect();
    order.sort_unstable_by_key(|&(category, parent, seq, id, _)| (category, if category { 0 } else { parent }, seq, id));
    let mut last = None;
    let mut next = 1;
    for (category, parent, _, _, at) in order {
        let group = (category, if category { 0 } else { parent });
        if last != Some(group) {
            last = Some(group);
            next = 1;
        }
        tags[at].seq = next;
        next += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_numbered_from_one_among_their_siblings_whichever_way_they_were() {
        let tag = |id, seq, attribute, parent| MyTag { id, seq, name: String::new(), attribute, parent };
        let mut tags = vec![tag(5, 0, 1, 0), tag(11, 1, 1, 0), tag(20, 1, 0, 5), tag(21, 0, 0, 5), tag(30, 7, 0, 11)];
        number_tags(&mut tags);
        let seqs: Vec<u32> = tags.iter().map(|t| t.seq).collect();
        assert_eq!(seqs, vec![1, 2, 2, 1, 1]);
    }

    #[test]
    fn ratings_read_as_stars_on_either_scale() {
        assert_eq!(stars(0), 0);
        assert_eq!(stars(4), 4);
        assert_eq!(stars(204), 4);
        assert_eq!(stars(255), 5);
        assert_eq!(stars(-3), 0);
    }
}
