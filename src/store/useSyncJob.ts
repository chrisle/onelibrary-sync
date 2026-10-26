import { useCallback, useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { Fingerprint, SyncProgress } from "@/ipc/types";

const IDLE: SyncProgress = { running: false, phase: "", path: null, error: null, report: null };

function same(a: SyncProgress, b: SyncProgress): boolean {
  return a.running === b.running && a.phase === b.phase && a.path === b.path && a.error === b.error && a.report === b.report;
}

/**
 * The backend owns the sync; this follows it, twice a second while it
 * runs. `onFinished` hears how it ended.
 */
export function useSyncJob(onFinished: (progress: SyncProgress) => void) {
  const [progress, setProgress] = useState(IDLE);
  const finished = useRef(onFinished);
  finished.current = onFinished;
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const live = useRef(true);
  const follow = useCallback(async () => {
    clearTimeout(timer.current);
    try {
      const next = await (await getBackend()).syncProgress();
      if (!live.current) return;
      setProgress(current => {
        if (current.running && !next.running) queueMicrotask(() => finished.current(next));
        return same(current, next) ? current : next;
      });
      if (next.running) timer.current = setTimeout(() => void follow(), 500);
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
    setProgress({ ...IDLE, running: true, phase: "reading", path });
    void follow();
  }, [follow]);
  return { progress, start };
}
