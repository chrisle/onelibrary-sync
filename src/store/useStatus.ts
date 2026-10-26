import { useCallback, useEffect, useRef, useState } from "react";
import { getBackend } from "@/ipc/client";
import type { Status } from "@/ipc/types";

/** Whether rekordbox is open, checked every two seconds. */
export function useStatus() {
  const [status, setStatus] = useState<Status | null>(null);
  const again = useRef<() => void>(() => undefined);
  useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      clearTimeout(timer);
      try {
        const next = await (await getBackend()).status();
        if (live) setStatus(current => current?.rekordboxRunning === next.rekordboxRunning ? current : next);
      } catch {
        // Keep the last known status; the next check may succeed.
      } finally {
        if (live) timer = setTimeout(() => void refresh(), 2000);
      }
    };
    again.current = () => void refresh();
    void refresh();
    return () => { live = false; clearTimeout(timer); };
  }, []);
  const refresh = useCallback(() => again.current(), []);
  return { status, refresh };
}
