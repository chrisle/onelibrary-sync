// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Backend } from "@/ipc/types";
import { createMockBackend } from "@/ipc/mock";
import { SyncPane, blockedReason } from "./SyncPane";

const held = vi.hoisted(() => ({ backend: null as unknown as Backend }));
vi.mock("@/ipc/client", () => ({ getBackend: () => Promise.resolve(held.backend) }));

let host: HTMLDivElement;
let root: Root;
const buttons = (name: string) => [...host.querySelectorAll("button")].filter(b => b.textContent === name);
const settle = () => act(async () => { await vi.advanceTimersByTimeAsync(3000); });
const row = (name: string) => [...host.querySelectorAll("tbody tr")].find(r => r.textContent?.includes(name));

beforeEach(() => {
  vi.useFakeTimers();
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  // jsdom lays nothing out, so it has no scrolling.
  Element.prototype.scrollIntoView = vi.fn();
  held.backend = createMockBackend();
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(() => { act(() => root.unmount()); host.remove(); vi.useRealTimers(); });

it("says what stands in the way of a sync", () => {
  expect(blockedReason({ rekordboxRunning: false }, false)).toBe("");
  expect(blockedReason(null, false)).toMatch(/^Checking/);
  expect(blockedReason({ rekordboxRunning: true }, false)).toMatch(/^Quit rekordbox/);
  expect(blockedReason({ rekordboxRunning: true }, true)).toBe("This USB is being synced.");
});

it("lists each USB with what a sync would do to it", async () => {
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  expect(host.textContent).toContain("Version 0.");
  expect(host.querySelectorAll("tbody tr")).toHaveLength(4);
  expect(row("DJ STICK")?.textContent).toContain("Needs sync");
  expect(row("TOUR USB")?.textContent).toContain("No Device Library yet");
  expect(row("WARMUP")?.textContent).toContain("In sync");
  expect(row("SAMSUNG T7")?.textContent).toContain("No OneLibrary");
  await act(async () => { row("DJ STICK")?.querySelector<HTMLButtonElement>("button[aria-expanded]")?.click(); });
  await settle();
  const details = host.querySelector("[aria-label='What each library holds']");
  expect(details?.textContent).toContain("1,843");
  expect(host.textContent).toContain("34 added, 6 updated and 3 removed");
  expect(host.textContent).toContain("8 differences");
  expect(host.textContent).toContain("FAT32");
  const sortRow = [...host.querySelectorAll("[aria-label='What each library holds'] tr")].find(r => r.textContent?.startsWith("Column beside"));
  expect([...sortRow?.children ?? []].map(cell => cell.textContent)).toEqual(["Column beside the track name", "BPM", "Comments"]);
  expect(host.textContent).toContain("Playlists and folders that only the Device Library has");
});

const box = (name: string) => row(name)?.querySelector<HTMLInputElement>("input[type=checkbox]");
const syncButton = () => buttons("Sync")[0];

it("syncs the ticked USB with Sync at the bottom and reads it again after", async () => {
  const start = vi.spyOn(held.backend, "startSync");
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  // Sync sits under the list, not in each row, and waits for a tick.
  expect(buttons("Sync")).toHaveLength(1);
  expect(row("DJ STICK")?.querySelector("button:not([aria-expanded]):not([aria-label^=Eject])")).toBeNull();
  expect(syncButton()?.disabled).toBe(true);
  await act(async () => { box("DJ STICK")?.click(); });
  expect(syncButton()?.disabled).toBe(false);
  await act(async () => { syncButton()?.click(); });
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(start).toHaveBeenCalledWith("/Volumes/DJ STICK", expect.objectContaining({ files: expect.any(Array) }));
  // The row's space gives way to the sync's step while it runs.
  expect(row("DJ STICK")?.textContent).toMatch(/Syncing…/);
  expect(row("DJ STICK")?.textContent).toMatch(/\d of 3/);
  expect(row("DJ STICK")?.textContent).not.toContain("GB free");
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
  await settle();
  expect(host.textContent).toContain("DJ STICK synced.");
  expect(row("DJ STICK")?.textContent).toContain("In sync");
});

it("ejects each USB from an icon in its row", async () => {
  const eject = vi.spyOn(held.backend, "ejectDevice");
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  const ejects = [...host.querySelectorAll<HTMLButtonElement>("button[aria-label^='Eject ']")];
  expect(ejects).toHaveLength(4);
  expect(ejects.every(button => button.querySelector("svg") && button.textContent === "")).toBe(true);
  await act(async () => { row("WARMUP")?.querySelector<HTMLButtonElement>("button[aria-label='Eject WARMUP']")?.click(); });
  await settle();
  expect(eject).toHaveBeenCalledWith("/Volumes/WARMUP");
  expect(row("WARMUP")).toBeUndefined();
});

it("syncs the ticked USBs side by side", async () => {
  const start = vi.spyOn(held.backend, "startSync");
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  // A USB already in sync can be ticked too; only one without a OneLibrary cannot.
  expect(box("WARMUP")?.disabled).toBe(false);
  expect(box("SAMSUNG T7")?.disabled).toBe(true);
  await act(async () => { box("DJ STICK")?.click(); });
  await act(async () => { box("TOUR USB")?.click(); });
  await act(async () => { box("WARMUP")?.click(); });
  await act(async () => { syncButton()?.click(); });
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(start).toHaveBeenCalledTimes(3);
  expect(row("DJ STICK")?.textContent).toMatch(/Syncing…/);
  expect(row("TOUR USB")?.textContent).toMatch(/Syncing…/);
  expect(host.textContent).toContain("Keep DJ STICK, TOUR USB and WARMUP connected");
  expect(syncButton()?.disabled).toBe(true);
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
  await settle();
  expect(host.textContent).toContain("DJ STICK synced.");
  expect(host.textContent).toContain("TOUR USB synced.");
  expect(host.textContent).toContain("WARMUP was already in sync.");
  expect(row("DJ STICK")?.textContent).toContain("In sync");
  expect(row("TOUR USB")?.textContent).toContain("In sync");
});

it("ejects the synced USBs when Eject when done is ticked", async () => {
  const eject = vi.spyOn(held.backend, "ejectDevice");
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  await act(async () => { box("DJ STICK")?.click(); });
  await act(async () => { box("TOUR USB")?.click(); });
  await act(async () => { host.querySelector<HTMLInputElement>("input[aria-label='Eject when done']")?.click(); });
  await act(async () => { syncButton()?.click(); });
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
  await settle();
  expect(eject).toHaveBeenCalledWith("/Volumes/DJ STICK");
  expect(eject).toHaveBeenCalledWith("/Volumes/TOUR USB");
  expect(host.textContent).toContain("DJ STICK synced. Players that read the Device Library now see what its OneLibrary holds. It was ejected and can be unplugged.");
  expect(row("DJ STICK")).toBeUndefined();
  expect(row("TOUR USB")).toBeUndefined();
  expect(row("WARMUP")).toBeDefined();
});
