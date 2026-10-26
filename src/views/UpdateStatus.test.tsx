// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Backend } from "@/ipc/types";
import { createMockBackend } from "@/ipc/mock";
import { AUTO_CHECK_AFTER_MS } from "@/store/useUpdater";
import { SyncPane } from "./SyncPane";

const held = vi.hoisted(() => ({ backend: null as unknown as Backend }));
vi.mock("@/ipc/client", () => ({ getBackend: () => Promise.resolve(held.backend) }));

let host: HTMLDivElement;
let root: Root;
const subtitle = () => host.querySelector("section > p")?.textContent ?? "";
const link = (name: string) => [...host.querySelectorAll<HTMLButtonElement>("section > p button")].find(b => b.textContent === name);
/** Moves time on in small steps, so timers each step sets up run too. */
async function advance(ms: number) {
  for (let left = ms; left > 0; left -= 250) await act(async () => { await vi.advanceTimersByTimeAsync(Math.min(250, left)); });
}

/** The window with the stand-in backend set up by `query`, as `?update=1` in the browser. */
async function open(query: string) {
  window.history.replaceState(null, "", `/${query}`);
  held.backend = createMockBackend();
  await act(async () => { root.render(<SyncPane />); });
}

beforeEach(() => {
  vi.useFakeTimers();
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(() => { act(() => root.unmount()); host.remove(); vi.useRealTimers(); window.history.replaceState(null, "", "/"); });

it("checks a while after launch, downloads what it finds and offers a restart", async () => {
  await open("?update=1");
  const download = vi.spyOn(held.backend, "downloadUpdate");
  expect(link("Check for updates")).toBeDefined();
  await advance(AUTO_CHECK_AFTER_MS + 500);
  expect(download).toHaveBeenCalledTimes(1);
  expect(subtitle()).toMatch(/Downloading version 9\.9\.9…/);
  await advance(5_000);
  expect(subtitle()).toContain("Version 9.9.9 is ready.");
  expect(link("Restart now")?.disabled).toBe(false);
});

it("waits for a sync to finish before it restarts into an update", async () => {
  await open("?update=1");
  await advance(AUTO_CHECK_AFTER_MS + 5_000);
  const restart = vi.spyOn(held.backend, "restartToUpdate");
  const box = [...host.querySelectorAll("tbody tr")].find(r => r.textContent?.includes("DJ STICK"))?.querySelector<HTMLInputElement>("input");
  await act(async () => { box?.click(); });
  await act(async () => { [...host.querySelectorAll("button")].find(b => b.textContent === "Sync")?.click(); });
  await advance(500);
  expect(link("Restart now")?.disabled).toBe(true);
  await act(async () => { link("Restart now")?.click(); });
  expect(restart).not.toHaveBeenCalled();
  await advance(10_000);
  expect(link("Restart now")?.disabled).toBe(false);
});

it("says nothing when its own check fails, and why when a check asked for fails", async () => {
  await open("?updatefail=1");
  await advance(AUTO_CHECK_AFTER_MS + 1_000);
  expect(subtitle()).not.toContain("could not reach");
  await act(async () => { link("Check for updates")?.click(); });
  expect(subtitle()).toContain("Checking for updates…");
  await advance(1_000);
  expect(subtitle()).toContain("The update check could not reach the download server.");
  expect(link("Try again")).toBeDefined();
});

it("says so when a check asked for finds nothing newer", async () => {
  await open("");
  await act(async () => { link("Check for updates")?.click(); });
  await advance(1_000);
  expect(subtitle()).toContain("Up to date.");
});
