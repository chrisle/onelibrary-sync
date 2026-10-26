import type { Changes, Counts, Plan, Table } from "@/ipc/types";

/** The rows of the comparison, in the order people check them. */
export const COUNT_ROWS = [
  { key: "tracks", label: "Tracks" },
  { key: "playlists", label: "Playlists" },
  { key: "folders", label: "Playlist folders" },
  { key: "artists", label: "Artists" },
  { key: "albums", label: "Albums" },
  { key: "genres", label: "Genres" },
  { key: "labels", label: "Labels" },
  { key: "keys", label: "Keys" },
  { key: "artwork", label: "Artwork images" },
  { key: "myTags", label: "My Tags" },
] as const satisfies readonly { key: keyof Counts; label: string }[];

/** The tables of export.pdb by the names people know them by. */
export const TABLE_NAMES: Record<Table, string> = {
  tracks: "tracks", genres: "genres", artists: "artists", albums: "albums", labels: "labels", keys: "keys",
  colors: "colour names", playlists: "playlist tree", playlistEntries: "playlist contents", artwork: "artwork",
  columns: "browse columns", categories: "browse categories", sorts: "sort options and the column beside the track name", property: "device name and track count",
};

/** export.pdb holds twenty tables; this many are the sync's. */
export const PDB_TABLES = 20;

/** "3 added, 1 updated and 2 removed", or "No change". */
export function describeCounts(added: number, updated: number, removed: number): string {
  const parts = [
    added ? `${added.toLocaleString()} added` : "",
    updated ? `${updated.toLocaleString()} updated` : "",
    removed ? `${removed.toLocaleString()} removed` : "",
  ].filter(Boolean);
  if (parts.length === 0) return "No change";
  if (parts.length === 1) return parts[0] ?? "";
  return `${parts.slice(0, -1).join(", ")} and ${parts.at(-1) ?? ""}`;
}

export function anyChange(changes: Changes): boolean {
  return changes.tables.length > 0 || changes.myTags;
}

/** What happens to export.pdb, in a line. */
export function describeTables(changes: Changes): string {
  if (changes.created) return "Created";
  if (changes.rebuilt) return "Written whole: it could not be edited in place";
  if (changes.tables.length === 0) return "Left as it is";
  const names = changes.tables.map((table) => TABLE_NAMES[table]);
  return `${changes.tables.length} of ${PDB_TABLES} tables rewritten: ${names.join(", ")}`;
}

/** The lists besides tracks and playlists, as the comparison names them. */
const RECORDS: Partial<Record<Table, { label: string; key: keyof Counts }>> = {
  artists: { label: "Artists", key: "artists" }, albums: { label: "Albums", key: "albums" }, genres: { label: "Genres", key: "genres" },
  labels: { label: "Labels", key: "labels" }, keys: { label: "Keys", key: "keys" }, artwork: { label: "Artwork images", key: "artwork" },
};

/**
 * A line of what a sync changes: what the OneLibrary has on the left, what
 * the Device Library has now on the right, and what the sync does to it.
 */
export interface ChangeRow {
  label: string;
  oneLibrary: string;
  deviceLibrary: string;
  /** What the sync does to the Device Library; "" when it leaves it. */
  change: string;
}

type Side = Pick<Plan, "oneLibrary" | "deviceLibrary" | "changes">;

/** What a sync changes, line by line. Lists and settings that stay as they are are left out. */
export function changeRows(plan: Side): ChangeRow[] {
  const { oneLibrary: one, deviceLibrary: device, changes } = plan;
  const count = (counts: Counts | null, key: keyof Counts | "playlistsAndFolders") => {
    if (!counts) return "—";
    return (key === "playlistsAndFolders" ? counts.playlists + counts.folders : counts[key]).toLocaleString();
  };
  const counted = (label: string, key: keyof Counts | "playlistsAndFolders", added: number, updated: number, removed: number): ChangeRow => ({
    label, oneLibrary: count(one, key), deviceLibrary: count(device, key),
    change: added + updated + removed > 0 ? describeCounts(added, updated, removed) : "",
  });
  return [
    counted("Tracks", "tracks", changes.tracksAdded, changes.tracksUpdated, changes.tracksRemoved),
    counted("Playlists and folders", "playlistsAndFolders", changes.playlistsAdded, changes.playlistsUpdated, changes.playlistsRemoved),
    ...changes.records.flatMap(r => {
      const named = RECORDS[r.table];
      return named ? [counted(named.label, named.key, r.added, r.updated, r.removed)] : [];
    }),
    ...changes.settings.map(s => ({ label: s.name, oneLibrary: s.becomes || "None", deviceLibrary: s.now || "None", change: `Becomes ${s.becomes || "None"}` })),
    { label: "My Tags", oneLibrary: count(one, "myTags"), deviceLibrary: count(device, "myTags"), change: changes.myTags ? "exportExt.pdb rewritten" : "" },
    { label: "Files", oneLibrary: "exportLibrary.db, only read", deviceLibrary: device ? "export.pdb" : "None yet",
      change: changes.tables.length > 0 ? describeTables(changes) : "" },
  ];
}

/** The changes, as the confirmation lists them. */
export function summarize(plan: Side): string[] {
  return changeRows(plan).filter(row => row.change).map(row => `${row.label}: ${row.change}`);
}

/** What a sync does that people should know before agreeing to it. */
export function syncNotes(changes: Changes, hasDeviceLibrary: boolean, missingAudio: number, missingAnalysis: number): string[] {
  const notes: string[] = [];
  if (hasDeviceLibrary && changes.playlistsRemoved > 0) {
    notes.push("Playlists and folders that only the Device Library has, such as ones saved on a player, are removed. The sync goes one way, from the OneLibrary.");
  }
  if (hasDeviceLibrary && changes.tracksRemoved > 0) {
    notes.push("Tracks that only the Device Library has are taken off it. Their files stay on the USB.");
  }
  if (missingAudio > 0) {
    const one = missingAudio === 1;
    notes.push(`${missingAudio.toLocaleString()} ${one ? "track's audio file isn't" : "tracks' audio files aren't"} on this USB. Players list ${one ? "it" : "them"} but can't load ${one ? "it" : "them"}.`);
  }
  if (missingAnalysis > 0) {
    notes.push(`${missingAnalysis.toLocaleString()} ${missingAnalysis === 1 ? "track has" : "tracks have"} no analysis file on this USB. Players show no waveform or beat grid for ${missingAnalysis === 1 ? "it" : "them"}.`);
  }
  return notes;
}
