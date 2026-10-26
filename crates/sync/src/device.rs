//! The Device Library, `export.pdb`: its tables as a `OneLibrary` makes
//! them, its tables as a stick holds them, and the edit that turns one into
//! the other.
//!
//! Both sides are compared in one decoded form. The wanted tables are
//! encoded with rbxport's row encoders, laid into a scratch file and read
//! back with the same decoders that read the stick's file, so a table counts
//! as changed only when a player would read something different from it.
//! A file rekordbox wrote with the same content but other bytes is left
//! alone.
//!
//! A changed table is rewritten in place with
//! [`rbl_pdb::build::replace_table`], which leaves every other table's pages
//! byte for byte as they were. The play history players record in
//! `export.pdb` (types 11 and 12) is the stick's, not the `OneLibrary`'s,
//! and is never rewritten.

use std::collections::{BTreeMap, BTreeSet};

use rbl_pdb::build::{max_row_len, FileBuilder, PageBuilder};
use rbl_pdb::rows::{
    album_row, artist_row, artwork_row, color_row, key_row, playlist_entry_row, playlist_row, property_row,
    set_index_shift, simple_named_row, track_row, PdbProperty, TrackInput,
};
use rbl_pdb::{Pdb, PageType, RowRef};

use crate::source::OneLibrary;
use crate::{Error, Result};

/// `export.pdb`'s page size, as rekordbox writes it.
pub const PAGE_SIZE: usize = 4096;

/// A table of `export.pdb` that this sync writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Table {
    Tracks,
    Genres,
    Artists,
    Albums,
    Labels,
    Keys,
    Colors,
    Playlists,
    PlaylistEntries,
    Artwork,
    Columns,
    Categories,
    Sorts,
    Property,
}

impl Table {
    pub const ALL: [Self; 14] = [
        Self::Tracks,
        Self::Genres,
        Self::Artists,
        Self::Albums,
        Self::Labels,
        Self::Keys,
        Self::Colors,
        Self::Playlists,
        Self::PlaylistEntries,
        Self::Artwork,
        Self::Columns,
        Self::Categories,
        Self::Sorts,
        Self::Property,
    ];

    /// The page type the file's header lists the table under.
    pub const fn page_type(self) -> u32 {
        match self {
            Self::Tracks => 0,
            Self::Genres => 1,
            Self::Artists => 2,
            Self::Albums => 3,
            Self::Labels => 4,
            Self::Keys => 5,
            Self::Colors => 6,
            Self::Playlists => 7,
            Self::PlaylistEntries => 8,
            Self::Artwork => 13,
            Self::Columns => 16,
            Self::Categories => 17,
            Self::Sorts => 18,
            Self::Property => 19,
        }
    }

    /// Rows that open with a subtype word carry their slot on the page
    /// ([`set_index_shift`]).
    const fn numbered(self) -> bool {
        matches!(self, Self::Tracks | Self::Artists | Self::Albums | Self::Property)
    }
}

/// A track row, decoded to every field this sync writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackFacts {
    pub title: String,
    pub artist_id: u32,
    pub remixer_id: u32,
    pub original_artist_id: u32,
    pub composer_id: u32,
    pub album_id: u32,
    pub genre_id: u32,
    pub label_id: u32,
    pub key_id: u32,
    pub artwork_id: u32,
    pub color_id: u8,
    pub rating: u8,
    pub tempo_x100: u32,
    pub duration_sec: u16,
    pub year: u16,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub sample_depth: u16,
    pub file_size: u32,
    pub track_number: u32,
    pub disc_number: u16,
    pub play_count: u16,
    pub master_content_id: u32,
    pub master_db_id: u32,
    pub hot_cue_auto_load: bool,
    pub file_path: String,
    pub filename: String,
    pub analyze_path: String,
    pub comment: String,
    pub date_added: String,
    pub date_created: String,
    pub release_date: String,
    pub mix_name: String,
    pub isrc: String,
}

/// A playlist tree row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Node {
    pub parent: u32,
    pub sort_order: u32,
    pub folder: bool,
    pub name: String,
}

/// A history session a player recorded, with its tracks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    pub id: u32,
    pub name: String,
    pub tracks: Vec<u32>,
}

/// What an `export.pdb` holds, decoded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contents {
    /// The tables the header lists, by page type.
    pub listed: BTreeSet<u32>,
    pub tracks: BTreeMap<u32, TrackFacts>,
    pub genres: BTreeMap<u32, String>,
    pub artists: BTreeMap<u32, String>,
    /// Artist and name.
    pub albums: BTreeMap<u32, (u32, String)>,
    pub labels: BTreeMap<u32, String>,
    pub keys: BTreeMap<u32, String>,
    pub colors: BTreeMap<u32, String>,
    pub tree: BTreeMap<u32, Node>,
    /// Each playlist's tracks in entry order.
    pub entries: BTreeMap<u32, Vec<u32>>,
    pub artwork: BTreeMap<u32, String>,
    pub columns: Vec<(u16, u16, String)>,
    pub categories: Vec<[u8; 8]>,
    pub sorts: Vec<[u8; 8]>,
    /// Device name, track count and background colour. The date is left
    /// out: a Device Library is not out of date for having been written on
    /// another day.
    pub property: Option<(String, u32, u8)>,
    pub history: Vec<History>,
}

impl Contents {
    /// Whether `table` reads the same in both.
    pub fn same(&self, other: &Self, table: Table) -> bool {
        let listed = self.listed.contains(&table.page_type()) && other.listed.contains(&table.page_type());
        listed
            && match table {
                Table::Tracks => self.tracks == other.tracks,
                Table::Genres => self.genres == other.genres,
                Table::Artists => self.artists == other.artists,
                Table::Albums => self.albums == other.albums,
                Table::Labels => self.labels == other.labels,
                Table::Keys => self.keys == other.keys,
                Table::Colors => self.colors == other.colors,
                Table::Playlists => self.tree == other.tree,
                Table::PlaylistEntries => self.entries == other.entries,
                Table::Artwork => self.artwork == other.artwork,
                Table::Columns => self.columns == other.columns,
                Table::Categories => self.categories == other.categories,
                Table::Sorts => self.sorts == other.sorts,
                Table::Property => self.property == other.property,
            }
    }
}

fn eight(pdb: &Pdb<'_>, row: RowRef) -> [u8; 8] {
    std::array::from_fn(|i| pdb.u1_at(row, i))
}

/// Reads an `export.pdb`.
pub fn decode(bytes: &[u8]) -> Result<Contents> {
    let pdb = Pdb::parse(bytes).map_err(|e| Error::Unreadable(format!("export.pdb: {e}")))?;
    let mut out = Contents { listed: pdb.tables.iter().map(|t| page_number(t.page_type)).collect(), ..Contents::default() };
    let named = |kind: PageType| -> BTreeMap<u32, String> {
        pdb.table(kind).map(|t| pdb.named_rows(t).into_iter().map(|n| (n.id, n.name)).collect()).unwrap_or_default()
    };
    out.genres = named(PageType::Genres);
    out.artists = named(PageType::Artists);
    out.labels = named(PageType::Labels);
    out.keys = named(PageType::Keys);
    out.colors = named(PageType::Colors);
    out.artwork = named(PageType::Artwork);
    if let Some(table) = pdb.table(PageType::Albums) {
        for (row, album) in pdb.rows(table).into_iter().zip(pdb.named_rows(table)) {
            out.albums.insert(album.id, (pdb.u4_at(row, 0x08), album.name));
        }
    }
    if let Some(table) = pdb.table(PageType::Tracks) {
        for (row, t) in pdb.rows(table).into_iter().zip(pdb.track_rows(table)) {
            let text = |slot: usize| pdb.string_ref(row, 0x5e + slot * 2);
            out.tracks.insert(
                t.id,
                TrackFacts {
                    remixer_id: pdb.u4_at(row, 0x2c),
                    original_artist_id: pdb.u4_at(row, 0x24),
                    composer_id: pdb.u4_at(row, 0x0c),
                    mix_name: text(rbl_pdb::rows::slot::MIX_NAME),
                    isrc: text(rbl_pdb::rows::slot::ISRC),
                    title: t.title,
                    artist_id: t.artist_id,
                    album_id: t.album_id,
                    genre_id: t.genre_id,
                    label_id: t.label_id,
                    key_id: t.key_id,
                    artwork_id: t.artwork_id,
                    color_id: t.color_id,
                    rating: t.rating,
                    tempo_x100: t.tempo_x100,
                    duration_sec: t.duration_sec,
                    year: t.year,
                    bitrate: t.bitrate,
                    sample_rate: t.sample_rate,
                    sample_depth: t.sample_depth,
                    file_size: t.file_size,
                    track_number: t.track_number,
                    disc_number: t.disc_number,
                    play_count: t.play_count,
                    master_content_id: t.master_content_id,
                    master_db_id: t.master_db_id,
                    hot_cue_auto_load: t.hot_cue_auto_load,
                    file_path: t.file_path,
                    filename: t.filename,
                    analyze_path: t.analyze_path,
                    comment: t.comment,
                    date_added: t.date_added,
                    date_created: t.date_created,
                    release_date: t.release_date,
                },
            );
        }
    }
    if let Some(table) = pdb.table(PageType::PlaylistTree) {
        for node in pdb.playlist_nodes(table) {
            out.tree.insert(
                node.id,
                Node { parent: node.parent_id, sort_order: node.sort_order, folder: node.is_folder, name: node.name },
            );
        }
    }
    rank_siblings(&mut out.tree);
    if let Some(table) = pdb.table(PageType::PlaylistEntries) {
        let mut entries = pdb.playlist_entries(table);
        entries.sort_by_key(|e| (e.playlist_id, e.entry_index));
        for entry in entries {
            out.entries.entry(entry.playlist_id).or_default().push(entry.track_id);
        }
    }
    if let Some(table) = pdb.table(PageType::Columns) {
        out.columns = pdb
            .rows(table)
            .into_iter()
            .map(|row| (pdb.u2_at(row, 0), pdb.u2_at(row, 2), pdb.string_at(row.offset + 4)))
            .collect();
    }
    out.categories = pdb.table(PageType::Categories).map(|t| pdb.rows(t).into_iter().map(|r| eight(&pdb, r)).collect()).unwrap_or_default();
    out.sorts = pdb.table(PageType::Sorts).map(|t| pdb.rows(t).into_iter().map(|r| eight(&pdb, r)).collect()).unwrap_or_default();
    out.property = pdb.property().map(|p| (p.device_name, p.contents, p.background_color));
    let entries = pdb.played_history_entries();
    for session in pdb.played_histories() {
        let mut rows: Vec<_> = entries.iter().filter(|e| e.playlist_id == session.id).collect();
        rows.sort_by_key(|e| e.entry_index);
        out.history.push(History { id: session.id, name: session.name, tracks: rows.iter().map(|e| e.track_id).collect() });
    }
    Ok(out)
}

/// Replaces each node's sort order with its place among its siblings.
///
/// A player lists a folder's children in sort order, and that order is all
/// the number means: rekordbox numbers a stick's `export.pdb` siblings from 1
/// where its `exportLibrary.db` numbers them from 0 [OBS: rekordbox 7
/// sticks with both libraries], and both list the same.
fn rank_siblings(tree: &mut BTreeMap<u32, Node>) {
    let mut siblings: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for (id, node) in tree.iter() {
        siblings.entry(node.parent).or_default().push((node.sort_order, *id));
    }
    for children in siblings.values_mut() {
        children.sort_unstable();
        for (rank, (_, id)) in (0_u32..).zip(children.iter()) {
            if let Some(node) = tree.get_mut(id) {
                node.sort_order = rank;
            }
        }
    }
}

/// The number a [`PageType`] stands for.
fn page_number(kind: PageType) -> u32 {
    match kind {
        PageType::Tracks => 0,
        PageType::Genres => 1,
        PageType::Artists => 2,
        PageType::Albums => 3,
        PageType::Labels => 4,
        PageType::Keys => 5,
        PageType::Colors => 6,
        PageType::PlaylistTree => 7,
        PageType::PlaylistEntries => 8,
        PageType::Artwork => 13,
        PageType::Columns => 16,
        PageType::Categories => 17,
        PageType::Sorts => 18,
        PageType::Property => 19,
        PageType::Other(n) => n,
    }
}

/// The rows of every table this sync writes, encoded and numbered.
#[derive(Debug, Clone, Default)]
pub struct Rows {
    tables: BTreeMap<u32, Vec<Vec<u8>>>,
}

impl Rows {
    fn of(&self, table: Table) -> &[Vec<u8>] {
        self.tables.get(&table.page_type()).map_or(&[], Vec::as_slice)
    }
}

/// Gives each row its slot on the page it will land on, as
/// `FileBuilder::add_table_numbered` does. [`replace_table`] lays rows out
/// on pages the same way and does not number them itself.
///
/// [`replace_table`]: rbl_pdb::build::replace_table
fn numbered(rows: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut page = PageBuilder::new(PAGE_SIZE, 0, 0, 0);
    rows.into_iter()
        .map(|mut row| {
            if !page.fits(row.len()) && page.row_count() > 0 {
                page = PageBuilder::new(PAGE_SIZE, 0, 0, 0);
            }
            set_index_shift(&mut row, u16::try_from(page.row_count()).unwrap_or(u16::MAX));
            page.push_row(&row);
            row
        })
        .collect()
}

/// The eight colour labels a Device Library lists, under the stick's names
/// for them.
fn color_rows(one: &OneLibrary) -> Vec<Vec<u8>> {
    rbl_onelibrary::build::reference::COLORS
        .iter()
        .zip(1_u16..)
        .map(|(name, id)| {
            let name = one.settings.colors.iter().find(|c| c.id == i64::from(id)).map_or(*name, |c| c.name.as_str());
            color_row(id, name)
        })
        .collect()
}

/// The `categories` (type 17) and `sorts` (type 18) rows for a stick's
/// browse settings: rbxport's `rbl_export::menu_rows`. Players that read
/// only `export.pdb` take their Category, Sort and Column settings from these
/// rows.
fn menu_rows(settings: &rbl_onelibrary::settings::StickSettings) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    use rbl_onelibrary::build::reference::MENU_ITEMS;
    use rbl_onelibrary::settings::{renumbered, MenuSlot};
    let ordered = |slots: &[MenuSlot]| {
        let known = |s: &MenuSlot| MENU_ITEMS.iter().any(|(id, _, _)| *id == s.menu_item);
        let mut slots: Vec<MenuSlot> = renumbered(slots).into_iter().filter(known).collect();
        slots.sort_by_key(|s| (s.visible, if s.visible { s.seq } else { s.id }));
        slots
    };
    let id = |value: i64| u16::try_from(value).unwrap_or(0);
    let categories = ordered(&settings.categories)
        .iter()
        .map(|s| rbl_pdb::rows::category_row(id(s.menu_item), id(s.id), s.visible, id(s.seq)))
        .collect();
    let sorts = ordered(&settings.sorts)
        .iter()
        .map(|s| {
            let seq = u8::try_from(s.seq).unwrap_or(0);
            rbl_pdb::rows::sort_row(id(s.menu_item), id(s.id), s.visible, seq, settings.sub_column == Some(s.menu_item))
        })
        .collect();
    (categories, sorts)
}

/// A row longer than a page holds cannot be written: the page builder would
/// cut it off and the file would be corrupt.
fn fits(rows: &[Vec<u8>], what: impl Fn(usize) -> String) -> Result<()> {
    let limit = max_row_len(PAGE_SIZE);
    match rows.iter().position(|row| row.len() > limit) {
        Some(at) => Err(Error::TooLong(format!(
            "{} has more text than the Device Library can hold for one row ({} bytes; at most {limit}). Shorten it and sync again; the USB was left as it was.",
            what(at),
            rows[at].len()
        ))),
        None => Ok(()),
    }
}

/// The rows the Device Library should hold for `one`. `before` is what it
/// holds now, `background` its own background colour, and `date` the export
/// date the `property` row carries.
///
/// A track the Device Library already holds keeps the fields rekordbox
/// writes differently into its two libraries, so a stick rekordbox wrote is
/// not rewritten over them [OBS: rekordbox 7 sticks with both libraries]:
/// the master library's ids (0 in some `export.pdb`s, set in
/// `exportLibrary.db`), the file size (the collection's figure against the
/// file's) and the file's creation date (left empty in `exportLibrary.db`).
/// It also keeps a bit rate, sample rate or bit depth the `OneLibrary` does
/// not know. Every field a player shows comes from the `OneLibrary`.
pub fn rows(one: &OneLibrary, before: Option<&Contents>, background: u8, date: &str) -> Result<Rows> {
    let known = |map: &BTreeMap<u32, String>, id: u32| if map.contains_key(&id) { id } else { 0 };
    let mut used_artists = BTreeSet::new();
    let used = |set: &mut BTreeSet<u32>, id: u32| {
        if id != 0 {
            set.insert(id);
        }
        id
    };
    let (mut used_albums, mut used_genres, mut used_labels, mut used_keys, mut used_images) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new());

    let mut track_rows = Vec::with_capacity(one.tracks.len());
    for t in &one.tracks {
        let had = before.and_then(|b| b.tracks.get(&t.id));
        let known_or = |value: u32, had: Option<u32>| if value == 0 { had.unwrap_or(0) } else { value };
        let mut row = track_row(&TrackInput {
            hot_cue_auto_load: t.hot_cue_auto_load,
            id: t.id,
            artist_id: used(&mut used_artists, known(&one.artists, t.artist_id)),
            album_id: used(&mut used_albums, if one.albums.contains_key(&t.album_id) { t.album_id } else { 0 }),
            genre_id: used(&mut used_genres, known(&one.genres, t.genre_id)),
            key_id: used(&mut used_keys, known(&one.keys, t.key_id)),
            label_id: used(&mut used_labels, known(&one.labels, t.label_id)),
            artwork_id: used(&mut used_images, known(&one.images, t.image_id)),
            color_id: t.color_id,
            rating: t.rating,
            tempo_x100: t.bpm_x100,
            duration_sec: t.length,
            year: t.year,
            bitrate: known_or(t.bitrate, had.map(|h| h.bitrate)),
            sample_rate: known_or(t.sample_rate, had.map(|h| h.sample_rate)),
            file_size: had.map_or(t.file_size, |h| h.file_size),
            track_number: t.track_no,
            play_count: t.play_count,
            disc_number: t.disc_no,
            sample_depth: u16::try_from(known_or(u32::from(t.bit_depth), had.map(|h| u32::from(h.sample_depth)))).unwrap_or(0),
            master_db_id: had.map_or(t.master_db_id, |h| h.master_db_id),
            master_content_id: had.map_or(t.master_content_id, |h| h.master_content_id),
            title: t.title.clone(),
            filename: t.file_name.clone(),
            file_path: t.path.clone(),
            analyze_path: t.analysis_path.clone(),
            comment: t.comment.clone(),
            date_added: t.date_added.clone(),
            date_created: had.map_or_else(|| t.date_created.clone(), |h| h.date_created.clone()),
            release_date: t.release_date.clone(),
            mix_name: t.subtitle.clone(),
            isrc: t.isrc.clone(),
        });
        // The encoder leaves the other credits at zero; the columns are
        // there in the row, and a player's REMIXER and ORIGINAL ARTIST
        // categories read them.
        for (at, artist) in [(0x0c, t.composer_id), (0x24, t.original_artist_id), (0x2c, t.remixer_id)] {
            let artist = used(&mut used_artists, known(&one.artists, artist));
            if let Some(word) = row.get_mut(at..at + 4) {
                word.copy_from_slice(&artist.to_le_bytes());
            }
        }
        track_rows.push(row);
    }
    fits(&track_rows, |at| format!("'{}'", one.tracks[at].title))?;

    let album_rows: Vec<(u32, Vec<u8>)> = used_albums
        .iter()
        .filter_map(|id| one.albums.get(id).map(|(name, artist)| (*id, name, *artist)))
        .map(|(id, name, artist)| (id, album_row(id, used(&mut used_artists, known(&one.artists, artist)), name)))
        .collect();
    let named = |set: &BTreeSet<u32>, names: &BTreeMap<u32, String>, encode: fn(u32, &str) -> Vec<u8>| -> Vec<(u32, Vec<u8>)> {
        set.iter().filter_map(|id| names.get(id).map(|name| (*id, encode(*id, name)))).collect()
    };
    let artist_rows = named(&used_artists, &one.artists, artist_row);
    let genre_rows = named(&used_genres, &one.genres, simple_named_row);
    let label_rows = named(&used_labels, &one.labels, simple_named_row);
    let key_rows = named(&used_keys, &one.keys, key_row);
    let artwork_rows = named(&used_images, &one.images, artwork_row);
    for (kind, rows, names) in [
        ("The artist", &artist_rows, &one.artists),
        ("The genre", &genre_rows, &one.genres),
        ("The label", &label_rows, &one.labels),
        ("The key", &key_rows, &one.keys),
        ("The artwork path", &artwork_rows, &one.images),
    ] {
        let encoded: Vec<Vec<u8>> = rows.iter().map(|(_, row)| row.clone()).collect();
        fits(&encoded, |at| format!("{kind} '{}'", names.get(&rows[at].0).map_or("", String::as_str)))?;
    }
    let encoded_albums: Vec<Vec<u8>> = album_rows.iter().map(|(_, row)| row.clone()).collect();
    fits(&encoded_albums, |at| format!("The album '{}'", one.albums.get(&album_rows[at].0).map_or("", |a| a.0.as_str())))?;

    let tree_rows: Vec<Vec<u8>> =
        one.playlists.iter().map(|p| playlist_row(p.id, p.parent, p.sequence, p.folder, &p.name)).collect();
    fits(&tree_rows, |at| format!("The playlist '{}'", one.playlists[at].name))?;
    let entry_rows: Vec<Vec<u8>> = one
        .playlists
        .iter()
        .filter(|p| !p.folder)
        .flat_map(|p| p.tracks.iter().zip(1_u32..).map(|(track, position)| playlist_entry_row(position, *track, p.id)))
        .collect();

    let (categories, sorts) = menu_rows(&one.settings);
    let property = PdbProperty {
        device_name: one.settings.device_name.clone(),
        contents: u32::try_from(one.tracks.len()).unwrap_or(u32::MAX),
        created_date: date.to_owned(),
        background_color: background,
        ..PdbProperty::default()
    };
    let property = property_row(&property).ok_or_else(|| Error::Unsupported(format!("'{date}' is not a date the Device Library can carry")))?;

    let strip = |rows: Vec<(u32, Vec<u8>)>| rows.into_iter().map(|(_, row)| row).collect::<Vec<_>>();
    let mut tables = BTreeMap::new();
    for (table, rows) in [
        (Table::Tracks, track_rows),
        (Table::Genres, strip(genre_rows)),
        (Table::Artists, strip(artist_rows)),
        (Table::Albums, strip(album_rows)),
        (Table::Labels, strip(label_rows)),
        (Table::Keys, strip(key_rows)),
        (Table::Colors, color_rows(one)),
        (Table::Playlists, tree_rows),
        (Table::PlaylistEntries, entry_rows),
        (Table::Artwork, strip(artwork_rows)),
        (Table::Columns, rbl_pdb::reference::COLUMNS.iter().map(|row| row.to_vec()).collect()),
        (Table::Categories, categories),
        (Table::Sorts, sorts),
        (Table::Property, vec![property]),
    ] {
        tables.insert(table.page_type(), if table.numbered() { numbered(rows) } else { rows });
    }
    Ok(Rows { tables })
}

/// A whole `export.pdb` with the twenty tables rekordbox writes, in its
/// order (rbxport `build_pdb`), carrying `history` as the stick had it.
pub fn build(rows: &Rows, history: &[History]) -> Vec<u8> {
    let mut file = FileBuilder::new(PAGE_SIZE);
    for page_type in 0..=19 {
        let carried;
        let table: &[Vec<u8>] = match page_type {
            11 => {
                carried = history.iter().map(|h| simple_named_row(h.id, &h.name)).collect::<Vec<_>>();
                &carried
            }
            12 => {
                carried = history
                    .iter()
                    .flat_map(|h| {
                        h.tracks.iter().zip(1_u32..).map(|(track, position)| {
                            [track.to_le_bytes(), h.id.to_le_bytes(), position.to_le_bytes()].concat()
                        })
                    })
                    .collect::<Vec<_>>();
                &carried
            }
            _ => rows.tables.get(&page_type).map_or(&[], Vec::as_slice),
        };
        file.add_table(page_type, table);
    }
    file.finish()
}

/// What bringing an `export.pdb` up to date takes.
#[derive(Debug, Clone)]
pub struct Update {
    /// The file as it should be.
    pub bytes: Vec<u8>,
    /// The tables that change. All of them when the file is new or had to
    /// be rebuilt.
    pub tables: Vec<Table>,
    /// Whether the file is written whole rather than edited in place.
    pub rebuilt: bool,
    pub before: Option<Contents>,
    pub after: Contents,
}

/// Works out the Device Library for `one`, against the file the stick holds
/// now (`None` when it has none). `None` when the file is already right.
pub fn update(one: &OneLibrary, existing: Option<&[u8]>, date: &str) -> Result<Option<Update>> {
    let before = existing.map(decode).transpose();
    // An unreadable file is replaced: nothing in it can be carried.
    let before = match before {
        Ok(before) => before,
        Err(error) => {
            tracing::warn!(%error, "the Device Library could not be read and will be rebuilt");
            None
        }
    };
    let background = before
        .as_ref()
        .and_then(|b| b.property.as_ref().map(|p| p.2))
        .unwrap_or_else(|| u8::try_from(one.settings.background_color_type.clamp(0, 8)).unwrap_or(0));
    let rows = rows(one, before.as_ref(), background, date)?;
    let history = before.as_ref().map_or(&[][..], |b| b.history.as_slice());
    let scratch = build(&rows, history);
    let wanted = decode(&scratch)?;

    let (Some(existing), Some(before)) = (existing, before) else {
        return Ok(Some(Update { bytes: scratch, tables: Table::ALL.to_vec(), rebuilt: true, before: None, after: wanted }));
    };
    let tables: Vec<Table> = Table::ALL.into_iter().filter(|t| !before.same(&wanted, *t)).collect();
    if tables.is_empty() {
        return Ok(None);
    }
    let mut bytes = Some(existing.to_vec());
    for table in &tables {
        bytes = bytes.and_then(|b| rbl_pdb::build::replace_table(&b, table.page_type(), rows.of(*table)));
    }
    // A file whose chains are not as its header says, or that lacks a
    // table, cannot be edited in place; it is written whole, with the
    // history it holds.
    let (bytes, tables, rebuilt) = if let Some(bytes) = bytes {
        (bytes, tables, false)
    } else {
        tracing::warn!("the Device Library could not be edited in place and will be rebuilt");
        (scratch, Table::ALL.to_vec(), true)
    };
    let after = decode(&bytes)?;
    if Table::ALL.iter().any(|t| !after.same(&wanted, *t)) || after.history != before.history {
        return Err(Error::Verify("The new Device Library did not read back as planned. Nothing was changed.".into()));
    }
    Ok(Some(Update { bytes, tables, rebuilt, before: Some(before), after }))
}
