import { useCallback, useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { Fingerprint, SyncProgress } from "@/ipc/types";

/** Each USB's latest sync, running or finished, by its path. */
export type SyncJobs = ReadonlyMap<string, SyncProgress>;

function same(a: SyncProgress, b: SyncProgress | undefined): boolean {
  return !!b && a.running === b.running && a.phase === b.phase && a.path === b.path && a.error === b.error && a.report === b.report;
}

function sameJobs(a: SyncJobs, b: SyncJobs): boolean {
  return a.size === b.size && [...a].every(([path, job]) => same(job, b.get(path)));
}

/**
 * The backend owns the syncs, one per USB, side by side; this follows them,
 * twice a second while any runs. `onFinished` hears how each ended.
 */
export function useSyncJobs(onFinished: (progress: SyncProgress) => void) {
  const [jobs, setJobs] = useState<SyncJobs>(() => new Map());
  const finished = useRef(onFinished);
  finished.current = onFinished;
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const live = useRef(true);
  const follow = useCallback(async () => {
    clearTimeout(timer.current);
    try {
      const listed = await (await getBackend()).syncProgress();
      if (!live.current) return;
      const next: SyncJobs = new Map(listed.flatMap(job => job.path ? [[job.path, job] as const] : []));
      setJobs(current => {
        for (const [path, job] of next) {
          if (current.get(path)?.running && !job.running) queueMicrotask(() => finished.current(job));
        }
        return sameJobs(current, next) ? current : next;
      });
      if (listed.some(job => job.running)) timer.current = setTimeout(() => void follow(), 500);
    } catch {
      if (live.current) timer.current = setTimeout(() => void follow(), 1000);
    }
  }, []);
  useEffect(() => {
    live.current = true;
    void follow();
    return () => { live.current = false; clearTimeout(timer.current); };
  }, [follow]);
  const start = useCallback(async (path: string, fingerprint: Fingerprint) => {
    await (await getBackend()).startSync(path, fingerprint);
    setJobs(current => new Map(current).set(path, { running: true, phase: "reading", path, error: null, report: null }));
    void follow();
  }, [follow]);
  return { jobs, start };
}
