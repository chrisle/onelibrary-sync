/** What stands in the way of a sync, apart from the USB itself. */
export interface Status {
  rekordboxRunning: boolean;
}

/** A connected volume. */
export interface Device {
  path: string;
  name: string;
  fileSystem: string;
  totalBytes: number;
  freeBytes: number;
  removable: boolean;
  hasOneLibrary: boolean;
  hasDeviceLibrary: boolean;
  /** An earlier write was interrupted and has to be finished first. */
  interrupted: boolean;
}

/** How much a library holds. */
export interface Counts {
  tracks: number;
  playlists: number;
  folders: number;
  artists: number;
  albums: number;
  genres: number;
  labels: number;
  keys: number;
  artwork: number;
  myTags: number;
  histories: number;
}

/** A table of export.pdb. */
export type Table = "tracks" | "genres" | "artists" | "albums" | "labels" | "keys" | "colors" | "playlists"
  | "playlistEntries" | "artwork" | "columns" | "categories" | "sorts" | "property";

/** What a sync changes in the Device Library. */
export interface Changes {
  tracksAdded: number;
  tracksUpdated: number;
  tracksRemoved: number;
  /** Playlists and folders. */
  playlistsAdded: number;
  playlistsUpdated: number;
  playlistsRemoved: number;
  /** The tables of export.pdb that are written; the rest stay as they are. */
  tables: Table[];
  /** export.pdb is created. */
  created: boolean;
  /** export.pdb is written whole because it could not be edited in place. */
  rebuilt: boolean;
  /** exportExt.pdb, the My Tags, is written. */
  myTags: boolean;
}

/** The USB's database files by size and time, as the plan saw them. */
export interface Fingerprint {
  files: ([number, number] | null)[];
}

/** What a USB holds and what syncing it would do. */
export interface Plan {
  fingerprint: Fingerprint;
  deviceName: string;
  oneLibrary: Counts;
  /** null when the USB has no Device Library yet. */
  deviceLibrary: Counts | null;
  changes: Changes;
  missingAudio: number;
  missingAnalysis: number;
}

export interface Report {
  changes: Changes;
  elapsedMs: number;
}

export type SyncPhase = "" | "reading" | "writing" | "verifying" | "complete" | "failed";

export interface SyncProgress {
  running: boolean;
  phase: SyncPhase;
  path: string | null;
  error: string | null;
  report: Report | null;
}

export interface Backend {
  status(): Promise<Status>;
  listDevices(): Promise<Device[]>;
  /** Reads the USB's libraries; a second or so for a large one. */
  planDevice(path: string): Promise<Plan>;
  /** Refused when the USB no longer matches the plan's fingerprint. */
  startSync(path: string, fingerprint: Fingerprint): Promise<void>;
  syncProgress(): Promise<SyncProgress>;
  recoverDevice(path: string): Promise<void>;
  ejectDevice(path: string): Promise<void>;
  openDevice(path: string): Promise<void>;
  confirm(message: string, labels?: { yes: string; no: string }): Promise<boolean>;
  /** Calls back when a volume is connected or disconnected; resolves to an unsubscribe. */
  onDevicesChanged(callback: () => void): Promise<() => void>;
}
