import { describe, expect, it } from "vitest";
import type { Changes } from "@/ipc/types";
import type { Counts } from "@/ipc/types";
import { comparisonRows, describeCounts, describeTables, headline, summarize, syncNotes } from "./changes";

const one: Counts = { tracks: 473, playlists: 14, folders: 3, artists: 383, albums: 349, genres: 20, labels: 201, keys: 22, artwork: 386, myTags: 95, histories: 0 };
const none: Changes = {
  tracksAdded: 0, tracksUpdated: 0, tracksRemoved: 0, playlistsAdded: 0, playlistsUpdated: 0, playlistsRemoved: 0,
  tables: [], created: false, rebuilt: false, myTags: false, records: [], settings: [],
};

describe("sync changes", () => {
  it("counts as a sentence would", () => {
    expect(describeCounts(0, 0, 0)).toBe("No change");
    expect(describeCounts(3, 0, 0)).toBe("3 added");
    expect(describeCounts(3, 0, 2)).toBe("3 added and 2 removed");
    expect(describeCounts(1_200, 1, 2)).toBe(`${(1200).toLocaleString()} added, 1 updated and 2 removed`);
  });

  it("names the tables that are rewritten, and only those", () => {
    expect(describeTables(none)).toBe("Left as it is");
    expect(describeTables({ ...none, created: true, tables: ["tracks"] })).toBe("Created");
    expect(describeTables({ ...none, tables: ["playlists", "playlistEntries"] })).toBe("2 of 20 tables rewritten: playlist tree, playlist contents");
  });

  it("sets each count and differing setting of the OneLibrary against the Device Library's", () => {
    const sorts: Changes = { ...none, tables: ["sorts"], settings: [{ name: "Column beside the track name", now: "Comments", becomes: "" }] };
    const rows = comparisonRows({ oneLibrary: one, deviceLibrary: one, changes: sorts });
    expect(rows.map(row => row.label)).toEqual([
      "Tracks", "Playlists", "Playlist folders", "Artists", "Albums", "Genres", "Labels", "Keys", "Artwork images", "My Tags",
      "History playlists", "Column beside the track name",
    ]);
    expect(rows.filter(row => row.changed)).toEqual([
      { label: "Column beside the track name", oneLibrary: "None", deviceLibrary: "Comments", change: "", changed: true, difference: 0, kind: "setting" },
    ]);
    expect(headline({ oneLibrary: one, deviceLibrary: one, changes: sorts })).toBe("1 difference, marked in yellow. A sync makes the Device Library match the OneLibrary.");
    expect(summarize({ oneLibrary: one, deviceLibrary: one, changes: sorts })).toEqual([
      "Column beside the track name: Comments becomes None",
      "export.pdb: 1 of 20 tables rewritten: sort options and the column beside the track name",
    ]);

    const artists: Changes = { ...none, tables: ["artists", "playlists"], myTags: true, playlistsUpdated: 1,
      records: [{ table: "artists", added: 2, updated: 0, removed: 1 }] };
    const device = { ...one, artists: 382 };
    expect(comparisonRows({ oneLibrary: one, deviceLibrary: device, changes: artists }).filter(row => row.changed).map(row => [row.label, row.change, row.difference]))
      .toEqual([["Playlists", "1 updated, folders included", 0], ["Artists", "2 added and 1 removed", -1], ["My Tags", "Rewritten", 0]]);
    expect(headline({ oneLibrary: one, deviceLibrary: device, changes: artists })).toBe("3 differences, marked in yellow. A sync makes the Device Library match the OneLibrary.");
  });

  it("says when there is nothing to compare with, or nothing to do", () => {
    const created: Changes = { ...none, created: true, tables: ["tracks"] };
    expect(comparisonRows({ oneLibrary: one, deviceLibrary: null, changes: created }).some(row => row.changed)).toBe(false);
    expect(headline({ oneLibrary: one, deviceLibrary: null, changes: created })).toBe("A sync creates the Device Library from the OneLibrary.");
    expect(headline({ oneLibrary: one, deviceLibrary: one, changes: none })).toBe("The Device Library already holds what the OneLibrary holds.");
    expect(headline({ oneLibrary: one, deviceLibrary: one, changes: { ...none, tables: ["columns"] } })).toBe("A sync rewrites the Device Library's browse columns.");
  });

  it("warns about what only the Device Library had, and about files the USB lacks", () => {
    expect(syncNotes(none, true, 0, 0)).toEqual([]);
    expect(syncNotes({ ...none, playlistsRemoved: 1 }, false, 0, 0)).toEqual([]);
    expect(syncNotes({ ...none, playlistsRemoved: 1 }, true, 0, 0)[0]).toMatch(/^Playlists and folders that only the Device Library has/);
    expect(syncNotes(none, true, 1, 2)).toEqual([
      "1 track's audio file isn't on this USB. Players list it but can't load it.",
      "2 tracks have no analysis file on this USB. Players show no waveform or beat grid for them.",
    ]);
  });
});
