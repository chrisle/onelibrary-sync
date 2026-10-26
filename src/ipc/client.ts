import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Backend, Device, Fingerprint, Plan, Status, SyncProgress, UpdateCheck, UpdateProgress, UpdateReady } from "./types";

function tauriBackend(): Backend {
  return {
    status: () => invoke<Status>("status"),
    listDevices: () => invoke<Device[]>("list_devices"),
    planDevice: (path) => invoke<Plan>("plan_device", { path }),
    startSync: (path, fingerprint: Fingerprint) => invoke<void>("start_sync", { path, fingerprint }),
    syncProgress: () => invoke<SyncProgress[]>("sync_progress"),
    recoverDevice: (path) => invoke<void>("recover_device", { path }),
    ejectDevice: (path) => invoke<void>("eject_device", { path }),
    openDevice: (path) => invoke<void>("open_device", { path }),
    onDevicesChanged: async (callback) => {
      const { listen } = await import("@tauri-apps/api/event");
      return listen("devices-changed", () => callback());
    },
    checkForUpdate: () => invoke<UpdateCheck>("check_for_update"),
    downloadUpdate: () => invoke<UpdateReady>("download_update"),
    restartToUpdate: () => invoke<void>("restart_to_update"),
    onUpdateProgress: async (callback) => {
      const { listen } = await import("@tauri-apps/api/event");
      return listen<UpdateProgress>("update:progress", (event) => callback(event.payload));
    },
  };
}

let backend: Promise<Backend> | null = null;

/** The app's backend, or a stand-in when the page runs in a browser. */
export function getBackend(): Promise<Backend> {
  backend ??= isTauri() ? Promise.resolve(tauriBackend()) : import("./mock").then((mock) => mock.createMockBackend());
  return backend;
}
