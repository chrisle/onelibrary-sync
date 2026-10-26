import type { Changes, Counts, Table } from "@/ipc/types";

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
  columns: "browse columns", categories: "browse categories", sorts: "sort options", property: "device name and track count",
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

/** The changes, as the confirmation lists them. */
export function summarize(changes: Changes): string[] {
  const lines = [
    `Tracks: ${describeCounts(changes.tracksAdded, changes.tracksUpdated, changes.tracksRemoved)}`,
    `Playlists and folders: ${describeCounts(changes.playlistsAdded, changes.playlistsUpdated, changes.playlistsRemoved)}`,
    `export.pdb: ${describeTables(changes)}`,
  ];
  if (changes.myTags) lines.push("My Tags: exportExt.pdb rewritten");
  return lines;
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
