import { describe, expect, it } from "vitest";
import type { Changes } from "@/ipc/types";
import { describeCounts, describeTables, summarize, syncNotes } from "./changes";

const none: Changes = {
  tracksAdded: 0, tracksUpdated: 0, tracksRemoved: 0, playlistsAdded: 0, playlistsUpdated: 0, playlistsRemoved: 0,
  tables: [], created: false, rebuilt: false, myTags: false,
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
    expect(summarize({ ...none, myTags: true }).at(-1)).toBe("My Tags: exportExt.pdb rewritten");
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
