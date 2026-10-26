/**
 * Software updates, taken as RBXport takes them.
 *
 * One state machine: idle → checking → up to date | available →
 * downloading → installing → ready, with failed reachable from any of the
 * working states. The backend does the checking, downloading and installing;
 * this holds where it has got to.
 *
 * An update is taken without asking. A while after launch the app checks on
 * its own and downloads what it finds, putting it in place where the platform
 * allows, so the next launch is the new version; Restart now runs it sooner.
 * A check the app ran on its own that fails is not shown: the next launch
 * tries again. A check somebody asked for shows how it ended.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import { getBackend } from "@/ipc/client";
import { errorMessage } from "@/lib/errorMessage";
import type { UpdateCheck, UpdateProgress, UpdateReady } from "@/ipc/types";

/** How long after launch the automatic check runs: after the USBs are read, not before. */
export const AUTO_CHECK_AFTER_MS = 5_000;

export type UpdaterState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "upToDate"; currentVersion: string }
  /** The update was found; the effect below starts its download. */
  | { phase: "available"; check: UpdateCheck }
  | { phase: "downloading"; check: UpdateCheck; progress: UpdateProgress | null }
  | { phase: "installing"; check: UpdateCheck }
  /** Downloaded and, where the platform allows, already in place. */
  | { phase: "ready"; check: UpdateCheck; ready: UpdateReady }
  | { phase: "failed"; message: string; check: UpdateCheck | null };

export interface Updater {
  state: UpdaterState;
  /** The current work was started by the automatic check after launch. */
  automatic: boolean;
  /** Ask the server. */
  check: () => void;
  /** Download what the last check found, after a download that failed. */
  retry: () => void;
  /** Restart into the downloaded update now rather than at the next launch. */
  restart: () => void;
}

export function useUpdater(autoCheck = true): Updater {
  const [state, setState] = useState<UpdaterState>({ phase: "idle" });
  const [automatic, setAutomatic] = useState(false);
  // The state outside a render, so an action can read it without a side
  // effect inside a state updater.
  const latest = useRef(state);
  latest.current = state;
  // The check that is running, so a slow one does not overwrite a newer one.
  const sequence = useRef(0);
  const downloadStarted = useRef(0);

  // The download, and its end: in place, staged, or failed. The backend
  // answers at once when the version is already downloaded this run.
  const download = useCallback((found: UpdateCheck, mine: number) => {
    setState({ phase: "downloading", check: found, progress: null });
    void (async () => {
      try {
        const ready = await (await getBackend()).downloadUpdate();
        if (mine === sequence.current) setState({ phase: "ready", check: found, ready });
      } catch (error) {
        if (mine === sequence.current) setState({ phase: "failed", message: errorMessage(error), check: found });
      }
    })();
  }, []);

  const run = useCallback((manual: boolean) => {
    const current = latest.current;
    // Work already under way, or done: nothing to start over.
    if (current.phase === "downloading" || current.phase === "installing" || current.phase === "ready") return;
    const mine = ++sequence.current;
    setAutomatic(!manual);
    setState({ phase: "checking" });
    void (async () => {
      try {
        const found = await (await getBackend()).checkForUpdate();
        if (mine !== sequence.current) return;
        if (found.version === null) setState({ phase: "upToDate", currentVersion: found.currentVersion });
        else if (found.ready) setState({ phase: "ready", check: found, ready: found.ready });
        else setState({ phase: "available", check: found });
      } catch (error) {
        if (mine === sequence.current) setState({ phase: "failed", message: errorMessage(error), check: null });
      }
    })();
  }, []);

  // Every update found is downloaded. Its own render keeps the transition
  // out of the check's async callback.
  useEffect(() => {
    if (state.phase !== "available") return;
    const mine = sequence.current;
    if (downloadStarted.current === mine) return;
    downloadStarted.current = mine;
    download(state.check, mine);
  }, [state, download]);

  const retry = useCallback(() => {
    const current = latest.current;
    if (current.phase !== "failed") return;
    setAutomatic(false);
    if (current.check) download(current.check, sequence.current); else run(true);
  }, [download, run]);

  const restart = useCallback(() => {
    const current = latest.current;
    if (current.phase !== "ready") return;
    const found = current.check;
    void (async () => {
      try {
        // A success never returns: the process ends. Returning is failure.
        await (await getBackend()).restartToUpdate();
        setState({ phase: "failed", message: "The update did not restart the app.", check: found });
      } catch (error) {
        setState({ phase: "failed", message: errorMessage(error), check: found });
      }
      setAutomatic(false);
    })();
  }, []);

  // The download's progress, and the moment it turns into an install.
  useEffect(() => {
    let live = true;
    let stop: (() => void) | undefined;
    void getBackend().then(backend => backend.onUpdateProgress(progress => {
      setState(current => {
        if (current.phase !== "downloading") return current;
        const done = progress.total !== null && progress.downloaded >= progress.total;
        return done ? { phase: "installing", check: current.check } : { ...current, progress };
      });
    })).then(unsubscribe => {
      if (live) stop = unsubscribe; else unsubscribe();
    });
    return () => { live = false; stop?.(); };
  }, []);

  useEffect(() => {
    if (!autoCheck) return;
    const timer = setTimeout(() => run(false), AUTO_CHECK_AFTER_MS);
    return () => clearTimeout(timer);
  }, [autoCheck, run]);

  const check = useCallback(() => run(true), [run]);
  return { state, automatic, check, retry, restart };
}
