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

type Side = Pick<Plan, "oneLibrary" | "deviceLibrary" | "changes">;

/**
 * A line of the comparison: what the OneLibrary has on the left, what the
 * Device Library has now on the right, and what a sync does to it.
 */
export interface ComparisonRow {
  label: string;
  oneLibrary: string;
  deviceLibrary: string;
  /** What a sync does to this line of the Device Library; "" when it leaves it or no more can be said. */
  change: string;
  changed: boolean;
  /** The Device Library's count less the OneLibrary's; 0 for a setting. */
  difference: number;
  /** A count; a history count, which each library keeps its own of and is never synced; or a setting. */
  kind: "count" | "own" | "setting";
}

/** The tables whose changes are told on a count's line. */
const RECORD_KEYS: Partial<Record<Table, keyof Counts>> = {
  artists: "artists", albums: "albums", genres: "genres", labels: "labels", keys: "keys", artwork: "artwork",
};

function countChange(key: keyof Counts, changes: Changes): string {
  const told = (added: number, updated: number, removed: number) => added + updated + removed > 0 ? describeCounts(added, updated, removed) : "";
  switch (key) {
    case "tracks": return told(changes.tracksAdded, changes.tracksUpdated, changes.tracksRemoved);
    case "playlists": {
      const text = told(changes.playlistsAdded, changes.playlistsUpdated, changes.playlistsRemoved);
      return text && `${text}, folders included`;
    }
    case "myTags": return changes.myTags ? "Rewritten" : "";
    default: {
      const records = changes.records.find(r => RECORD_KEYS[r.table] === key);
      return records ? told(records.added, records.updated, records.removed) : "";
    }
  }
}

/** The comparison, line by line: every count, then the settings a sync changes. */
export function comparisonRows(plan: Side): ComparisonRow[] {
  const { oneLibrary: one, deviceLibrary: device, changes } = plan;
  const counts: ComparisonRow[] = COUNT_ROWS.map(({ key, label }) => {
    const change = device ? countChange(key, changes) : "";
    return {
      label, oneLibrary: one[key].toLocaleString(), deviceLibrary: device ? device[key].toLocaleString() : "—",
      change, changed: device !== null && (change !== "" || device[key] !== one[key]), difference: device ? device[key] - one[key] : 0, kind: "count",
    };
  });
  return [
    ...counts,
    { label: "History playlists", oneLibrary: one.histories.toLocaleString(), deviceLibrary: device ? device.histories.toLocaleString() : "—",
      change: "", changed: false, difference: 0, kind: "own" },
    ...changes.settings.map((s): ComparisonRow => ({
      label: s.name, oneLibrary: s.becomes || "None", deviceLibrary: s.now || "None", change: "", changed: true, difference: 0, kind: "setting",
    })),
  ];
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
