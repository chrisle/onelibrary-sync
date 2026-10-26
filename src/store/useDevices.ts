import { useCallback, useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { Device, Plan } from "@/ipc/types";
import { errorMessage } from "@/lib/errorMessage";

/** A USB's plan: being read, read, or not readable. */
export type PlanState = { state: "checking" } | { state: "ready"; plan: Plan } | { state: "failed"; message: string };

/**
 * The connected volumes, listed again whenever one is connected or
 * disconnected, and the plan of each one with a OneLibrary, read one at a
 * time so a slow USB does not hold the others' reads up behind each other.
 */
export function useDevices() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [plans, setPlans] = useState<ReadonlyMap<string, PlanState>>(() => new Map());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const live = useRef(true);
  // Plans are read one at a time; each read waits for the one before.
  const queue = useRef<Promise<unknown>>(Promise.resolve());

  const setPlan = useCallback((path: string, state: PlanState) => {
    setPlans(current => new Map(current).set(path, state));
  }, []);

  const check = useCallback((device: Device) => {
    if (!device.hasOneLibrary || device.interrupted) return;
    setPlan(device.path, { state: "checking" });
    queue.current = queue.current.then(async () => {
      try {
        const plan = await (await getBackend()).planDevice(device.path);
        if (live.current) setPlan(device.path, { state: "ready", plan });
      } catch (e) {
        if (live.current) setPlan(device.path, { state: "failed", message: errorMessage(e) });
      }
    });
  }, [setPlan]);

  const load = useCallback(async () => {
    try {
      const listed = await (await getBackend()).listDevices();
      if (!live.current) return;
      setError("");
      setDevices(listed);
      setPlans(current => new Map([...current].filter(([path]) => listed.some(device => device.path === path))));
      listed.forEach(check);
    } catch (e) {
      if (live.current) setError(errorMessage(e));
    } finally {
      if (live.current) setLoading(false);
    }
  }, [check]);

  useEffect(() => {
    live.current = true;
    void load();
    let unsubscribe: (() => void) | undefined;
    void getBackend().then(b => b.onDevicesChanged(() => void load())).then(stop => {
      if (live.current) unsubscribe = stop; else stop();
    });
    return () => { live.current = false; unsubscribe?.(); };
  }, [load]);

  const recheck = useCallback((path: string) => {
    const device = devices.find(d => d.path === path);
    if (device) check(device);
  }, [devices, check]);

  return { devices, plans, loading, error, reload: load, recheck };
}
