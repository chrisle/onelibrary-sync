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
  expect(blockedReason({ rekordboxRunning: true }, true)).toBe("A sync is running.");
});

it("lists each USB with what a sync would do to it", async () => {
  await act(async () => { root.render(<SyncPane />); });
  await settle();
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
  expect(host.textContent).toContain("8 of 20 tables rewritten");
  const sortRow = [...host.querySelectorAll("[aria-label='What each library holds'] tr")].find(r => r.textContent?.startsWith("Column beside"));
  expect([...sortRow?.children ?? []].map(cell => cell.textContent)).toEqual(["Column beside the track name", "BPM", "Comments"]);
  expect(host.textContent).toContain("Playlists and folders that only the Device Library has");
});

it("syncs the USB the plan was read from and reads it again after", async () => {
  const start = vi.spyOn(held.backend, "startSync");
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  const sync = row("DJ STICK")?.querySelector<HTMLButtonElement>("button:not([aria-expanded])");
  expect(sync?.textContent).toBe("Sync…");
  await act(async () => { sync?.click(); });
  await settle();
  expect(start).toHaveBeenCalledWith("/Volumes/DJ STICK", expect.objectContaining({ files: expect.any(Array) }));
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
  expect(host.textContent).toContain("DJ STICK synced.");
  expect(row("DJ STICK")?.textContent).toContain("In sync");
});

it("does not sync a USB that is already in sync", async () => {
  await act(async () => { root.render(<SyncPane />); });
  await settle();
  expect(row("WARMUP")?.querySelector<HTMLButtonElement>("button:not([aria-expanded])")?.disabled).toBe(true);
  expect(buttons("Eject")).toHaveLength(4);
});
