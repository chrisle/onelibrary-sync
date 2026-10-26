import { describe, expect, it } from "vitest";
import type { Changes } from "@/ipc/types";
import type { Counts } from "@/ipc/types";
import { changeRows, describeCounts, describeTables, summarize, syncNotes } from "./changes";

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

  it("sets what the OneLibrary has against what the Device Library has now", () => {
    const sorts: Changes = { ...none, tables: ["sorts"], settings: [{ name: "Column beside the track name", now: "Comments", becomes: "" }] };
    expect(changeRows({ oneLibrary: one, deviceLibrary: one, changes: sorts })).toEqual([
      { label: "Tracks", oneLibrary: "473", deviceLibrary: "473", change: "" },
      { label: "Playlists and folders", oneLibrary: "17", deviceLibrary: "17", change: "" },
      { label: "Column beside the track name", oneLibrary: "None", deviceLibrary: "Comments", change: "Becomes None" },
      { label: "My Tags", oneLibrary: "95", deviceLibrary: "95", change: "" },
      { label: "Files", oneLibrary: "exportLibrary.db, only read", deviceLibrary: "export.pdb", change: "1 of 20 tables rewritten: sort options and the column beside the track name" },
    ]);
    const artists: Changes = { ...none, tables: ["artists"], myTags: true, records: [{ table: "artists", added: 2, updated: 0, removed: 1 }] };
    expect(changeRows({ oneLibrary: one, deviceLibrary: { ...one, artists: 382 }, changes: artists })[2])
      .toEqual({ label: "Artists", oneLibrary: "383", deviceLibrary: "382", change: "2 added and 1 removed" });
    expect(summarize({ oneLibrary: one, deviceLibrary: null, changes: artists })).toEqual([
      "Artists: 2 added and 1 removed",
      "My Tags: exportExt.pdb rewritten",
      "Files: 1 of 20 tables rewritten: artists",
    ]);
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
