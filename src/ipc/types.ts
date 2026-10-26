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
  /** The artists, albums, genres, labels, keys and artwork that change. */
  records: Records[];
  /** The settings that change, such as the column beside the track name. Empty when export.pdb is created. */
  settings: Setting[];
}

/** How a list of the Device Library changes. */
export interface Records {
  table: Table;
  added: number;
  updated: number;
  removed: number;
}

/** A setting of the Device Library that a sync changes. */
export interface Setting {
  name: string;
  /** What the Device Library has now. */
  now: string;
  /** What it has after the sync: the OneLibrary's. */
  becomes: string;
}

/** The USB's database files by size, time and content, as the plan saw them. */
export interface Fingerprint {
  files: (Stamp | null)[];
}

export interface Stamp {
  len: number;
  modifiedMs: number;
  sha256: string;
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

/** What an update check found. */
export interface UpdateCheck {
  currentVersion: string;
  /** The version on offer, or null when this build is the newest. */
  version: string | null;
  /** The version on offer is already downloaded this run. */
  ready: UpdateReady | null;
}

/** A downloaded update. */
export interface UpdateReady {
  version: string;
  /** The next launch runs it as things stand; false when an installer still has to run, at the quit or on Restart now. */
  installed: boolean;
}

export interface UpdateProgress {
  downloaded: number;
  /** null when the server did not say how big the file is. */
  total: number | null;
}

export interface Backend {
  status(): Promise<Status>;
  listDevices(): Promise<Device[]>;
  /** Reads the USB's libraries; a second or so for a large one. */
  planDevice(path: string): Promise<Plan>;
  /** Refused when the USB no longer matches the plan's fingerprint, or is already syncing. Other USBs may sync meanwhile. */
  startSync(path: string, fingerprint: Fingerprint): Promise<void>;
  /** Each USB's latest sync, running or finished. */
  syncProgress(): Promise<SyncProgress[]>;
  recoverDevice(path: string): Promise<void>;
  ejectDevice(path: string): Promise<void>;
  openDevice(path: string): Promise<void>;
  /** Calls back when a volume is connected or disconnected; resolves to an unsubscribe. */
  onDevicesChanged(callback: () => void): Promise<() => void>;
  checkForUpdate(): Promise<UpdateCheck>;
  /** Downloads what the last check found and puts it in place, or stages it where the app must quit first. */
  downloadUpdate(): Promise<UpdateReady>;
  /** Restarts into the downloaded update. A success never returns. */
  restartToUpdate(): Promise<void>;
  /** Calls back as a download goes; resolves to an unsubscribe. */
  onUpdateProgress(callback: (progress: UpdateProgress) => void): Promise<() => void>;
}
