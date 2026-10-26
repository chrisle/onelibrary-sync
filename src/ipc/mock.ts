/**
 * A stand-in backend for the browser, so the window can be designed and
 * tested without a USB. Query flags set the situation:
 * `?rekordbox=1` has rekordbox running, `?interrupted=1` leaves an
 * interrupted write on the first USB, `?fail=1` fails syncs, `?decline=1`
 * answers no to every confirmation, `?empty=1` connects nothing, and
 * `?late=1` connects another USB a few seconds after the window opens.
 */
import type { Backend, Changes, Counts, Device, Plan, Stamp, Status, SyncProgress } from "./types";

const GB = 1024 ** 3;

function flag(name: string): boolean {
  return new URLSearchParams(globalThis.location?.search ?? "").get(name) === "1";
}

function wait<T>(value: T, ms = 60): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

function counts(tracks: number, extra: Partial<Counts> = {}): Counts {
  return {
    tracks, playlists: Math.round(tracks / 40), folders: Math.round(tracks / 300), artists: Math.round(tracks * 0.8),
    albums: Math.round(tracks * 0.7), genres: 24, labels: Math.round(tracks * 0.4), keys: 24, artwork: Math.round(tracks * 0.7),
    myTags: 31, histories: 3, ...extra,
  };
}

function stamp(len: number, modifiedMs: number): Stamp {
  return { len, modifiedMs, sha256: modifiedMs.toString(16).padStart(64, "0") };
}

const NONE: Changes = {
  tracksAdded: 0, tracksUpdated: 0, tracksRemoved: 0, playlistsAdded: 0, playlistsUpdated: 0, playlistsRemoved: 0,
  tables: [], created: false, rebuilt: false, myTags: false, records: [], settings: [],
};

interface Stick {
  device: Device;
  one: Counts | null;
  legacy: Counts | null;
  changes: Changes;
  missingAudio: number;
  missingAnalysis: number;
  stamp: number;
}

function stick(name: string, device: Partial<Device>, rest: Omit<Stick, "device" | "stamp">): Stick {
  return {
    device: {
      path: `/Volumes/${name}`, name, fileSystem: "FAT32", totalBytes: 64 * GB, freeBytes: 21.4 * GB, removable: true,
      hasOneLibrary: rest.one !== null, hasDeviceLibrary: rest.legacy !== null, interrupted: false, ...device,
    },
    stamp: 1,
    ...rest,
  };
}

export function createMockBackend(): Backend {
  const sticks = new Map<string, Stick>();
  const add = (item: Stick) => sticks.set(item.device.path, item);
  if (!flag("empty")) {
    add(stick("DJ STICK", { interrupted: flag("interrupted") }, {
      one: counts(1_843), legacy: counts(1_812, { playlists: 47, myTags: 29 }),
      changes: { ...NONE, tracksAdded: 34, tracksUpdated: 6, tracksRemoved: 3, playlistsAdded: 2, playlistsUpdated: 5, playlistsRemoved: 1,
        tables: ["tracks", "artists", "albums", "playlists", "playlistEntries", "artwork", "sorts", "property"], myTags: true,
        records: [{ table: "artists", added: 12, updated: 1, removed: 0 }, { table: "albums", added: 9, updated: 0, removed: 2 }, { table: "artwork", added: 21, updated: 0, removed: 0 }],
        settings: [{ name: "Column beside the track name", now: "Comments", becomes: "BPM" }] },
      missingAudio: 0, missingAnalysis: 2,
    }));
    add(stick("TOUR USB", { fileSystem: "exFAT", totalBytes: 128 * GB, freeBytes: 88.2 * GB }, {
      one: counts(612), legacy: null,
      changes: { ...NONE, created: true, tracksAdded: 612, playlistsAdded: 18, myTags: true,
        tables: ["tracks", "genres", "artists", "albums", "labels", "keys", "colors", "playlists", "playlistEntries", "artwork", "columns", "categories", "sorts", "property"] },
      missingAudio: 4, missingAnalysis: 0,
    }));
    add(stick("WARMUP", { totalBytes: 32 * GB, freeBytes: 9.1 * GB }, {
      one: counts(288), legacy: counts(288), changes: NONE, missingAudio: 0, missingAnalysis: 0,
    }));
    add(stick("SAMSUNG T7", { fileSystem: "APFS", removable: false, totalBytes: 1000 * GB, freeBytes: 402 * GB }, {
      one: null, legacy: null, changes: NONE, missingAudio: 0, missingAnalysis: 0,
    }));
  }
  const listeners = new Set<() => void>();
  const changed = () => listeners.forEach((listener) => listener());
  if (flag("late")) {
    setTimeout(() => {
      add(stick("NEW STICK", {}, {
        one: counts(95), legacy: counts(90), changes: { ...NONE, tracksAdded: 5, playlistsUpdated: 1, tables: ["tracks", "playlistEntries", "property"] },
        missingAudio: 0, missingAnalysis: 0,
      }));
      changed();
    }, 3000);
  }

  let progress: SyncProgress = { running: false, phase: "", path: null, error: null, report: null };
  const refuse = (message: string) => Promise.reject(new Error(message));
  const find = (path: string) => sticks.get(path);

  const run = (item: Stick) => {
    const steps: SyncProgress["phase"][] = ["reading", "writing", "verifying"];
    let step = 0;
    const tick = () => {
      if (step < steps.length) {
        progress = { ...progress, phase: steps[step] ?? "reading" };
        step += 1;
        setTimeout(tick, 700);
      } else if (flag("fail")) {
        progress = { ...progress, running: false, phase: "failed", error: "The USB changed since it was checked. Nothing was changed; check it again and sync." };
      } else {
        progress = { ...progress, running: false, phase: "complete", report: { changes: item.changes, elapsedMs: 2_140 } };
        item.legacy = item.one ? { ...item.one } : null;
        item.device = { ...item.device, hasDeviceLibrary: true };
        item.changes = NONE;
        item.stamp += 1;
      }
    };
    tick();
  };

  return {
    status: (): Promise<Status> => wait({ rekordboxRunning: flag("rekordbox") }),
    listDevices: () => wait([...sticks.values()].map((item) => ({ ...item.device })), 150),
    planDevice: (path) => {
      const item = find(path);
      if (!item) return refuse("The USB is no longer connected.");
      if (!item.one) return refuse("This USB has no OneLibrary to sync from.");
      if (item.device.interrupted) return refuse("An earlier write to this USB was interrupted. Finish it before syncing.");
      const plan: Plan = {
        fingerprint: { files: [stamp(126_976, item.stamp), null, item.legacy ? stamp(262_144, item.stamp) : null, null] },
        deviceName: item.device.name, oneLibrary: item.one, deviceLibrary: item.legacy, changes: item.changes,
        missingAudio: item.missingAudio, missingAnalysis: item.missingAnalysis,
      };
      // A large OneLibrary takes a moment to read over USB.
      return wait(plan, 300 + item.one.tracks / 4);
    },
    startSync: (path, fingerprint) => {
      const item = find(path);
      if (!item) return refuse("The USB is no longer connected.");
      if (flag("rekordbox")) return refuse("Quit rekordbox before syncing. It writes to the USBs it sees.");
      if (progress.running) return refuse("A sync is already running.");
      if (fingerprint.files[0]?.modifiedMs !== item.stamp) return refuse("The USB changed since it was checked. Nothing was changed; check it again and sync.");
      progress = { running: true, phase: "reading", path, error: null, report: null };
      run(item);
      return wait(undefined);
    },
    syncProgress: () => wait({ ...progress }, 10),
    recoverDevice: (path) => {
      const item = find(path);
      if (item) item.device = { ...item.device, interrupted: false };
      return wait(undefined, 400);
    },
    ejectDevice: (path) => {
      sticks.delete(path);
      setTimeout(changed, 200);
      return wait(undefined, 300);
    },
    openDevice: () => wait(undefined),
    // A browser dialog would stop automated checks; `?decline=1` answers no.
    confirm: () => wait(!flag("decline")),
    onDevicesChanged: (callback) => {
      listeners.add(callback);
      return Promise.resolve(() => listeners.delete(callback));
    },
  };
}
